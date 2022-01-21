use std::sync::{Arc, Mutex};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use mc_server_wrapper::Wrapper;
use tokio::sync::oneshot;

pub(crate) async fn stop_server(
    wrapper: Arc<Mutex<Wrapper>>,
    shutdown_signal_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
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
                    "Failed to send an API shutdown signal message along the oneshot channel";
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
