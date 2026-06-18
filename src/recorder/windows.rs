//! Windows-specific recorder pieces (`#[cfg(windows)]`).
//!
//! Only the genuinely platform-specific parts live here; the PTY plumbing is
//! shared in [`super`] and runs over ConPTY (selected by `portable-pty`). On
//! Windows there is no `SIGWINCH`, so:
//!
//!   * resize detection polls the console size and emits a resize whenever it
//!     changes ([`ResizeWatcher`]); the shared session then drives the ConPTY
//!     resize;
//!   * stdin is read on a dedicated thread feeding a channel ([`StdinPoller`]),
//!     so the input-forwarding loop can poll with a timeout and react to the
//!     child exiting rather than blocking on a console read.

use std::io::{self, Read};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use super::StdinRead;

/// Returns the user's shell. Honors `%COMSPEC%`, falling back to `cmd.exe`.
pub fn default_shell() -> String {
    std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
}

/// Detects terminal resizes by polling the console size.
///
/// Windows has no `SIGWINCH`; each [`take_pending`](ResizeWatcher::take_pending)
/// compares the current geometry against the last seen value.
pub struct ResizeWatcher {
    last: (u16, u16),
}

impl ResizeWatcher {
    /// Captures the initial console size as the baseline.
    pub fn install() -> io::Result<Self> {
        Ok(Self {
            last: crossterm::terminal::size().unwrap_or((80, 24)),
        })
    }

    /// Returns whether the console size changed since the last call.
    pub fn take_pending(&mut self) -> bool {
        let current = crossterm::terminal::size().unwrap_or(self.last);
        if current != self.last {
            self.last = current;
            true
        } else {
            false
        }
    }
}

/// Non-blocking reader over standard input backed by a dedicated reader thread.
///
/// Console reads block until input arrives, so a background thread performs the
/// blocking reads and forwards chunks over a channel; [`poll`](StdinPoller::poll)
/// drains the channel with a timeout.
pub struct StdinPoller {
    rx: Receiver<Vec<u8>>,
    leftover: Vec<u8>,
    eof: bool,
}

impl StdinPoller {
    /// Spawns the background stdin reader.
    pub fn new() -> io::Result<Self> {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let stdin = io::stdin();
            let mut lock = stdin.lock();
            let mut buf = [0u8; 8192];
            loop {
                match lock.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            rx,
            leftover: Vec::new(),
            eof: false,
        })
    }

    /// Returns up to `buf.len()` bytes of available input, waiting at most
    /// `timeout`.
    pub fn poll(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<StdinRead> {
        if self.leftover.is_empty() {
            if self.eof {
                return Ok(StdinRead::Eof);
            }
            match self.rx.recv_timeout(timeout) {
                Ok(data) => self.leftover = data,
                Err(RecvTimeoutError::Timeout) => return Ok(StdinRead::Timeout),
                Err(RecvTimeoutError::Disconnected) => {
                    self.eof = true;
                    return Ok(StdinRead::Eof);
                }
            }
        }

        let n = buf.len().min(self.leftover.len());
        buf[..n].copy_from_slice(&self.leftover[..n]);
        self.leftover.drain(..n);
        Ok(StdinRead::Data(n))
    }
}
