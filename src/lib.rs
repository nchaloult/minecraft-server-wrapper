use std::io::BufRead;
use std::{error, io, process};

const SERVER_JAR_PATH: &str =
    "/Users/npc/projects/mine/mc-server-wrapper/server-playground/server.jar";

pub struct Wrapper {
    process: process::Child,
    pub stdin: process::ChildStdin,
    pub stdout: io::BufReader<process::ChildStdout>,
    // TODO: Do we want to save stderr for anything?
}

impl Wrapper {
    pub fn new() -> Result<Wrapper, Box<dyn error::Error>> {
        let mut process = process::Command::new("java")
            .args(&["-jar", SERVER_JAR_PATH, "nogui"])
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;

        let stdout = io::BufReader::new(process.stdout.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "could not capture stdout of the spawned process",
            )
        })?);
        let stdin = process
            .stdin
            .take()
            .ok_or("could not capture stdin of the spawned process")?;

        Ok(Wrapper {
            process,
            stdin,
            stdout,
        })
    }

    pub fn wait_for_server_to_spin_up(&mut self) -> Result<(), io::Error> {
        let mut buf = String::new();
        while !buf.contains("Done") {
            // TODO: Temporary.
            print!("{}", &buf);
            buf.clear();
            self.stdout.read_line(&mut buf)?;
        }
        // Print the buffer one last time. At this point, its contents are the
        // "Done" line.
        print!("{}", &buf);

        Ok(())
    }
}
