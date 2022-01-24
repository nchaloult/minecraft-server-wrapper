use std::{
    error,
    io::{self, BufRead, Write},
    process,
    sync::mpsc::{self, Receiver},
    thread,
};

use anyhow::{bail, Context};

pub struct Wrapper {
    process: process::Child,
    stdin: process::ChildStdin,
    stdout: Receiver<String>,
    // TODO: Do we want to save stderr for anything?
}

impl Wrapper {
    pub fn new(
        max_memory_buffer_size: u16,
        server_jar_path: &str,
    ) -> Result<Wrapper, Box<dyn error::Error>> {
        let (process, stdin, stdout_rx) =
            spawn_server_process(max_memory_buffer_size, server_jar_path)?;
        Ok(Wrapper {
            process,
            stdin,
            stdout: stdout_rx,
        })
    }

    pub fn wait_for_server_to_spin_up(&mut self) {
        // TODO: Implement timeout functionality? What if something goes wrong
        // with the underlying server and it just hangs?

        // When the Minecraft server finishes spinning up, it will send a
        // message to stdout that looks something like this:
        // [02:00:14] [Server thread/INFO]: Done (9.797s)! For help, type "help"
        //
        // TODO: Revisit this .unwrap() call on recv().
        while !self.stdout.recv().unwrap().contains("Done") {
            continue;
        }
    }

    /// Returns the names of players who are currently logged in and playing on
    /// the server.
    pub fn list_players(&mut self) -> anyhow::Result<Vec<String>> {
        self.run_custom_command("/list").with_context(|| {
            "Something went wrong while sending the Minecraft server the \"/list\" command"
        })?;
        // Will look something like this:
        // [16:14:22] [Server thread/INFO]: There are 2 of a max of 20 players online: player1, player2
        let response = self.stdout.recv().unwrap();

        // Strip away everything but the list of players.
        //
        // Should be safe to unwrap() after the rsplit_one() call since we know
        // in advance what the contents of response will look like.
        let (_, players_as_str) = response.rsplit_once(": ").unwrap();
        if players_as_str.is_empty() {
            return Ok(Vec::new());
        }

        let players_as_vec = players_as_str
            .split(',')
            .map(|name| name.to_owned())
            .collect();
        Ok(players_as_vec)
    }

    pub fn stop_server(&mut self) -> anyhow::Result<()> {
        self.run_custom_command("/stop").with_context(|| {
            "Something went wrong while sending the Minecraft server the \"/stop\" command"
        })?;
        let exit_status = self
            .process
            .wait()
            .with_context(|| "Failed to wait for the Minecraft server process to exit")?;
        if !exit_status.success() {
            match exit_status.code() {
                Some(code) => bail!(
                    "The Minecraft server process exited with status code {}",
                    code
                ),
                None => bail!("The Minecraft server process was terminated forcefully by a signal"),
            }
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
            if let Err(e) = self.stdout.try_recv() {
                match e {
                    mpsc::TryRecvError::Empty => return Ok(()),
                    mpsc::TryRecvError::Disconnected => {
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            // TODO: Improve error message?
                            "The stdout channel was closed unexpectedly",
                        ));
                    }
                }
            }
        }
    }
}

/// Starts a Minecraft server, captures stdin so we can interact with that
/// server while it's running, and captures the contents of stdout so we can see
/// what that server is up to.
///
/// This function spawns a separate thread which reads new lines that the server
/// writes to stdout. When a new line comes in, it prints that line to stdout on
/// the host for visibility, and it sends the line along a mpsc channel. Some
/// consumer can then pull messages from this channel if it needs to parse
/// messages that the Minecraft server produces.
fn spawn_server_process(
    max_memory_buffer_size: u16,
    server_jar_path: &str,
) -> anyhow::Result<(process::Child, process::ChildStdin, Receiver<String>)> {
    let (stdout_tx, stdout_rx) = mpsc::channel::<String>();

    let mut process = process::Command::new("java")
        .args(&[
            // Just in case...
            // https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2021-44832
            // https://twitter.com/slicedlime/status/1469164192389287939
            "-Dlog4j2.formatMsgNoLookups=true",
            &format!("-Xmx{}m", max_memory_buffer_size),
            "-jar",
            server_jar_path,
            "nogui",
        ])
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::piped())
        .spawn()?;

    let stdin = process
        .stdin
        .take()
        .with_context(|| "Failed to capture stdin of the newly-spawned Minecraft server process")?;

    let stdout_reader = io::BufReader::new(process.stdout.take().with_context(|| {
        "Failed to capture stdout of the newly-spawned Minecraft server process"
    })?);
    // Spawn a separate thread to read the messages the Minecraft server
    // writes to stdout, and send those messages along the mpsc channel we
    // were given.
    thread::spawn(move || {
        stdout_reader
            .lines()
            .filter_map(|line| line.ok())
            .for_each(|line| {
                // Print each line for visibility.
                println!("{}", line);
                // TODO: Revisit this .unwrap() call on send().
                //
                // Do we even want to handle errors here? A Q&D solution
                // might be to just drop stdout messages that fail to send.
                stdout_tx.send(line).unwrap()
            });
    });

    Ok((process, stdin, stdout_rx))
}
