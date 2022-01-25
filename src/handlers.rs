use std::sync::{Arc, Mutex};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use log::{info, warn};
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

pub(crate) async fn make_world_backup(wrapper: Arc<Mutex<Wrapper>>) -> Result<String, Response> {
    let mut w = wrapper.lock().unwrap();
    match w.make_world_backup() {
        Ok(tarball_path) => {
            let response_msg = format!(
                "Created a new world backup: {}",
                // TODO: Revisit unwrap() call here.
                //
                // This func is already pretty verbose... not sure if the extra
                // complexity is worth it.
                tarball_path.into_os_string().into_string().unwrap()
            );
            info!("{}", &response_msg);
            Ok(response_msg)
        }
        Err(e) => {
            let mut err_msg = format!(
                "Something went wrong while trying to make a server backup: {}",
                e
            );
            // Try to restart the Minecraft server again before building a
            // Response.
            match w.restart_server() {
                Ok(()) => {
                    warn!("GET /make-world-backup: {}", &err_msg);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response())
                }
                Err(e) => {
                    let err_msg_addendum = format!("\nAfter failing to make that backup, something went wrong while trying to restart the Minecraft server: {}", e);
                    err_msg.push_str(&err_msg_addendum);
                    warn!("GET /make-world-backup: {}", &err_msg);
                    Err((StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response())
                }
            }
        }
    }
}
