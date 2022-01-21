use std::sync::{Arc, Mutex};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use log::warn;
use mc_server_wrapper::Wrapper;
use tokio::sync::oneshot;

use crate::send_api_server_shutdown_signal;

pub(crate) async fn stop_server(
    wrapper: Arc<Mutex<Wrapper>>,
    shutdown_signal_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
) -> Result<StatusCode, Response> {
    if let Err(e) = wrapper.lock().unwrap().stop_server() {
        let err_msg = format!(
            "Something went wrong while trying to stop the server: {}",
            e
        );
        warn!("GET /stop: {}", &err_msg);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response());
    }

    if let Err(e) = send_api_server_shutdown_signal(shutdown_signal_tx) {
        let err_msg = format!(
            "Something went wrong while trying to stop the API server: {}",
            e
        );
        warn!("GET /stop: {}", err_msg);
        return Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response());
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
            warn!("GET /list-players: {}", err_msg);
            Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response())
        }
    }
}
