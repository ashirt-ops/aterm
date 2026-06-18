//! ASHIRT request signing.
//!
//! ASHIRT authenticates API calls with an HMAC-SHA256 signature over a canonical
//! representation of the request, sent in the `Authorization` header. This is a
//! port of the Go `ashirt-server/signer` (see `network/common.go`) and MUST match
//! the server byte-for-byte.
//!
//! The canonical request is:
//!
//! ```text
//! METHOD \n REQUEST_URI \n DATE \n SHA256(body)
//! ```
//!
//! where `REQUEST_URI` includes any query string, `DATE` is the RFC 1123 value of
//! the request's `Date` header (in GMT), and `SHA256(body)` is the **raw 32-byte
//! digest** of the body (an empty body for GET) appended directly — not hex, not
//! base64. The resulting MAC is base64 (standard alphabet) encoded:
//!
//! ```text
//! Authorization = access_key + ":" + base64(HMAC_SHA256(secret_bytes, canonical))
//! ```
//!
//! `secret_bytes` is the base64 decoding of the credential's secret key.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

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
    /// The secret key, base64-encoded (standard alphabet), as stored in config.
    pub secret_key: String,
}

/// Decodes a base64 (standard alphabet) secret key into its raw bytes.
///
/// This is the single source of truth for how a stored secret key is decoded:
/// both request signing and config validation go through it, so they agree
/// byte-for-byte on what counts as a well-formed secret.
pub fn decode_secret_key(secret_key: &str) -> Result<Vec<u8>, SigningError> {
    STANDARD
        .decode(secret_key.as_bytes())
        .map_err(|_| SigningError::InvalidSecret)
}

/// Computes the `Authorization` header value for an ASHIRT API request.
///
/// `request_uri` is the path including any query string. `date` is the value of
/// the request's `Date` header, which must be formatted as RFC 1123 in GMT and
/// identical to what is sent on the wire. `body` is the raw request body (empty
/// for GET).
pub fn sign_request(
    creds: &Credentials,
    method: &str,
    request_uri: &str,
    date: &str,
    body: &[u8],
) -> Result<String, SigningError> {
    let secret_bytes = decode_secret_key(&creds.secret_key)?;

    // Canonical request: METHOD \n REQUEST_URI \n DATE \n SHA256(body).
    // The body digest is the raw 32 digest bytes, appended directly.
    let body_digest = Sha256::digest(body);

    let mut canonical = Vec::new();
    canonical.extend_from_slice(method.as_bytes());
    canonical.push(b'\n');
    canonical.extend_from_slice(request_uri.as_bytes());
    canonical.push(b'\n');
    canonical.extend_from_slice(date.as_bytes());
    canonical.push(b'\n');
    canonical.extend_from_slice(&body_digest);

    // HMAC accepts a key of any length, so this never fails.
    let mut mac =
        HmacSha256::new_from_slice(&secret_bytes).expect("HMAC-SHA256 accepts keys of any length");
    mac.update(&canonical);
    let signature = mac.finalize().into_bytes();

    Ok(format!(
        "{}:{}",
        creds.access_key,
        STANDARD.encode(signature)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden vector computed by an INDEPENDENT oracle (Python `hmac`/`hashlib`),
    /// NOT by this Rust implementation. Asserting against this literal proves the
    /// port reproduces an externally-derived MAC byte-for-byte and breaks any
    /// self-referential circularity.
    #[test]
    fn golden_vector_get_empty_body() {
        let creds = Credentials {
            access_key: "test-access-key".to_string(),
            secret_key: "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=".to_string(),
        };

        let auth = sign_request(
            &creds,
            "GET",
            "/api/operations",
            "Mon, 02 Jan 2006 15:04:05 GMT",
            b"",
        )
        .expect("signing must succeed for a valid secret");

        assert_eq!(
            auth,
            "test-access-key:BPZTE5El8Zw7uXr3sFyb1r9QuWeHgPTrbqSU3u0lbYY="
        );
    }

    #[test]
    fn invalid_secret_is_rejected() {
        let creds = Credentials {
            access_key: "test-access-key".to_string(),
            secret_key: "not valid base64!!!".to_string(),
        };

        let err = sign_request(
            &creds,
            "GET",
            "/api/operations",
            "Mon, 02 Jan 2006 15:04:05 GMT",
            b"",
        )
        .expect_err("malformed base64 secret must be rejected");
        assert!(matches!(err, SigningError::InvalidSecret));
    }
}
