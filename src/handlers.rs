use std::convert::Infallible;
use std::sync::{Arc, Mutex};

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use mc_server_wrapper::Wrapper;
use tokio::sync;
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

pub(crate) async fn axum_stop_server(
    wrapper: Arc<Mutex<Wrapper>>,
    shutdown_signal_tx: Arc<Mutex<Option<sync::oneshot::Sender<()>>>>,
) -> Result<StatusCode, Response> {
    if let Err(e) = wrapper.lock().unwrap().stop_server() {
        let err_msg = format!(
            "Something went wrong while trying to stop the server: {}",
            e
        );
        eprintln!("{}", &err_msg);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response());
    }

    match shutdown_signal_tx.lock().unwrap().take() {
        Some(tx) => {
            if tx.send(()).is_err() {
                let err_msg =
                    "Failed to take the shutdown_signal_tx from the Option it's encased in";
                eprintln!("{}", err_msg);
                return Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response());
            }
        }
        None => {
            let err_msg = "Failed to take the shutdown_signal_tx from the Option it's encased in";
            eprintln!("{}", err_msg);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response());
        }
    }

    Ok(StatusCode::NO_CONTENT)
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

pub(crate) async fn axum_list_players(
    wrapper: Arc<Mutex<Wrapper>>,
) -> Result<Json<Vec<String>>, Response> {
    match wrapper.lock().unwrap().list_players() {
        Ok(players) => Ok(players.into()),
        Err(e) => {
            let err_msg = format!(
                "Something went wrong while trying to fetch the list of players online: {}",
                e
            );
            eprintln!("{}", err_msg);
            Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response())
        }
    }
}
