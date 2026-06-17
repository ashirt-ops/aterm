//! Windows ConPTY recorder backend (`#[cfg(windows)]`).

use super::{RecorderError, Session};

/// Windows ConPTY-backed recording session.
// TODO(aterm-8tn.7): hold a `portable_pty` ConPTY pair + child handle.
#[derive(Debug, Default)]
pub struct WindowsSession;

impl Session for WindowsSession {
    fn spawn(&mut self, _shell: &str) -> Result<(), RecorderError> {
        todo!("aterm-8tn.7: windows conpty spawn via portable-pty")
    }

    fn resize(&mut self, _cols: u16, _rows: u16) -> Result<(), RecorderError> {
        todo!("aterm-8tn.7: windows conpty resize")
    }

    fn wait(&mut self) -> Result<i32, RecorderError> {
        todo!("aterm-8tn.7: windows conpty wait")
    }
}
