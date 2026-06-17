//! ASHIRT HTTP client base (BLOCKING).
//!
//! This wraps a [`reqwest::blocking::Client`] and ports the request plumbing
//! from the Go `network/common.go`. Per the locked runtime decision there is NO
//! async runtime: the menu-driven flow makes a handful of sequential calls, so a
//! blocking client is both simpler and sufficient.
//!
//! Responsibilities mirrored from Go:
//!   * base-URL handling — store the frontend URL and derive the API root by
//!     normalizing the trailing slash and appending `api` (see [`build_api_url`],
//!     a port of `SetBaseURL`).
//!   * JSON request helper — set `Content-Type: application/json`, attach a
//!     `Date` (RFC 1123 / GMT) and `Authorization` header signed via
//!     [`crate::ashirt::signing`], and deserialize the JSON response body.
//!   * status-code error mapping — `401` is an auth error, `500` is a server
//!     error, any other non-2xx is a generic status error.

use std::time::{Duration, SystemTime};

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, DATE};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;

use super::signing::{self, Credentials, SigningError};

/// Errors produced by the ASHIRT HTTP layer.
#[derive(Debug, Error)]
pub enum HttpError {
    /// The underlying HTTP client could not be constructed (e.g. a malformed
    /// `HTTP(S)_PROXY` environment variable).
    #[error("failed to build HTTP client: {0}")]
    Builder(#[source] reqwest::Error),
    /// The configured base URL (plus path) could not be parsed into a valid URL.
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    /// Computing the `Authorization` header failed.
    #[error("failed to sign request: {0}")]
    Signing(#[from] SigningError),
    /// The request failed to send or hit a transport-level error.
    #[error("ashirt request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// The server rejected the credentials (HTTP 401).
    #[error("unable to authenticate with server; please check credentials")]
    Unauthorized,
    /// The server encountered an internal error (HTTP 500).
    #[error("server encountered an error")]
    ServerError,
    /// The server returned some other non-success status.
    #[error("ashirt returned status {0}")]
    Status(u16),
    /// The request body could not be serialized to JSON.
    #[error("failed to serialize request body")]
    Serialize(#[source] serde_json::Error),
    /// The response body could not be parsed as the expected JSON shape.
    #[error("failed to parse response")]
    Deserialize(#[source] serde_json::Error),
}

/// Base client for ASHIRT API calls.
///
/// Construct with [`Client::new`], passing the frontend base URL (e.g.
/// `https://ashirt.example`); the API root (`.../api`) is derived internally.
#[derive(Debug, Clone)]
pub struct Client {
    /// The derived API root, e.g. `https://ashirt.example/api`. No trailing slash.
    api_url: String,
    /// Credentials used to sign every request.
    creds: Credentials,
    /// The underlying blocking HTTP client.
    inner: reqwest::blocking::Client,
}

/// Connection-establishment timeout for the blocking client.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Overall per-request timeout so the menu-driven flow never hangs forever on a
/// stalled server.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

impl Client {
    /// Creates a client targeting the ASHIRT instance at `base_url`, signing
    /// every request with `creds`.
    ///
    /// `base_url` is the frontend URL; the API root is derived from it via
    /// [`build_api_url`].
    ///
    /// Returns an error if the underlying HTTP client cannot be built — building
    /// via [`reqwest::blocking::ClientBuilder`] propagates failures (e.g. a
    /// malformed proxy environment variable) instead of panicking the process.
    pub fn new(base_url: impl AsRef<str>, creds: Credentials) -> Result<Self, HttpError> {
        let inner = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(HttpError::Builder)?;
        Ok(Self {
            api_url: build_api_url(base_url.as_ref()),
            creds,
            inner,
        })
    }

    /// The derived API root URL (`.../api`, no trailing slash).
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Issues a signed `GET` to `path` (relative to the API root, e.g.
    /// `/operations`) and deserializes the JSON response body into `T`.
    pub fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, HttpError> {
        self.request_json(Method::GET, path, None)
    }

    /// Issues a signed `POST` to `path` with `body` serialized as JSON, then
    /// deserializes the JSON response body into `T`.
    pub fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, HttpError> {
        let bytes = serde_json::to_vec(body).map_err(HttpError::Serialize)?;
        self.request_json(Method::POST, path, Some(bytes))
    }

    /// Issues a signed `POST` to `path` with a caller-supplied `content_type`
    /// and raw `body`, returning the raw response **without** status mapping or
    /// body parsing.
    ///
    /// The HMAC signature is computed over the exact `body` bytes that are sent,
    /// so the caller owns body construction — e.g. a serialized multipart
    /// payload whose boundary must be reflected in `content_type`. This is the
    /// escape hatch [`crate::ashirt::upload`] uses: reqwest's streaming
    /// multipart never exposes its serialized bytes, but the ASHIRT scheme signs
    /// `sha256(body)`, so the body must be materialized before signing.
    ///
    /// Status interpretation is left to the caller because the multipart upload
    /// endpoint needs the non-success response body (it carries the server's
    /// `error` field) rather than the generic [`map_status`] mapping.
    pub fn post_signed(
        &self,
        path: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> Result<reqwest::blocking::Response, HttpError> {
        let full_url = format!("{}{}", self.api_url, path);
        let url = reqwest::Url::parse(&full_url).map_err(|_| HttpError::InvalidUrl(full_url))?;

        // Request URI (path plus query) exactly as it appears on the wire,
        // mirroring the signing input used by `request_json`.
        let mut request_uri = url.path().to_string();
        if let Some(query) = url.query() {
            request_uri.push('?');
            request_uri.push_str(query);
        }

        // RFC 1123 / GMT date, identical to the header sent below.
        let date = httpdate::fmt_http_date(SystemTime::now());
        let authorization = signing::sign_request(&self.creds, "POST", &request_uri, &date, &body)?;

        let response = self
            .inner
            .post(url)
            .header(CONTENT_TYPE, content_type)
            .header(DATE, &date)
            .header(AUTHORIZATION, authorization)
            .body(body)
            .send()?;
        Ok(response)
    }

    /// Core request helper: builds the URL, signs the request, sends it, maps
    /// the status code to an error (if any), and deserializes the body.
    fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<T, HttpError> {
        let full_url = format!("{}{}", self.api_url, path);
        let url = reqwest::Url::parse(&full_url).map_err(|_| HttpError::InvalidUrl(full_url))?;

        // The canonical signing string uses the request URI (path plus query),
        // exactly as it appears on the wire, mirroring Go's `URL.RequestURI()`.
        let mut request_uri = url.path().to_string();
        if let Some(query) = url.query() {
            request_uri.push('?');
            request_uri.push_str(query);
        }

        // RFC 1123 / GMT date, identical to the header sent below — the signature
        // is computed over this exact value.
        let date = httpdate::fmt_http_date(SystemTime::now());

        // GET has an empty body; the signature covers whatever bytes are sent.
        let body_bytes = body.unwrap_or_default();
        let authorization = signing::sign_request(
            &self.creds,
            method.as_str(),
            &request_uri,
            &date,
            &body_bytes,
        )?;

        let mut request = self
            .inner
            .request(method, url)
            .header(CONTENT_TYPE, "application/json")
            .header(DATE, &date)
            .header(AUTHORIZATION, authorization);
        if !body_bytes.is_empty() {
            request = request.body(body_bytes);
        }

        let response = request.send()?;
        let status = response.status();
        if let Some(err) = map_status(status.as_u16()) {
            return Err(err);
        }

        let bytes = response.bytes()?;
        // A 204 No Content (or any empty body) has no JSON to parse; feeding an
        // empty slice to serde_json is an EOF error. Treat empty as JSON `null`
        // so unit/`Option` response types deserialize cleanly.
        if bytes.is_empty() {
            return serde_json::from_slice(b"null").map_err(HttpError::Deserialize);
        }
        serde_json::from_slice(&bytes).map_err(HttpError::Deserialize)
    }
}

/// Derives the ASHIRT API root from a frontend base URL.
///
/// Port of Go `SetBaseURL`, hardened to be idempotent: strip any trailing slash
/// and an already-present `/api` segment before appending `/api`, so that
/// `https://h`, `https://h/`, `https://h/api`, and `https://h/api/` all yield
/// `https://h/api` (never `https://h/api/api`).
fn build_api_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    let root = trimmed.strip_suffix("/api").unwrap_or(trimmed);
    format!("{root}/api")
}

/// Maps an HTTP status code to an [`HttpError`], or `None` for success (2xx).
///
/// Mirrors Go `evaluateResponseStatusCode`: 401 -> auth, 500 -> server, any
/// other non-2xx -> generic status.
fn map_status(code: u16) -> Option<HttpError> {
    match code {
        401 => Some(HttpError::Unauthorized),
        500 => Some(HttpError::ServerError),
        _ if (200..300).contains(&code) => None,
        _ => Some(HttpError::Status(code)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use serde::Deserialize;

    fn test_creds() -> Credentials {
        Credentials {
            access_key: "test-access-key".to_string(),
            // Valid base64; decodes to 24 bytes.
            secret_key: "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=".to_string(),
        }
    }

    fn client_for(server: &MockServer) -> Client {
        Client::new(server.base_url(), test_creds()).expect("client should build")
    }

    #[test]
    fn build_api_url_appends_api_without_trailing_slash() {
        assert_eq!(build_api_url("https://h"), "https://h/api");
    }

    #[test]
    fn build_api_url_collapses_trailing_slash() {
        assert_eq!(build_api_url("https://h/"), "https://h/api");
    }

    #[test]
    fn build_api_url_preserves_subpath() {
        assert_eq!(build_api_url("https://h/sub"), "https://h/sub/api");
        assert_eq!(build_api_url("https://h/sub/"), "https://h/sub/api");
    }

    #[test]
    fn build_api_url_is_idempotent_when_base_already_ends_in_api() {
        assert_eq!(build_api_url("https://h/api"), "https://h/api");
        assert_eq!(build_api_url("https://h/api/"), "https://h/api");
    }

    #[test]
    fn client_derives_api_url() {
        let c = Client::new("https://ashirt.example", test_creds()).expect("client should build");
        assert_eq!(c.api_url(), "https://ashirt.example/api");
    }

    #[test]
    fn map_status_success_is_none() {
        assert!(map_status(200).is_none());
        assert!(map_status(201).is_none());
        assert!(map_status(299).is_none());
    }

    #[test]
    fn map_status_401_is_unauthorized() {
        assert!(matches!(map_status(401), Some(HttpError::Unauthorized)));
    }

    #[test]
    fn map_status_500_is_server_error() {
        assert!(matches!(map_status(500), Some(HttpError::ServerError)));
    }

    #[test]
    fn map_status_other_non_2xx_is_generic() {
        assert!(matches!(map_status(404), Some(HttpError::Status(404))));
        assert!(matches!(map_status(403), Some(HttpError::Status(403))));
        assert!(matches!(map_status(502), Some(HttpError::Status(502))));
    }

    #[derive(Debug, Deserialize)]
    struct Op {
        slug: String,
        name: String,
    }

    #[test]
    fn signed_get_round_trips_with_auth_and_date_headers() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            // The request must carry the signed Authorization header (prefixed
            // with the access key) and a Date header, and target /api/operations.
            when.method(GET)
                .path("/api/operations")
                .header_exists("Date")
                .header_exists("Authorization")
                .header("Content-Type", "application/json");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!([
                    { "slug": "op1", "name": "Operation One" }
                ]));
        });

        let client = client_for(&server);
        let ops: Vec<Op> = client
            .get_json("/operations")
            .expect("signed GET should round-trip and deserialize");

        mock.assert();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].slug, "op1");
        assert_eq!(ops[0].name, "Operation One");
    }

    #[test]
    fn authorization_header_equals_independently_computed_signature() {
        use std::sync::{Arc, Mutex};

        let server = MockServer::start();

        // Capture the exact Date and Authorization headers the client sends so we
        // can recompute the signature from them and assert byte-for-byte equality.
        let captured: Arc<Mutex<Option<(String, String)>>> = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&captured);

        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/operations")
                .is_true(move |req| {
                    let headers = req.headers();
                    let date = headers
                        .get("date")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string);
                    let auth = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .map(str::to_string);
                    if let (Some(date), Some(auth)) = (date, auth) {
                        *sink.lock().unwrap() = Some((date, auth));
                    }
                    true
                });
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!([]));
        });

        let client = client_for(&server);
        let _: Vec<Op> = client.get_json("/operations").expect("GET should succeed");
        mock.assert();

        let (sent_date, sent_auth) = captured
            .lock()
            .unwrap()
            .clone()
            .expect("request must carry Date and Authorization headers");

        // Recompute the expected Authorization independently from the SAME inputs:
        // method, request URI, the Date the client actually sent, and an empty
        // GET body. Equality here proves the client signs exactly what it sends —
        // including that the signed Date matches the Date header on the wire
        // (a mismatch would break server-side HMAC validation).
        let expected =
            signing::sign_request(&test_creds(), "GET", "/api/operations", &sent_date, b"")
                .expect("independent signing should succeed");

        assert_eq!(sent_auth, expected);
        // Sanity: not just any string — it is access-key-prefixed.
        assert!(sent_auth.starts_with("test-access-key:"));
    }

    #[test]
    fn signed_post_round_trips_body_and_response() {
        let server = MockServer::start();

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/api/operations")
                .header_exists("Date")
                .header_exists("Authorization")
                .header("Content-Type", "application/json")
                .json_body(serde_json::json!({ "name": "New Op" }));
            then.status(201)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({ "slug": "new-op", "name": "New Op" }));
        });

        let client = client_for(&server);
        let body = serde_json::json!({ "name": "New Op" });
        let op: Op = client
            .post_json("/operations", &body)
            .expect("signed POST should round-trip and deserialize");

        mock.assert();
        assert_eq!(op.slug, "new-op");
    }

    #[test]
    fn empty_204_response_deserializes_to_unit() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/api/operations");
            // 204 No Content: success status, empty body.
            then.status(204);
        });

        let client = client_for(&server);
        // A unit response type must succeed against an empty body rather than
        // hitting a serde_json EOF error.
        client
            .get_json::<()>("/operations")
            .expect("empty 204 body should deserialize to unit");
        mock.assert();
    }

    #[test]
    fn unauthorized_status_maps_to_error() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/api/operations");
            then.status(401);
        });

        let client = client_for(&server);
        let err = client
            .get_json::<Vec<Op>>("/operations")
            .expect_err("401 must surface as an error");
        assert!(matches!(err, HttpError::Unauthorized));
    }

    #[test]
    fn server_error_status_maps_to_error() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/api/operations");
            then.status(500);
        });

        let client = client_for(&server);
        let err = client
            .get_json::<Vec<Op>>("/operations")
            .expect_err("500 must surface as an error");
        assert!(matches!(err, HttpError::ServerError));
    }

    #[test]
    fn generic_non_2xx_status_maps_to_status_error() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/api/operations");
            then.status(404);
        });

        let client = client_for(&server);
        let err = client
            .get_json::<Vec<Op>>("/operations")
            .expect_err("404 must surface as an error");
        assert!(matches!(err, HttpError::Status(404)));
    }
}
