use std::io::{BufRead, Write};
use std::sync::mpsc::{self, Receiver};
use std::{error, io, process, thread};

const SERVER_JAR_PATH: &str =
    "/Users/npc/projects/mine/mc-server-wrapper/server-playground/server.jar";

pub struct Wrapper {
    process: process::Child,
    pub stdin: process::ChildStdin,
    pub stdout: Receiver<String>,
    // TODO: Do we want to save stderr for anything?
}

impl Wrapper {
    /// Starts a Minecraft server, captures stdin so we can interact with that
    /// server while it's running, and captures the contents of stdout so we can
    /// see what that server is up to.
    ///
    /// This function spawns a separate thread which reads new lines that the
    /// server writes to stdout. When a new line comes in, it prints that line
    /// to stdout on the host for visibility, and it sends the line along a mpsc
    /// channel. The Wrapper can then pull messages from this channel if it
    /// needs to parse messages that the Minecraft server produces.
    pub fn new() -> Result<Wrapper, Box<dyn error::Error>> {
        let (stdout_tx, stdout_rx) = mpsc::channel::<String>();

        let mut process = process::Command::new("java")
            .args(&["-jar", SERVER_JAR_PATH, "nogui"])
            .stdin(process::Stdio::piped())
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;

        let stdin = process
            .stdin
            .take()
            .ok_or("could not capture stdin of the spawned process")?;

        let stdout_reader = io::BufReader::new(process.stdout.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Other,
                "could not capture stdout of the spawned process",
            )
        })?);
        // Spawn a separate thread to read the messages the Minecraft server
        // writes to stdout, and send those messages along the mpsc channel we
        // stood up earlier.
        thread::spawn(move || {
            stdout_reader
                .lines()
                .filter_map(|line| line.ok())
                // TODO: Revisit this .unwrap() call on send().
                //
                // Do we even want to handle errors here? A Q&D solution might
                // be to just drop stdout messages that fail to send.
                .for_each(|line| {
                    // Print each line for visibility.
                    println!("[Server] {}", line);
                    stdout_tx.send(line).unwrap()
                });
        });

        Ok(Wrapper {
            process,
            stdin,
            stdout: stdout_rx,
        })
    }

    pub fn wait_for_server_to_spin_up(&mut self) -> Result<(), io::Error> {
        // TODO: Revisit this .unwrap() call on recv().
        while !self.stdout.recv().unwrap().contains("Done") {
            continue;
        }
        Ok(())
    }

    pub fn stop_server(&mut self) -> io::Result<()> {
        self.stdin.write_all(b"/stop\n")?;
        let exit_status = self.process.wait()?;
        if !exit_status.success() {
            // TODO: Revisit this implementation. Perhaps have this function
            // return some new type of error that indices this happened?
            eprintln!("The Minecraft server process exited with an unsuccessful status code");
        }

        Ok(())
    }
}
