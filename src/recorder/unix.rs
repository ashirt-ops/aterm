//! Unix PTY recorder backend (`#[cfg(unix)]`).

use super::{RecorderError, Session};

/// Unix PTY-backed recording session.
// TODO(aterm-8tn.7): hold a `portable_pty::PtyPair` + child handle.
#[derive(Debug, Default)]
pub struct UnixSession;

impl Session for UnixSession {
    fn spawn(&mut self, _shell: &str) -> Result<(), RecorderError> {
        todo!("aterm-8tn.7: unix pty spawn via portable-pty")
    }

    fn resize(&mut self, _cols: u16, _rows: u16) -> Result<(), RecorderError> {
        todo!("aterm-8tn.7: unix pty resize")
    }

    fn wait(&mut self) -> Result<i32, RecorderError> {
        todo!("aterm-8tn.7: unix pty wait")
    }
}
