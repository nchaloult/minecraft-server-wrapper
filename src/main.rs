use std::error;

use mc_server_wrapper::Wrapper;

fn main() -> Result<(), Box<dyn error::Error>> {
    let mut wrapper = Wrapper::new()?;
    wrapper.wait_for_server_to_spin_up()?;

    wrapper.stop_server()?;

    Ok(())
}
