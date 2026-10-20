//! Running a confirmed command: inline with its output streamed, or in a terminal window.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};

pub struct Run {
    child: Child,
    rx: Receiver<String>,
    pub exit: Option<i32>,
}

impl Run {
    /// Starts the program without a shell; stdout and stderr lines arrive through `poll`.
    pub fn start(program: &Path, args: &[String]) -> Result<Run, String> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", program.display()))?;
        let (tx, rx) = channel();
        if let Some(out) = child.stdout.take() {
            pump(out, tx.clone());
        }
        if let Some(err) = child.stderr.take() {
            pump(err, tx);
        }
        Ok(Run { child, rx, exit: None })
    }

    /// The lines that arrived since the last call; sets `exit` once the process has ended.
    pub fn poll(&mut self) -> Vec<String> {
        let lines: Vec<String> = self.rx.try_iter().collect();
        if self.exit.is_none() {
            if let Ok(Some(status)) = self.child.try_wait() {
                self.exit = Some(status.code().unwrap_or(-1));
            }
        }
        lines
    }

    pub fn running(&self) -> bool {
        self.exit.is_none()
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.exit = Some(-1);
    }
}

fn pump<R: std::io::Read + Send + 'static>(reader: R, tx: Sender<String>) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
}

/// Runs the command line in a terminal window that stays open until Enter is pressed.
pub fn in_terminal(command_line: &str) -> Result<(), String> {
    let script = format!("{command_line}; echo; read -p \"exit $? - press Enter\"");
    Command::new("omarchy-launch-terminal")
        .args(["bash", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("omarchy-launch-terminal: {e}"))
}
