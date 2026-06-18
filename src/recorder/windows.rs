//! Windows-specific recorder pieces (`#[cfg(windows)]`).
//!
//! Only the genuinely platform-specific parts live here; the PTY plumbing is
//! shared in [`super`] and runs over ConPTY (selected by `portable-pty`). On
//! Windows there is no `SIGWINCH`, so:
//!
//!   * console VT modes are configured for the recording so host key presses are
//!     forwarded to the ConPTY child as VT escape sequences and the child's VT
//!     output renders correctly ([`TerminalModeGuard`]);
//!   * resize detection polls the console size and emits a resize whenever it
//!     changes ([`ResizeWatcher`]); the shared session then drives the ConPTY
//!     resize;
//!   * stdin is read on a dedicated thread feeding a channel ([`StdinPoller`]),
//!     so the input-forwarding loop can poll with a timeout and react to the
//!     child exiting rather than blocking on a console read.
//!
//! # `unsafe`
//!
//! The crate is otherwise `#![deny(unsafe_code)]`; this module is the single
//! opt-out. The Windows console APIs used here — `GetStdHandle` /
//! `GetConsoleMode` / `SetConsoleMode` for VT mode setup, and
//! `WaitForSingleObject` to wait on the console input handle with a timeout so
//! the stdin reader stays interruptible — have no safe-wrapper equivalent (the
//! way `rustix` covers the Unix syscalls), so each call is a small, individually
//! justified `unsafe` block with a `SAFETY` comment.
#![allow(unsafe_code)]

use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Console::{
    CONSOLE_MODE, DISABLE_NEWLINE_AUTO_RETURN, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT,
    ENABLE_PROCESSED_INPUT, ENABLE_VIRTUAL_TERMINAL_INPUT, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
    GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetConsoleMode,
};
use windows_sys::Win32::System::Threading::WaitForSingleObject;

use super::StdinRead;

/// Reads the console mode for `handle`, returning `None` when the handle is not
/// a console (e.g. redirected stdin/stdout) so callers can no-op gracefully.
fn console_mode(handle: HANDLE) -> Option<CONSOLE_MODE> {
    let mut mode: CONSOLE_MODE = 0;
    // SAFETY: `handle` comes from `GetStdHandle`; `GetConsoleMode` writes the
    // current mode through the out-pointer and returns nonzero on success.
    let ok = unsafe { GetConsoleMode(handle, &mut mode) };
    (ok != 0).then_some(mode)
}

/// Best-effort `SetConsoleMode`; failures (e.g. redirected handles) are ignored.
fn set_console_mode(handle: HANDLE, mode: CONSOLE_MODE) {
    // SAFETY: `handle` comes from `GetStdHandle`; the call only reads `mode`.
    unsafe {
        let _ = SetConsoleMode(handle, mode);
    }
}

/// Configures the host console's VT modes for the duration of a recording and
/// restores the exact previous modes on drop.
///
/// Unlike crossterm's raw mode — which only *clears* the cooked-input flags and
/// never touches the virtual-terminal flags — a ConPTY input forwarder must:
///
///   * **input:** enable `ENABLE_VIRTUAL_TERMINAL_INPUT` so the console encodes
///     key presses (arrows, Ctrl-R, function keys, …) as the VT escape sequences
///     the child shell expects. PowerShell/PSReadLine's line editor and
///     reverse-search only work when those sequences arrive; without the flag
///     the console delivers only typed characters and special keys are lost. The
///     cooked-input flags are also cleared so the child — not our console — owns
///     line editing and echo.
///   * **output:** enable `ENABLE_VIRTUAL_TERMINAL_PROCESSING` so the child's VT
///     output (colours, cursor motion) renders instead of printing escape bytes
///     literally, plus `DISABLE_NEWLINE_AUTO_RETURN` to stop the console adding a
///     carriage return at the right margin.
///
/// Restoring the captured modes on drop matters as much as setting them: the
/// post-recording menu uses crossterm/inquire, which reads native key-event
/// records rather than VT input, so leaving `ENABLE_VIRTUAL_TERMINAL_INPUT`
/// enabled is what makes the menu's arrow keys misbehave afterwards.
#[must_use = "console modes are restored when the guard is dropped; bind it to a variable"]
pub struct TerminalModeGuard {
    stdin: HANDLE,
    stdout: HANDLE,
    original_stdin: Option<CONSOLE_MODE>,
    original_stdout: Option<CONSOLE_MODE>,
}

