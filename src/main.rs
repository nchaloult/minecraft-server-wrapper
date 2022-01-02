use std::error;
use std::io::{BufRead, Write};

use mc_server_wrapper::Wrapper;

fn main() -> Result<(), Box<dyn error::Error>> {
    let mut wrapper = Wrapper::new()?;
    wrapper.wait_for_server_to_spin_up()?;

    wrapper.stdin.write_all(b"/stop\n")?;
    println!("Just sent the stop command");

    wrapper
        .stdout
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| println!("{}", line));

    Ok(())
}
