//! ASHIRT HTTP client base (BLOCKING).
//!
//! The concrete client lands in aterm-8tn.8 and will wrap a
//! `reqwest::blocking::Client`. Per the locked runtime decision there is NO
//! async runtime; when added, the dep is:
//!   reqwest = { version = "0.13", features = ["blocking", "json", "multipart"] }

use thiserror::Error;

use super::signing::Credentials;

/// Errors produced by the ASHIRT HTTP layer.
#[derive(Debug, Error)]
pub enum HttpError {
    /// The HTTP request failed to send or returned a transport-level error.
    #[error("ashirt request failed")]
    Request,
    /// The server returned a non-success status.
    #[error("ashirt returned status {0}")]
    Status(u16),
}

/// Base client for ASHIRT API calls.
#[derive(Debug, Clone, Default)]
pub struct Client {
    /// Base URL of the ASHIRT API.
    pub api_url: String,
    // TODO(aterm-8tn.8): hold a `reqwest::blocking::Client` and [`Credentials`].
}

impl Client {
    /// Creates a client targeting `api_url` authenticated with `creds`.
    pub fn new(api_url: impl Into<String>, _creds: Credentials) -> Self {
        Self {
            api_url: api_url.into(),
        }
    }
}