impl TerminalModeGuard {
    /// Captures the current console modes and switches them into VT mode for
    /// recording. Non-console handles are left untouched (and not restored).
    pub fn install() -> io::Result<Self> {
        // SAFETY: `GetStdHandle` returns this process's standard handles.
        let stdin = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        let stdout = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };

        let original_stdin = console_mode(stdin);
        let original_stdout = console_mode(stdout);

        if let Some(mode) = original_stdin {
            let new = (mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT))
                | ENABLE_VIRTUAL_TERMINAL_INPUT;
            set_console_mode(stdin, new);
        }

        if let Some(mode) = original_stdout {
            let new = mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN;
            set_console_mode(stdout, new);
        }

        Ok(Self {
            stdin,
            stdout,
            original_stdin,
            original_stdout,
        })
    }
}

impl Drop for TerminalModeGuard {
    fn drop(&mut self) {
        // Best-effort restore of the exact modes captured at install; nothing is
        // actionable if it fails and a Drop impl must not panic.
        if let Some(mode) = self.original_stdin {
            set_console_mode(self.stdin, mode);
        }
        if let Some(mode) = self.original_stdout {
            set_console_mode(self.stdout, mode);
        }
    }
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
///
/// The thread is stopped and joined on drop (see [`Drop`]). This matters because
/// a blocking console read left running past the end of a recording would steal
/// the first keystroke aimed at the post-recording menu — and a join on a thread
/// stuck in an un-interruptible read would hang until the user pressed a key. To
/// avoid both, the reader gates each read behind a short timed wait so it can
/// observe the stop flag and exit on its own.
pub struct StdinPoller {
    rx: Receiver<Vec<u8>>,
    leftover: Vec<u8>,
    eof: bool,
    /// The reader thread, taken and joined in [`Drop`].
    reader: Option<JoinHandle<()>>,
    /// Signals the reader thread to stop between waits.
    stop: Arc<AtomicBool>,
}

impl StdinPoller {
    /// Spawns the background stdin reader.
    pub fn new() -> io::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let reader_stop = Arc::clone(&stop);
        let reader = thread::spawn(move || {
            // SAFETY: returns this process's standard input handle.
            let handle: HANDLE = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
            let stdin = io::stdin();
            let mut lock = stdin.lock();
            let mut buf = [0u8; 8192];
            loop {
                if reader_stop.load(Ordering::Relaxed) {
                    break;
                }
                // A blocking console read cannot be cancelled, so it would pin the
                // thread until the next keystroke — making teardown hang and the
                // post-recording menu fail to appear. Instead wait on the console
                // handle with a 50ms timeout: on timeout the loop re-checks `stop`
                // and can exit on its own (the shell-exit case, where no input is
                // pending), and only reads when input is actually available.
                // SAFETY: `handle` is the console input handle from GetStdHandle.
                match unsafe { WaitForSingleObject(handle, 50) } {
                    WAIT_TIMEOUT => continue,
                    WAIT_OBJECT_0 => {}
                    // WAIT_FAILED / WAIT_ABANDONED: stop forwarding rather than
                    // spin on a handle we can no longer wait on.
                    _ => break,
                }
                // Input is pending, so this read returns promptly. (A lone
                // non-character event could still make `read` block until the next
                // character, but at shell-exit the thread is parked in the wait
                // above, not here, so teardown is not held up.)
                match lock.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        // If we were asked to stop, or the receiver is gone, drop
                        // this chunk and exit rather than forward (and thereby
                        // consume) input meant for the next phase.
                        if reader_stop.load(Ordering::Relaxed)
                            || tx.send(buf[..n].to_vec()).is_err()
                        {
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
            reader: Some(reader),
            stop,
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

impl Drop for StdinPoller {
    fn drop(&mut self) {
        // Signal the reader to stop. It is parked in a 50ms-bounded wait rather
        // than an un-interruptible console read, so it observes this and exits
        // within one wait interval — the join completes promptly without needing
        // a keystroke, letting the post-recording menu appear immediately.
        self.stop.store(true, Ordering::Relaxed);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
