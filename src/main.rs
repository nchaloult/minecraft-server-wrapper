use std::{
    convert::Infallible,
    error,
    sync::{Arc, Mutex},
};

use mc_server_wrapper::Wrapper;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let wrapper = Arc::new(Mutex::new(Wrapper::new()?));
    // TODO: Revisit this unwrap() call. How do we properly handle this error?
    wrapper.lock().unwrap().wait_for_server_to_spin_up()?;
    // TODO: Do we need to manually unlock the mutex here? Apparently not since
    // all the code below works... idk still worth looking into.

    let stop_server = warp::path("stop")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_wrapper(wrapper.clone()))
        .and_then(stop_server_handler);
    let list_players = warp::path("list-players")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_wrapper(wrapper))
        .and_then(list_players_handler);

    let routes = stop_server.or(list_players);
    warp::serve(routes).run(([0, 0, 0, 0], 6969)).await;
    Ok(())
}

fn with_wrapper(
    wrapper: Arc<Mutex<Wrapper>>,
) -> impl Filter<Extract = (Arc<Mutex<Wrapper>>,), Error = Infallible> + Clone {
    warp::any().map(move || wrapper.clone())
}

async fn stop_server_handler(wrapper: Arc<Mutex<Wrapper>>) -> Result<impl warp::Reply, Infallible> {
    match wrapper.lock().unwrap().stop_server() {
        Ok(()) => Ok(warp::http::StatusCode::NO_CONTENT),
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
