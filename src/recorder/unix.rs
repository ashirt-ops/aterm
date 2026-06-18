//! Unix-specific recorder pieces (`#[cfg(unix)]`).
//!
//! Only the genuinely platform-specific parts live here; PTY plumbing is shared
//! in [`super`]. On Unix:
//!
//!   * resize detection is driven by `SIGWINCH` ([`ResizeWatcher`]);
//!   * stdin is polled non-blocking with `poll(2)` ([`StdinPoller`]) so the
//!     input-forwarding loop can notice the child exiting and stop promptly.

use std::io;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};
use rustix::io::Errno;

use super::StdinRead;

/// No-op terminal-mode guard on Unix.
///
/// Unix PTYs already deliver raw VT input and render VT output without any extra
/// console-mode setup, so the cooked-mode clearing done by the shared
/// [`RawModeGuard`](super::RawModeGuard) is sufficient. This type exists only so
/// the shared recorder can install platform terminal-mode setup uniformly; its
/// Windows counterpart configures the console's virtual-terminal modes.
#[must_use = "kept symmetric with the Windows guard; bind it to a variable"]
pub struct TerminalModeGuard;

impl TerminalModeGuard {
    /// Installs nothing; succeeds unconditionally.
    pub fn install() -> io::Result<Self> {
        Ok(Self)
    }
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
        let stdin = io::stdin();
        let timeout = Timespec {
            tv_sec: timeout.as_secs() as Secs,
            tv_nsec: timeout.subsec_nanos() as Nsecs,
        };
        let mut fds = [PollFd::new(&stdin, PollFlags::IN)];

        let ready = match poll(&mut fds, Some(&timeout)) {
            Ok(ready) => ready,
            // A signal (e.g. SIGWINCH) interrupted the poll — not an error.
            Err(Errno::INTR) => return Ok(StdinRead::Timeout),
            Err(err) => return Err(err.into()),
        };
        if ready == 0 {
            return Ok(StdinRead::Timeout);
        }
        let revents = fds[0].revents();
        // The fd hung up or errored (e.g. closed stdin) — end of input.
        if revents.intersects(PollFlags::HUP | PollFlags::ERR) {
            return Ok(StdinRead::Eof);
        }
        if !revents.contains(PollFlags::IN) {
            return Ok(StdinRead::Timeout);
        }

        let n = rustix::io::read(stdin.as_fd(), buf).map_err(io::Error::from)?;
        if n == 0 {
            return Ok(StdinRead::Eof);
        }
        Ok(StdinRead::Data(n))
    }
}
