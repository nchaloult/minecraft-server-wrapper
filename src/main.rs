use std::{
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

    let stop_server = warp::path("stop").and(warp::get()).map({
        let wrapper = wrapper.clone();
        move || {
            // TODO: Revisit these unwrap() calls.
            wrapper.lock().unwrap().stop_server().unwrap();
            warp::http::StatusCode::NO_CONTENT
        }
    });
    let list_players = warp::path("list-players").and(warp::get()).map({
        let wrapper = wrapper.clone();
        move || {
            // TODO: Revisit these unwrap() calls.
            let players = wrapper.lock().unwrap().list_players().unwrap();
            format!("{:?}", players)
        }
    });

    let routes = stop_server.or(list_players);
    warp::serve(routes).run(([0, 0, 0, 0], 6969)).await;
    Ok(())
}
