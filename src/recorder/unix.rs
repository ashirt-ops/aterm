//! Unix-specific recorder pieces (`#[cfg(unix)]`).
//!
//! Only the genuinely platform-specific parts live here; PTY plumbing is shared
//! in [`super`]. On Unix:
//!
//!   * resize detection is driven by `SIGWINCH` ([`ResizeWatcher`]);
//!   * stdin is polled non-blocking with `poll(2)` ([`StdinPoller`]) so the
//!     input-forwarding loop can notice the child exiting and stop promptly.

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::StdinRead;

/// Returns the user's preferred shell (`$SHELL`), falling back to `/bin/sh`.
pub fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

/// Detects terminal resizes via the `SIGWINCH` signal.
///
/// A single flag is set by the signal handler (installed once via `signal-hook`)
/// and consumed by [`take_pending`](ResizeWatcher::take_pending) on each loop
/// iteration. The actual new geometry is queried separately by the caller.
pub struct ResizeWatcher {
    flag: Arc<AtomicBool>,
}

impl ResizeWatcher {
    /// Registers a `SIGWINCH` handler that records pending resizes.
    pub fn install() -> io::Result<Self> {
        let flag = Arc::new(AtomicBool::new(false));
        signal_hook::flag::register(signal_hook::consts::SIGWINCH, Arc::clone(&flag))?;
        Ok(Self { flag })
    }

    /// Returns whether a resize is pending, clearing the flag.
    pub fn take_pending(&mut self) -> bool {
        self.flag.swap(false, Ordering::Relaxed)
    }
}

/// Non-blocking reader over standard input using `poll(2)`.
pub struct StdinPoller;

impl StdinPoller {
    /// Creates a poller over file descriptor 0.
    pub fn new() -> io::Result<Self> {
        Ok(Self)
    }

    /// Waits up to `timeout` for input and reads what is available into `buf`.
    ///
    /// `SIGWINCH` interrupting `poll(2)` surfaces as [`StdinRead::Timeout`] so the
    /// caller loops back around and picks up the pending resize.
    pub fn poll(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<StdinRead> {
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        let mut fds = [libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        }];

        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 1, timeout_ms) };
        if ready < 0 {
            let err = io::Error::last_os_error();
            // A signal (e.g. SIGWINCH) interrupted the poll — not an error.
            if err.kind() == io::ErrorKind::Interrupted {
                return Ok(StdinRead::Timeout);
            }
            return Err(err);
        }
        if ready == 0 {
            return Ok(StdinRead::Timeout);
        }
        // The fd hung up or errored (e.g. closed stdin) — end of input.
        if fds[0].revents & (libc::POLLHUP | libc::POLLERR) != 0 {
            return Ok(StdinRead::Eof);
        }
        if fds[0].revents & libc::POLLIN == 0 {
            return Ok(StdinRead::Timeout);
        }

        let n = unsafe {
            libc::read(
                libc::STDIN_FILENO,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n < 0 {
            return Err(io::Error::last_os_error());
        }
        if n == 0 {
            return Ok(StdinRead::Eof);
        }
        Ok(StdinRead::Data(n as usize))
    }
}
