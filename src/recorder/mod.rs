//! PTY recording core.
//!
//! Platform split is established from day one: a [`unix`] submodule and a
//! [`windows`] submodule, selected by `#[cfg]`. The concrete backends land in
//! aterm-8tn.7 and will use `portable-pty` (BLOCKING — no async runtime).

use thiserror::Error;

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

/// Errors produced by a recording session.
#[derive(Debug, Error)]
pub enum RecorderError {
    /// The PTY/child process could not be spawned.
    #[error("failed to spawn pty session")]
    Spawn,
    // TODO(aterm-8tn.7): real variants (io, non-zero exit, resize failure...).
}

/// A recordable PTY session.
///
/// Implementations drive a child shell inside a pseudo-terminal and surface its
/// I/O so the recorder can emit asciicast events.
// TODO(aterm-8tn.7): concrete impls back this with `portable-pty` (blocking).
pub trait Session {
    /// Spawns `shell` inside a fresh PTY.
    fn spawn(&mut self, shell: &str) -> Result<(), RecorderError>;

    /// Propagates a terminal resize to the child PTY.
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), RecorderError>;

    /// Blocks until the child exits, returning its exit code.
    fn wait(&mut self) -> Result<i32, RecorderError>;
}

/// Returns the default [`Session`] implementation for the current platform.
#[cfg(unix)]
pub fn default_session() -> unix::UnixSession {
    unix::UnixSession
}

/// Returns the default [`Session`] implementation for the current platform.
#[cfg(windows)]
pub fn default_session() -> windows::WindowsSession {
    windows::WindowsSession
}
