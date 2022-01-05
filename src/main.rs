use std::{
    convert::Infallible,
    error,
    sync::{Arc, Mutex},
};

use mc_server_wrapper::Wrapper;
use tokio::{sync, task};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let wrapper = Arc::new(Mutex::new(Wrapper::new()?));
    // TODO: Revisit this unwrap() call. How do we properly handle this error?
    wrapper.lock().unwrap().wait_for_server_to_spin_up()?;
    // TODO: Do we need to manually unlock the mutex here? Apparently not since
    // all the code below works... idk still worth looking into.
    let (shutdown_signal_tx, shutdown_signal_rx) = sync::oneshot::channel::<()>();
    let shutdown_signal_tx_mutex = Arc::new(Mutex::new(Some(shutdown_signal_tx)));

    let stop_server = warp::path("stop")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_wrapper(wrapper.clone()))
        .and(with_shutdown_signal_tx(shutdown_signal_tx_mutex))
        .and_then(stop_server_handler);
    let list_players = warp::path("list-players")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_wrapper(wrapper))
        .and_then(list_players_handler);

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

async fn stop_server_handler(
    wrapper: Arc<Mutex<Wrapper>>,
    shutdown_signal_tx: Arc<Mutex<Option<sync::oneshot::Sender<()>>>>,
) -> Result<impl warp::Reply, Infallible> {
    match wrapper.lock().unwrap().stop_server() {
        Ok(()) => {
            // TODO: Properly handle the Result this send() call returns?
            match shutdown_signal_tx.lock().unwrap().take() {
                Some(tx) => {
                    match tx.send(()) {
                        Ok(()) => return Ok(warp::http::StatusCode::NO_CONTENT),
                        Err(()) => {
                            // TODO: Report this error to the client in the
                            // response body we send back.
                            eprintln!(
                                "after shutting down the Minecraft server, failed to send a shutdown signal to the API server"
                            );
                            return Ok(warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                        }
                    }
                }
                None => {
                    // TODO: Report this error to the client in the response
                    // body we send back.
                    eprintln!(
                        "failed to take the shutdown_signal_tx from the Option it's encased in"
                    );
                    return Ok(warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        }
        Err(e) => {
            // TODO: Revisit this error message, or even the way we're handling
            // this error in general.
            eprintln!(
                "something went wrong while trying to stop the server: {}",
                e
            );
            Ok(warp::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn list_players_handler(
    wrapper: Arc<Mutex<Wrapper>>,
) -> Result<impl warp::Reply, Infallible> {
    match wrapper.lock().unwrap().list_players() {
        Ok(players) => Ok(format!("{:?}", players)),
        Err(e) => {
            // TODO: Revisit this error message, or even the way we're handling
            // this error in general.
            eprintln!(
                "something went wrong while trying to fetch the list of players online: {}",
                e
            );
            // Ok(warp::http::StatusCode::INTERNAL_SERVER_ERROR)
            Ok(format!("{:?}", Vec::<String>::new()))
        }
    }
}
