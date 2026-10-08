//! Bounded, cross-platform PTY sessions for the desktop shell.

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use vt100::Parser;

const INITIAL_ROWS: u16 = 24;
const INITIAL_COLS: u16 = 80;
const SCROLLBACK_ROWS: usize = 5_000;
const QUEUED_CHUNKS: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("terminal folder does not exist: {0}")]
    InvalidFolder(String),
    #[error("PTY error: {0}")]
    Pty(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, TerminalError>;

pub struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    receiver: Receiver<Vec<u8>>,
    parser: Parser,
    rows: u16,
    cols: u16,
    exited: Option<String>,
}

impl TerminalSession {
    pub fn spawn(folder: &Path) -> Result<Self> {
        if !folder.is_dir() {
            return Err(TerminalError::InvalidFolder(folder.display().to_string()));
        }
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: INITIAL_ROWS,
                cols: INITIAL_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| TerminalError::Pty(error.to_string()))?;
        let mut command = shell_command();
        command.cwd(folder);
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| TerminalError::Pty(error.to_string()))?;
        let master = pair.master;
        let mut reader = master
            .try_clone_reader()
            .map_err(|error| TerminalError::Pty(error.to_string()))?;
        let mut writer = master
            .take_writer()
            .map_err(|error| TerminalError::Pty(error.to_string()))?;
        let (sender, receiver) = mpsc::sync_channel(QUEUED_CHUNKS);
        thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if sender.send(buffer[..count].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        // ConPTY asks for cursor position before its first prompt on some
        // Windows shells. Responding early also covers a split initial read.
        if cfg!(windows) {
            writer.write_all(b"\x1b[1;1R")?;
            writer.flush()?;
        }
        Ok(Self {
            master,
            child,
            writer,
            receiver,
            parser: Parser::new(INITIAL_ROWS, INITIAL_COLS, SCROLLBACK_ROWS),
            rows: INITIAL_ROWS,
            cols: INITIAL_COLS,
            exited: None,
        })
    }

    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Drain at most `budget` bytes per UI frame. The reader's bounded channel
    /// applies backpressure when a command produces output faster than render.
    pub fn poll(&mut self, budget: usize) -> Result<usize> {
        let mut processed = 0;
        while processed < budget {
            match self.receiver.try_recv() {
                Ok(bytes) => {
                    processed += bytes.len();
                    self.parser.process(&bytes);
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        if self.exited.is_none()
            && let Some(status) = self.child.try_wait()?
        {
            self.exited = Some(status.to_string());
        }
        Ok(processed)
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let rows = rows.clamp(3, 100);
        let cols = cols.clamp(20, 300);
        if (rows, cols) == (self.rows, self.cols) {
            return Ok(());
        }
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| TerminalError::Pty(error.to_string()))?;
        self.parser.screen_mut().set_size(rows, cols);
        self.rows = rows;
        self.cols = cols;
        Ok(())
    }

    pub fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    pub fn contents(&self) -> String {
        self.parser.screen().contents()
    }

    pub fn scrollback(&self) -> usize {
        self.parser.screen().scrollback()
    }

    pub fn set_scrollback(&mut self, rows: usize) {
        self.parser.screen_mut().set_scrollback(rows);
    }

    pub fn exit_status(&self) -> Option<&str> {
        self.exited.as_deref()
    }

    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }

    pub fn close(&mut self) -> Result<()> {
        if self.exited.is_none() {
            if let Some(status) = self.child.try_wait()? {
                self.exited = Some(status.to_string());
            } else {
                self.child.kill()?;
                self.exited = Some(self.child.wait()?.to_string());
            }
        }
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn shell_command() -> CommandBuilder {
    if cfg!(windows) {
        let mut command = CommandBuilder::new("powershell.exe");
        command.arg("-NoLogo");
        command.arg("-NoProfile");
        command
    } else {
        let shell = std::env::var("SHELL")
            .ok()
            .filter(|shell| Path::new(shell).is_file())
            .unwrap_or_else(|| {
                if cfg!(target_os = "macos") {
                    "/bin/zsh".to_owned()
                } else {
                    "/bin/sh".to_owned()
                }
            });
        CommandBuilder::new(shell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn shell_starts_in_selected_folder_and_exits() {
        let folder = tempfile::tempdir().unwrap();
        let mut session = TerminalSession::spawn(folder.path()).unwrap();
        let marker = folder
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let command = if cfg!(windows) {
            "Write-Output ((Get-Location).Path + ('|PIPE' + 'LINE_READY'))\r"
        } else {
            "printf '%s|PIPELINE_READY\\n' \"$PWD\"\n"
        };
        session.send(command.as_bytes()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let expected = format!("{marker}|PIPELINE_READY");
        while Instant::now() < deadline && !session.contents().contains(&expected) {
            session.poll(64 * 1024).unwrap();
            thread::sleep(Duration::from_millis(25));
        }
        assert!(
            session.contents().contains(&expected),
            "{}",
            session.contents()
        );
        session.resize(30, 100).unwrap();
        assert_eq!(session.size(), (30, 100));
        session.send(b"exit\r").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline && session.exit_status().is_none() {
            session.poll(64 * 1024).unwrap();
            thread::sleep(Duration::from_millis(25));
        }
        assert!(session.exit_status().is_some(), "{}", session.contents());
    }

    #[test]
    fn parser_handles_ansi_unicode_and_scrollback() {
        let mut parser = Parser::new(4, 20, 10);
        parser.process("\x1b[31mRED\x1b[0m λ".as_bytes());
        assert!(parser.screen().contents().contains("RED λ"));
        for index in 0..15 {
            parser.process(format!("line-{index}\r\n").as_bytes());
        }
        parser.screen_mut().set_scrollback(5);
        assert!(parser.screen().scrollback() > 0);
    }

    #[test]
    fn noisy_command_is_drained_in_bounded_chunks_and_close_reaps_shell() {
        let folder = tempfile::tempdir().unwrap();
        let mut session = TerminalSession::spawn(folder.path()).unwrap();
        let command = if cfg!(windows) {
            "1..2000 | ForEach-Object { Write-Output ('line-' + $_) }; Write-Output ('PIPE' + 'LINE_DONE')\r"
        } else {
            "i=0; while [ $i -lt 2000 ]; do echo \"line-$i\"; i=$((i+1)); done; echo PIPE'LINE_DONE'\n"
        };
        session.send(command.as_bytes()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline && !session.contents().contains("PIPELINE_DONE") {
            let processed = session.poll(16 * 1024).unwrap();
            assert!(processed <= 16 * 1024 + 4096);
            thread::sleep(Duration::from_millis(15));
        }
        assert!(
            session.contents().contains("PIPELINE_DONE"),
            "{}",
            session.contents()
        );
        session.close().unwrap();
        assert!(session.exit_status().is_some());
    }
}
