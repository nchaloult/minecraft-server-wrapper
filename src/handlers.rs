use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use mc_server_wrapper::Wrapper;
use tokio::sync;
use warp::http::StatusCode;
use warp::reply;

pub(crate) async fn stop_server(
    wrapper: Arc<Mutex<Wrapper>>,
    shutdown_signal_tx: Arc<Mutex<Option<sync::oneshot::Sender<()>>>>,
) -> Result<impl warp::Reply, Infallible> {
    match wrapper.lock().unwrap().stop_server() {
        Ok(()) => {
            // TODO: Properly handle the Result this send() call returns?
            match shutdown_signal_tx.lock().unwrap().take() {
                Some(tx) => {
                    match tx.send(()) {
                        Ok(()) => Ok(reply::with_status("NO_CONTENT", StatusCode::NO_CONTENT)),
                        Err(()) => {
                            // TODO: Report this error to the client in the
                            // response body we send back.
                            eprintln!(
                                "after shutting down the Minecraft server, failed to send a shutdown signal to the API server"
                            );
                            Ok(reply::with_status(
                                "INTERNAL_SERVER_ERROR",
                                StatusCode::INTERNAL_SERVER_ERROR,
                            ))
                        }
                    }
                }
                None => {
                    // TODO: Report this error to the client in the response
                    // body we send back.
                    eprintln!(
                        "failed to take the shutdown_signal_tx from the Option it's encased in"
                    );
                    Ok(reply::with_status(
                        "INTERNAL_SERVER_ERROR",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    ))
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
            Ok(reply::with_status(
                "INTERNAL_SERVER_ERROR",
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub(crate) async fn list_players(
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
            // Ok(reply::with_status(
            //     "INTERNAL_SERVER_ERROR",
            //     StatusCode::INTERNAL_SERVER_ERROR,
            // ))
            Ok(format!("{:?}", Vec::<String>::new()))
        }
    }
}
