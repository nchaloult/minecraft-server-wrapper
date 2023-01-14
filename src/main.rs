mod handlers;

use std::{
    error,
    fs::{self, File},
    io::{self, BufRead, Read, Write},
    net::SocketAddr,
    process,
    sync::{Arc, Mutex},
    thread,
};

use anyhow::{bail, Context};
use axum::{routing::get, Router};
use directories::ProjectDirs;
use log::{error, warn};
use mc_server_wrapper::Wrapper;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

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
    pretty_env_logger::init();

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

    // Get a one-time-use channel that will carry a message indicating that the
    // HTTP server should be shut down. Designed to be used by the handler for
    // the /stop route -- this way, when the Minecraft server spins down, we'll
    // stop serving new incoming requests to talk to it.
    let (shutdown_signal_tx, shutdown_signal_rx) = oneshot::channel::<()>();
    // Wrapped in an Arc<Mutex<_>> for the same reasons as the server wrapper.
    let shutdown_signal_tx_mutex = Arc::new(Mutex::new(Some(shutdown_signal_tx)));

    // Set up API route handlers.
    let routes = Router::new()
        .route(
            "/stop",
            get({
                let wrapper = Arc::clone(&wrapper);
                let shutdown_signal_tx_mutex = Arc::clone(&shutdown_signal_tx_mutex);
                move || {
                    handlers::stop_server(
                        Arc::clone(&wrapper),
                        Arc::clone(&shutdown_signal_tx_mutex),
                    )
                }
            }),
        )
        .route(
            "/list-players",
            get({
                let wrapper = Arc::clone(&wrapper);
                move || handlers::list_players(Arc::clone(&wrapper))
            }),
        )
        .route(
            "/make-world-backup",
            get({
                let wrapper = Arc::clone(&wrapper);
                move || handlers::make_world_backup(Arc::clone(&wrapper))
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
                // If a user types "/stop", we want to shut down the API server,
                // as well. Intercept "/stop" commands and treat them as a
                // special case.
                if line == "/stop" {
                    if let Err(e) = wrapper.lock().unwrap().stop_server() {
                        warn!("Something went wrong while trying to stop the Minecraft server: {}", e);
                        // Don't fail fast with process::exit() or something. If
                        // we fail to properly shut down the Minecraft server,
                        // we still want to try to shut down the API server.
                    }

                    if let Err(e) = send_api_server_shutdown_signal(shutdown_signal_tx_mutex.clone()) {
                        error!("{}", e);
                        process::exit(1);
                    }
                } else if let Err(e) = wrapper.lock().unwrap().run_custom_command(&line) {
                    warn!("Something went wrong while trying to pass a command to the wrapper's stdin: {}", e);
                }
            });
    });

    // Stand up the API server.
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
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
fn get_config() -> anyhow::Result<Config> {
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
        let mut config_file = match File::options()
            .read(true)
            .write(true)
            .open(&config_file_path)
        {
            Ok(f) => f,
            Err(e) => {
                if e.kind() == io::ErrorKind::NotFound {
                    // Create an empty config file. Later on, when we see that
                    // this file is empty, we won't overwrite any of the values
                    // in our default config instantiated above.
                    fs::create_dir_all(&config_dir).with_context(|| format!("Something went wrong while making a {:?} directory for the config file to live in", &config_dir))?;
                    // We can't use something more simple here like
                    // fs::File::create() because we need to be able to read
                    // from this file later on.
                    File::options()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .open(&config_file_path)
                        .with_context(|| {
                            format!(
                                "Something went wrong while trying to create a config file at {:?}",
                                &config_file_path
                            )
                        })?
                } else {
                    bail!(
                        "Something went wrong while trying to open the config file at {:?}",
                        &config_file_path
                    )
                }
            }
        };

        let mut config_file_contents = String::new();
        config_file
            .read_to_string(&mut config_file_contents)
            .with_context(|| format!("Failed to read the contents of {:?}", &config_file_path))?;
        if config_file_contents.is_empty() {
            // Write the default configs into that file.
            //
            // Set config_file_contents so the logic below can act like the file
            // we just read wasn't actually empty.
            config_file_contents = serde_yaml::to_string(&config)?;
            config_file
                .write_all(config_file_contents.as_bytes())
                .with_context(|| {
                    format!(
                        "Failed to write the Config below to {:?}\n{:?}",
                        &config_file_path, &config
                    )
                })?;
        }
        // Overwrite our config struct with the config file's contents.
        config = serde_yaml::from_str(&config_file_contents)?;
    }

    Ok(config)
}

/// Sends a signal to the API server to begin gracefully shutting down.
///
/// Sends an empty message along the provided [oneshot channel](tokio::sync::oneshot::channel),
/// then returns. After this message is sent, no new clients connections will be
/// established, but all existing, active connections with clients will remain
/// open until they receive the responses they're waiting on.
fn send_api_server_shutdown_signal(
    shutdown_signal_tx_mutex: Arc<Mutex<Option<oneshot::Sender<()>>>>,
) -> anyhow::Result<()> {
    match shutdown_signal_tx_mutex.lock().unwrap().take() {
        Some(tx) => {
            if let Err(e) = tx.send(()) {
                bail!("Failed to send an API server shutdown signal message along the oneshot channel: {:?}", e)
            }
        }
        None => {
            bail!("Failed to take the shutdown_signal_tx from the Option it's encased in")
        }
    }

    Ok(())
}
