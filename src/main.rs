mod handlers;

use std::{
    convert::Infallible,
    error,
    io::{self, BufRead, Write},
    sync::{Arc, Mutex},
    thread,
};

use mc_server_wrapper::Wrapper;
use tokio::{sync, task};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let wrapper = Arc::new(Mutex::new(Wrapper::new()?));
    wrapper.lock().unwrap().wait_for_server_to_spin_up()?;

    let (shutdown_signal_tx, shutdown_signal_rx) = sync::oneshot::channel::<()>();
    let shutdown_signal_tx_mutex = Arc::new(Mutex::new(Some(shutdown_signal_tx)));

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

    // Passing stdin to the wrapper's stdin
    let stdin_reader = io::BufReader::new(io::stdin());
    thread::spawn(move || {
        stdin_reader
            .lines()
            .filter_map(|line| line.ok())
            .for_each(|line| {
                // Add the newline char back in since the lines() call trims it
                // off.
                let cmd_with_newline_suffix = [line.as_bytes(), &vec![b'\n']].concat();
                if let Err(e) = wrapper.lock().unwrap().stdin.write_all(&cmd_with_newline_suffix) {
                    // TODO: Handle this error properly.
                    eprintln!("something went wrong while trying to pass a command to the wrapper's stdin: {}", e);
                }
            });
    });

    // API server
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
