use std::error;

use mc_server_wrapper::Wrapper;

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let mut wrapper = Wrapper::new()?;
    wrapper.wait_for_server_to_spin_up()?;

    let players = wrapper.list_players()?;
    println!("players online: {:?}", players);

    wrapper.stop_server()?;

    Ok(())
}
