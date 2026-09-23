//! PTY session driver: spawn shells, pump bytes, resize, teardown.
//!
//! This module moves bytes between a child process and the terminal. It never
//! parses them — parsing is the VT layer in [`crate::terminal`]. The split is
//! deliberate: the driver is testable with a fake command (`echo`), and the
//! [`crate::terminal::Terminal`] wrapper is testable with recorded byte
//! fixtures, with no coupling between the two.
//!
//! Backend: [`portable_pty`], whose native system is ConPTY on Windows and
//! `openpty` on macOS/Linux. There is exactly one PTY implementation in play;
//! no `winpty` fallback, no per-platform crate switch.
//!
//! Threading contract (matches `docs/terminal-component.md`): the app owns the
//! read loop. Take a reader via [`PtySession::clone_reader`], hand it to
//! [`pump_to_channel`] on a background thread, and feed the received chunks to
//! the terminal on the main thread. Terminal types are `!Send`, so the pump
//! thread must never touch them.

#[cfg(test)]
mod tests;

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

pub use portable_pty::ExitStatus;

/// How long [`PtySession::wait_for_exit`] sleeps between polls.
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Chunk size for [`pump_to_channel`] reads.
const PUMP_CHUNK_SIZE: usize = 8192;

/// Configuration for spawning a [`PtySession`].
pub struct PtyConfig {
    /// Program to run. `None` means the platform default shell
    /// (`COMSPEC` on Windows, `$SHELL` or `/bin/sh` elsewhere).
    pub shell: Option<String>,
    /// Arguments to the program (excluding argv[0]).
    pub args: Vec<String>,
    /// Initial viewport width in cells.
    pub cols: u16,
    /// Initial viewport height in cells.
    pub rows: u16,
    /// Working directory for the child. `None` inherits ours.
    pub cwd: Option<PathBuf>,
}

impl PtyConfig {
    /// An interactive shell with the given viewport size.
    #[must_use]
    pub fn shell(cols: u16, rows: u16) -> Self {
        Self {
            shell: None,
            args: Vec::new(),
            cols,
            rows,
            cwd: None,
        }
    }

    /// A one-shot command with the given viewport size.
    #[must_use]
    pub fn command(program: &str, args: &[&str], cols: u16, rows: u16) -> Self {
        Self {
            shell: Some(program.to_string()),
            args: args.iter().map(ToString::to_string).collect(),
            cols,
            rows,
            cwd: None,
        }
    }
}

/// A live child process attached to a PTY.
///
/// Owns the master end and the child handle. Dropping the session kills the
/// child as a backstop, but prefer explicit teardown: [`PtySession::kill`]
/// then [`PtySession::wait_for_exit`], which reaps promptly instead of
/// leaving a zombie until this process exits.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    cols: u16,
    rows: u16,
}

impl PtySession {
    /// Spawn a child per `config` and attach it to a fresh PTY.
    pub fn spawn(config: &PtyConfig) -> std::io::Result<Self> {
        let system = native_pty_system();
        let pair = system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io_err)?;
        let mut builder = match &config.shell {
            Some(program) => CommandBuilder::new(program),
            None => CommandBuilder::new_default_prog(),
        };
        builder.args(&config.args);
        if let Some(cwd) = &config.cwd {
            builder.cwd(cwd);
        }
        let child = pair.slave.spawn_command(builder).map_err(io_err)?;
        let writer = pair.master.take_writer().map_err(io_err)?;
        Ok(Self {
            master: pair.master,
            writer,
            child,
            cols: config.cols,
            rows: config.rows,
        })
    }

    /// Current viewport size in cells, as `(cols, rows)`.
    #[must_use]
    pub fn dims(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Resize the PTY and record the new dimensions.
    ///
    /// The caller must also resize the [`crate::terminal::Terminal`] and,
    /// per the component contract, forward the size to whatever owns the
    /// layout — this method only informs the kernel (and the child).
    pub fn resize(&mut self, cols: u16, rows: u16) -> std::io::Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(io_err)?;
        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    /// Kernel-known PTY size. Must agree with [`PtySession::dims`].
    pub fn kernel_size(&self) -> std::io::Result<(u16, u16)> {
        let size = self.master.get_size().map_err(io_err)?;
        Ok((size.cols, size.rows))
    }

    /// An independent reader for the child's output. Clone as needed; each
    /// clone shares the same underlying stream.
    pub fn clone_reader(&self) -> std::io::Result<Box<dyn Read + Send>> {
        self.master.try_clone_reader().map_err(io_err)
    }

    /// Send input (keystrokes, paste payloads) to the child.
    pub fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(bytes)?;
        self.writer.flush()
    }

    /// `true` while the child has not yet exited.
    pub fn is_alive(&mut self) -> std::io::Result<bool> {
        Ok(self.child.try_wait().map_err(io_err)?.is_none())
    }

    /// Block until the child exits, yielding its status.
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child.wait().map_err(io_err)
    }

    /// Poll until the child exits or `timeout` elapses.
    ///
    /// Returns `Ok(None)` on timeout without touching the child.
    pub fn wait_for_exit(&mut self, timeout: Duration) -> std::io::Result<Option<ExitStatus>> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().map_err(io_err)? {
                return Ok(Some(status));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(EXIT_POLL_INTERVAL);
        }
    }

    /// Terminate the child. Idempotent-ish: errors (e.g. already exited)
    /// propagate so tests can assert on them; [`Drop`] ignores them.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill().map_err(io_err)
    }

    /// OS process id of the child, if the backend reports one.
    #[must_use]
    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        // Backstop only: explicit kill + wait_for_exit reaps promptly.
        let _ = self.child.kill();
    }
}

/// Forward everything from `reader` to `tx` until EOF or error.
///
/// Blocks, so run it on its own thread. Dropping the receiver ends delivery
/// but not the pump — the thread exits on the next chunk or EOF. The terminal
/// itself is never touched here: the app feeds received chunks to it on the
/// main thread (`!Send` stays on one thread).
///
/// ConPTY note: EOF does not arrive while the master end is open, so a
/// `read_until_eof` shape would block forever on Windows. Always pair this
/// with child-exit polling ([`PtySession::wait_for_exit`]) and stop
/// consuming once the child is gone; the thread dies with the session drop.
pub fn pump_to_channel(mut reader: Box<dyn Read + Send>, tx: Sender<Vec<u8>>) {
    let mut buf = [0u8; PUMP_CHUNK_SIZE];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

fn io_err(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}
