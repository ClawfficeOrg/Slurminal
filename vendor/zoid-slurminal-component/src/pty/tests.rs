//! Integration tests for the PTY driver.
//!
//! The driver moves bytes and never parses them, so each test pairs it with
//! the real VT layer: spawn a command, pump its output into a [`Terminal`],
//! and assert on the grid. A fake byte source is unnecessary — `echo` is the
//! fake command.
//!
//! Windows note: ConPTY does not deliver read-EOF while the master end is
//! open, so tests never read to EOF. They pump on a detached thread, poll the
//! child for exit, drain briefly, and assert. Blocking reads only happen on
//! the pump thread, which dies when the session (and its pipes) drops.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{PtyConfig, PtySession, pump_to_channel};
use crate::terminal::{Terminal, TerminalConfig};

/// One-shot command printing `marker` and exiting, per platform.
fn echo_config(marker: &str) -> PtyConfig {
    #[cfg(windows)]
    {
        PtyConfig::command("cmd.exe", &["/C", &format!("echo {marker}")], 80, 24)
    }
    #[cfg(not(windows))]
    {
        PtyConfig::command("sh", &["-c", &format!("echo {marker}")], 80, 24)
    }
}

fn other(message: &str) -> std::io::Error {
    std::io::Error::other(message.to_string())
}

/// Collect pumped output until the child exits, plus a short drain.
///
/// Bounded by `timeout`; the pump thread is detached and dies with the
/// session drop at the end of the calling test.
fn collect_until_exit(session: &mut PtySession, timeout: Duration) -> std::io::Result<Vec<u8>> {
    let reader = session.clone_reader()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(|| pump_to_channel(reader, tx));
    let mut out = Vec::new();
    let deadline = Instant::now() + timeout;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => out.extend_from_slice(&chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !session.is_alive()? {
                    while let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
                        out.extend_from_slice(&chunk);
                    }
                    return Ok(out);
                }
                if Instant::now() >= deadline {
                    return Err(other("timed out collecting pty output"));
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(out),
        }
    }
}

#[test]
fn echo_output_reaches_terminal_grid() -> std::io::Result<()> {
    let mut session = PtySession::spawn(&echo_config("slurminal-pty-probe"))?;
    let bytes = collect_until_exit(&mut session, Duration::from_secs(15))?;

    let mut term = Terminal::new(TerminalConfig::default()).map_err(|e| other(&e.to_string()))?;
    term.feed(&bytes);
    let text = term.visible_text().map_err(|e| other(&e.to_string()))?;
    assert!(
        text.contains("slurminal-pty-probe"),
        "pty output never reached the grid: {text:?}"
    );
    let status = session
        .wait_for_exit(Duration::from_secs(15))?
        .ok_or_else(|| other("echo child did not exit"))?;
    assert!(status.success(), "echo child failed: {status:?}");
    Ok(())
}

#[test]
fn session_reports_process_id_and_exit() -> std::io::Result<()> {
    let mut session = PtySession::spawn(&echo_config("pid-probe"))?;
    assert!(
        session.process_id().is_some(),
        "spawned child has no process id"
    );
    assert!(session.is_alive()?);
    let status = session
        .wait_for_exit(Duration::from_secs(15))?
        .ok_or_else(|| other("echo child did not exit in 15s"))?;
    assert!(status.success(), "echo child failed: {status:?}");
    assert!(!session.is_alive()?);
    Ok(())
}

#[test]
fn resize_reaches_kernel_and_tracks_dims() -> std::io::Result<()> {
    let mut session = PtySession::spawn(&PtyConfig::shell(80, 24))?;
    assert_eq!(session.dims(), (80, 24));
    session.resize(100, 30)?;
    assert_eq!(session.dims(), (100, 30));
    assert_eq!(session.kernel_size()?, (100, 30));

    // Mirror into the VT layer the way the app does, then reap: no orphans.
    let mut term = Terminal::new(TerminalConfig::default()).map_err(|e| other(&e.to_string()))?;
    term.resize(100, 30, 0, 0)
        .map_err(|e| other(&e.to_string()))?;
    assert_eq!(term.cols().map_err(|e| other(&e.to_string()))?, 100);
    session.kill()?;
    let status = session
        .wait_for_exit(Duration::from_secs(15))?
        .ok_or_else(|| other("shell survived kill"))?;
    let _ = status;
    assert!(!session.is_alive()?);
    Ok(())
}

#[test]
fn pump_thread_delivers_chunks_to_channel() -> std::io::Result<()> {
    let mut session = PtySession::spawn(&echo_config("pump-probe"))?;
    let collected = collect_until_exit(&mut session, Duration::from_secs(15))?;
    let text = String::from_utf8_lossy(&collected);
    assert!(
        text.contains("pump-probe"),
        "pump thread lost output: {text:?}"
    );
    Ok(())
}

#[test]
fn dropping_session_does_not_hang_or_panic() -> std::io::Result<()> {
    // Drop is the kill backstop; this only proves it fires without hanging.
    // Prompt reaping is kill + wait_for_exit (see resize test above).
    let session = PtySession::spawn(&PtyConfig::shell(80, 24))?;
    assert!(session.process_id().is_some());
    drop(session);
    Ok(())
}
