//! Terminal UI prompt wrappers.
//!
//! Thin wrappers over interactive prompts so the rest of the app never depends
//! on the prompt library directly. The concrete impls land in aterm-8tn.5 and
//! will use `inquire` (over `crossterm`).

use thiserror::Error;

/// Errors produced by interactive prompts.
#[derive(Debug, Error)]
pub enum TuiError {
    /// The user aborted the prompt (e.g. Ctrl-C / Esc).
    #[error("prompt cancelled")]
    Cancelled,
}

/// Prompts the user to choose one of `options`.
// TODO(aterm-8tn.5): implement via `inquire::Select`.
pub fn select(_message: &str, _options: &[String]) -> Result<String, TuiError> {
    todo!("aterm-8tn.5: select prompt via inquire")
}

/// Prompts the user for a line of free-text input.
// TODO(aterm-8tn.5): implement via `inquire::Text`.
pub fn input(_message: &str) -> Result<String, TuiError> {
    todo!("aterm-8tn.5: text input via inquire")
}
