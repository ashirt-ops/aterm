//! ASHIRT request signing.
//!
//! ASHIRT authenticates API calls with an HMAC-SHA256 signature over a canonical
//! representation of the request, sent in the `Authorization` header.

use thiserror::Error;

/// Errors produced while signing a request.
#[derive(Debug, Error)]
pub enum SigningError {
    /// The secret key was not valid base64 / could not be decoded.
    #[error("invalid secret key encoding")]
    InvalidSecret,
}

/// ASHIRT API credentials.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub access_key: String,
    pub secret_key: String,
}

/// Computes the `Authorization` header value for an ASHIRT API request.
// TODO(aterm-8tn.3): implement HMAC-SHA256 over the canonical request.
// Deps to add then: `hmac`, `sha2`, `base64`.
pub fn sign_request(
    _creds: &Credentials,
    _method: &str,
    _path: &str,
    _body: &[u8],
) -> Result<String, SigningError> {
    todo!("aterm-8tn.3: HMAC-SHA256 ASHIRT request signing")
}
