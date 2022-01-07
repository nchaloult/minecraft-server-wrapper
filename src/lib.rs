use std::io::{BufRead, Write};
use std::sync::mpsc::{self, Receiver};
use std::{error, io, process, thread};

const SERVER_JAR_PATH: &str =
    "/Users/npc/projects/mine/mc-server-wrapper/server-playground/server.jar";

pub struct Wrapper {
    process: process::Child,
    stdin: process::ChildStdin,
    stdout: Receiver<String>,
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

    /// Returns the names of players who are currently logged in and playing on
    /// the server.
    pub fn list_players(&mut self) -> io::Result<Vec<String>> {
        self.run_custom_command("/list")?;
        // Will look something like this:
        // [16:14:22] [Server thread/INFO]: There are 2 of a max of 20 players online: player1, player2
        let response = self.stdout.recv().unwrap();

        // Strip away everything but the list of players.
        //
        // Should be safe to unwrap() after the rsplit_one() call since we know
        // in advance what the contents of response will look like.
        let (_, players_as_str) = response.rsplit_once(": ").unwrap();
        if players_as_str.len() == 0 {
            return Ok(Vec::new());
        }

        let players_as_vec = players_as_str
            .split(",")
            .map(|name| name.to_owned())
            .collect();
        Ok(players_as_vec)
    }

    pub fn stop_server(&mut self) -> io::Result<()> {
        self.run_custom_command("/stop")?;
        let exit_status = self.process.wait()?;
        if !exit_status.success() {
            // TODO: Revisit this implementation. Perhaps have this function
            // return some new type of error that indices this happened?
            eprintln!("The Minecraft server process exited with an unsuccessful status code");
        }

        Ok(())
    }

    /// Gives the Minecraft server the provided custom command. This function
    /// immediately returns after the command is run; it doesn't watch stdout
    /// or wait to see what the result of that command is.
    ///
    /// The provided `cmd` string doesn't need a trailing newline `\n`
    /// character.
    pub fn run_custom_command(&mut self, cmd: &str) -> io::Result<()> {
        self.disregard_irrelevant_stdout_contents()?;

        // Make sure the command is suffixed with a newline char. This is
        // necessary because the Minecraft server waits until a newline char
        // comes through on stdin before attempting to parse stdin's contents as
        // a command.
        let cmd_with_newline = if cmd.ends_with('\n') {
            cmd.to_owned()
        } else {
            format!("{}\n", cmd)
        };

        self.stdin.write_all(cmd_with_newline.as_bytes())?;
        Ok(())
    }

    /// Reads all the lines written to stdout that haven't been processed yet,
    /// and discards them.
    ///
    /// Sometimes, the Minecraft server will write logs to stdout on its own,
    /// like when a player dies. This wrapper is only concerned with monitoring
    /// stdout after the user invokes a command, like asking for a list of
    /// players who are currently online. Since stdout is buffered, we need to
    /// drain that buffer of all messages irrelevant to us before we run a
    /// command against the server.
    fn disregard_irrelevant_stdout_contents(&mut self) -> io::Result<()> {
        loop {
            match self.stdout.try_recv() {
                Ok(_) => continue,
                Err(e) => match e {
                    mpsc::TryRecvError::Empty => return Ok(()),
                    mpsc::TryRecvError::Disconnected => {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            // TODO: Improve error message?
                            "the stdout channel was closed unexpectedly",
                        ));
                    }
                },
            }
        }
    }
}
