mod handlers;

use std::{
    convert::Infallible,
    error,
    io::{self, BufRead},
    sync::{Arc, Mutex},
    thread,
};

use mc_server_wrapper::Wrapper;
use tokio::{sync, task};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    // Get a new server wrapper, and wait for that wrapper to launch the
    // underlying Minecraft server.
    //
    // The wrapper is guarded by a mutex because it's not designed to be used
    // asynchronously, but a shared reference of it is given to each HTTP API
    // handler.
    //
    // That whole thing is wrapped in an Arc so we can share ownership of that
    // mutex across multiple async tasks, and consequently multiple threads.
    let wrapper = Arc::new(Mutex::new(Wrapper::new()?));
    wrapper.lock().unwrap().wait_for_server_to_spin_up()?;

    // Get a one-time-use channel that will carry a message indicating that the
    // warp server should be shut down. Designed to be used by the handler for
    // the /stop route -- this way, when the Minecraft server spins down, we'll
    // stop serving new incoming requests to talk to it.
    let (shutdown_signal_tx, shutdown_signal_rx) = sync::oneshot::channel::<()>();
    // Wrapped in an Arc<Mutex<_>> for the same reasons as the server wrapper.
    let shutdown_signal_tx_mutex = Arc::new(Mutex::new(Some(shutdown_signal_tx)));

    // Route filters.
    let stop_server = warp::path("stop")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_wrapper(wrapper.clone()))
        .and(with_shutdown_signal_tx(shutdown_signal_tx_mutex))
        .and_then(handlers::stop_server);
    let list_players = warp::path("list-players")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_wrapper(wrapper.clone()))
        .and_then(handlers::list_players);

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
    let routes = stop_server.or(list_players);
    let (_, server) =
        warp::serve(routes).bind_with_graceful_shutdown(([0, 0, 0, 0], 6969), async {
            shutdown_signal_rx.await.ok();
        });
    task::spawn(server).await.unwrap();

    Ok(())
}

fn with_wrapper(
    wrapper: Arc<Mutex<Wrapper>>,
) -> impl Filter<Extract = (Arc<Mutex<Wrapper>>,), Error = Infallible> + Clone {
    warp::any().map(move || wrapper.clone())
}

fn with_shutdown_signal_tx(
    shutdown_signal_tx: Arc<Mutex<Option<sync::oneshot::Sender<()>>>>,
) -> impl Filter<Extract = (Arc<Mutex<Option<sync::oneshot::Sender<()>>>>,), Error = Infallible> + Clone
{
    warp::any().map(move || shutdown_signal_tx.clone())
}
