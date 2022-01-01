use std::error;
use std::io::{BufRead, BufReader, Error, ErrorKind, Write};
use std::process::{Command, Stdio};

const SERVER_JAR_PATH: &str =
    "/Users/npc/projects/mine/mc-server-wrapper/server-playground/server.jar";

fn main() -> Result<(), Box<dyn error::Error>> {
    let mut child = Command::new("java")
        .args(&["-jar", SERVER_JAR_PATH, "nogui"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let mut stdout = BufReader::new(
        child
            .stdout
            .ok_or_else(|| Error::new(ErrorKind::Other, "could not capture stdout"))?,
    );
    let mut buf = String::new();

    while !buf.contains("Done") {
        print!("{}", &buf);
        buf.clear();
        stdout.read_line(&mut buf)?;
    }
    // Print and clear the buffer one last time. At this point, its content are
    // the "Done" line.
    print!("{}", &buf);
    buf.clear();

    println!("Reached the point where the server is up");

    child
        .stdin
        .as_mut()
        .ok_or("could not capture stdin")?
        .write_all(b"/stop\n")?;

    println!("Just sent the stop command");

    stdout
        .lines()
        .filter_map(|line| line.ok())
        .for_each(|line| println!("{}", line));

    Ok(())
}
