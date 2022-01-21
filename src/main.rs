mod handlers;

use std::{
    error, fs,
    fs::File,
    io::{self, BufRead, Read, Write},
    net::SocketAddr,
    process,
    sync::{Arc, Mutex},
    thread,
};

use axum::routing::get;
use axum::Router;
use directories::ProjectDirs;
use mc_server_wrapper::Wrapper;
use serde::{Deserialize, Serialize};
use tokio::sync;

const DEFAULT_CONFIG_FILE_NAME: &str = "config.yaml";
const DEFAULT_PORT: u16 = 6969;
// Assume that users run the mc-server-wrapper binary in the same directory as
// their server.jar file.
const DEFAULT_SERVER_JAR_PATH: &str = "server.jar";
const DEFAULT_MAX_MEMORY_BUFFER_SIZE: u16 = 2048;

// TODO: Write doc comments for each of these fields.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    port: u16,
    server_jar_path: String,
    max_memory_buffer_size: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    // Initialize a Config with default values. If a config file is present on
    // disk, those defaults are replaced by that file's contents.
    let config = get_config()?;

    // Get a new server wrapper, and wait for that wrapper to launch the
    // underlying Minecraft server.
    //
    // The wrapper is guarded by a mutex because it's not designed to be used
    // asynchronously, but a shared reference of it is given to each HTTP API
    // handler.
    //
    // That whole thing is wrapped in an Arc so we can share ownership of that
    // mutex across multiple async tasks, and consequently multiple threads.
    let wrapper = Arc::new(Mutex::new(Wrapper::new(
        config.max_memory_buffer_size,
        &config.server_jar_path,
    )?));
    wrapper.lock().unwrap().wait_for_server_to_spin_up();

    // Get a one-time-use channel that will carry a message indicating that the
    // warp server should be shut down. Designed to be used by the handler for
    // the /stop route -- this way, when the Minecraft server spins down, we'll
    // stop serving new incoming requests to talk to it.
    let (shutdown_signal_tx, shutdown_signal_rx) = sync::oneshot::channel::<()>();
    // Wrapped in an Arc<Mutex<_>> for the same reasons as the server wrapper.
    let shutdown_signal_tx_mutex = Arc::new(Mutex::new(Some(shutdown_signal_tx)));

    // Set up API route handlers.
    let routes = Router::new()
        .route(
            "/stop",
            get({
                let wrapper = Arc::clone(&wrapper);
                move || handlers::stop_server(Arc::clone(&wrapper), shutdown_signal_tx_mutex)
            }),
        )
        .route(
            "/list-players",
            get({
                let wrapper = Arc::clone(&wrapper);
                move || handlers::list_players(Arc::clone(&wrapper))
            }),
        );

    // Pass any lines that are written to stdin onto the underlying Minecraft
    // server's stdin pipe. This lets server admins with access to the machine
    // that the server is running on interact with it the same way they would if
    // this wrapper weren't present.
    let stdin_reader = io::BufReader::new(io::stdin());
    thread::spawn(move || {
        stdin_reader
            .lines()
            .filter_map(|line| line.ok())
            .for_each(|line| {
                if let Err(e) = wrapper.lock().unwrap().run_custom_command(&line) {
                    // TODO: Handle this error properly.
                    eprintln!("something went wrong while trying to pass a command to the wrapper's stdin: {}", e);
                }
            });
    });

    // Stand up the API server.
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    axum::Server::bind(&addr)
        .serve(routes.into_make_service())
        .with_graceful_shutdown(async {
            shutdown_signal_rx.await.ok();
        })
        .await
        .unwrap();

    Ok(())
}

/// Reads configs from a config file, and returns a [Config] with those values.
/// If a config file doesn't exist, it creates one with sensible defaults, and
/// returns a [Config] populated with those defaults.
///
/// The config file lives in the canonical place depending on the operating
/// system that the user is running the mc-server-wrapper binary on. The
/// `directories` crate determines where that location is.
fn get_config() -> Result<Config, Box<dyn error::Error>> {
    // Create a Config with sensible defaults. If a config file is present,
    // these will be overwritten after that file is read.
    let mut config = Config {
        port: DEFAULT_PORT,
        server_jar_path: DEFAULT_SERVER_JAR_PATH.to_string(),
        max_memory_buffer_size: DEFAULT_MAX_MEMORY_BUFFER_SIZE,
    };

    if let Some(proj_dirs) = ProjectDirs::from("com", "nchaloult", "mc-server-wrapper") {
        let config_dir = proj_dirs.config_dir();
        let config_file_path = config_dir.join(DEFAULT_CONFIG_FILE_NAME);
        let mut config_file = File::options().read(true).write(true).open(&config_file_path).unwrap_or_else(|err| {
            if err.kind() == io::ErrorKind::NotFound {
                // Create an empty config file. Later on, when we see that this
                // file is empty, we won't overwrite any of the values in our
                // default config instantiated above.
                fs::create_dir_all(&config_dir).unwrap_or_else(|err| {
                    // TODO: Improve error message.
                    eprintln!("something went wrong while making a {:?} directory for the config file to live in: {}", &config_dir, err);
                    process::exit(1);
                });
                // We can't use something more simple here like
                // fs::File::create() because we need to be able to read from
                // this file later on.
                File::options().read(true).write(true).create_new(true).open(&config_file_path).unwrap_or_else(|err| {
                    // TODO: Improve error message.
                    eprintln!(
                        "something went wrong while trying to create a config file at {:?}: {}",
                        &config_file_path, err
                    );
                    process::exit(1);
                })
            } else {
                eprintln!(
                    "something went wrong while trying to open the config file: {:?}: {}",
                    &config_file_path, err
                );
                process::exit(1);
            }
        });

        let mut config_file_contents = String::new();
        config_file.read_to_string(&mut config_file_contents)?;
        if config_file_contents.is_empty() {
            // Write the default configs into that file.
            //
            // Set config_file_contents so the logic below can act like the file
            // we just read wasn't actually empty.
            config_file_contents = serde_yaml::to_string(&config)?;
            config_file.write_all(config_file_contents.as_bytes())?;
        }
        // Overwrite our config struct with the config file's contents.
        config = serde_yaml::from_str(&config_file_contents)?;
    }

    Ok(config)
}
