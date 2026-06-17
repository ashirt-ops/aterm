//! ASHIRT evidence upload (multipart).
//!
//! Uploads a recording (asciicast) as evidence against an operation using a
//! `multipart/form-data` POST to `/operations/{slug}/evidence`. Port of the Go
//! `network/upload.go` (`UploadToAshirt`).
//!
//! ## Why the body is built by hand
//!
//! ASHIRT signs `sha256(REQUEST_BODY)` and sends the MAC in the `Authorization`
//! header (see [`crate::ashirt::signing`]). The signature must therefore be
//! computed over the *exact* bytes that go on the wire, and the request's
//! `Content-Type` must carry the multipart boundary used in those bytes.
//!
//! reqwest's `multipart::Form` streams its parts and never exposes the
//! serialized payload, so there is nothing to hash before sending. We instead
//! serialize the multipart body into an in-memory buffer with a known boundary,
//! sign that buffer, and hand the same buffer to [`Client::post_signed`] as the
//! raw body alongside `Content-Type: multipart/form-data; boundary=...`.

use std::fs;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use super::http::{Client, HttpError};

/// Content type for any terminal recording, per the ASHIRT API
/// (`network.ContentTypeTerminalRecording` in Go).
pub const CONTENT_TYPE_TERMINAL_RECORDING: &str = "terminal-recording";

/// Errors produced while uploading evidence.
#[derive(Debug, Error)]
pub enum UploadError {
    /// The evidence file could not be read from disk.
    #[error("failed to read evidence file {path}: {source}")]
    ReadFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// A transport, signing, or URL error from the HTTP layer.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The server rejected the upload (non-201). Carries the server's `error`
    /// field when present, mirroring Go's `Unable to upload file: <reason>`.
    #[error("unable to upload file: {0}")]
    Rejected(String),
    /// The 201 response body could not be parsed as the expected JSON shape.
    #[error("upload succeeded but the response could not be parsed")]
    ParseResponse(#[source] serde_json::Error),
}

/// Metadata describing a piece of evidence to upload.
#[derive(Debug, Clone)]
pub struct Evidence {
    /// Operation slug to attach the evidence to.
    pub operation_slug: String,
    /// MIME content type of the uploaded file (e.g.
    /// [`CONTENT_TYPE_TERMINAL_RECORDING`]).
    pub content_type: String,
    /// Human-readable description (sent as the `notes` field).
    pub description: String,
    /// Tag IDs to associate with the evidence; serialized as a JSON array
    /// string in the `tagIds` field.
    pub tag_ids: Vec<i64>,
}

/// The created-evidence response returned on a successful (201) upload.
#[derive(Debug, Clone, Deserialize)]
pub struct CreatedEvidence {
    /// Server-assigned UUID of the new evidence.
    pub uuid: String,
}

/// Uploads `file` as evidence described by `evidence`.
///
/// Reads the file into memory, builds a signed multipart request, and posts it
/// to `/operations/{slug}/evidence`. On 201 the created-evidence response is
/// parsed and returned; any other status surfaces the server's `error` message.
pub fn upload_evidence(
    client: &Client,
    evidence: &Evidence,
    file: &Path,
) -> Result<CreatedEvidence, UploadError> {
    let content = fs::read(file).map_err(|source| UploadError::ReadFile {
        path: file.display().to_string(),
        source,
    })?;

    // Derive the multipart filename from the path; fall back to a sensible
    // default if the path has no final component (e.g. ends in `..`).
    let filename = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("recording.cast");

    // `tagIds` is a JSON array string, matching Go's `json.Marshal(TagIDs)`.
    // Serializing a `Vec<i64>` never fails, so the error is unreachable.
    let tag_ids = serde_json::to_string(&evidence.tag_ids).unwrap_or_else(|_| "[]".to_string());

    let fields = [
        ("notes", evidence.description.as_str()),
        ("contentType", evidence.content_type.as_str()),
        ("tagIds", tag_ids.as_str()),
    ];

    let boundary = pick_boundary(&fields, &content);
    let body = serialize_multipart(&boundary, &fields, filename, &content);
    let content_type = format!("multipart/form-data; boundary={boundary}");

    let path = format!("/operations/{}/evidence", evidence.operation_slug);
    let response = client.post_signed(&path, &content_type, body)?;

    let status = response.status();
    let bytes = response.bytes().map_err(HttpError::Request)?;

    if status.as_u16() != 201 {
        return Err(UploadError::Rejected(extract_error_reason(&bytes)));
    }

    serde_json::from_slice(&bytes).map_err(UploadError::ParseResponse)
}

/// Pulls the `error` field out of a non-201 response body, falling back to a
/// generic message when the body is missing or not the expected shape — a port
/// of the Go branch that reads `parsed["error"]` or `(unknown server error)`.
fn extract_error_reason(bytes: &[u8]) -> String {
    serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .as_ref()
        .and_then(|v| v.get("error"))
        .and_then(|e| e.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| "(unknown server error)".to_string())
}

/// Serializes the text fields and a single file part into a `multipart/form-data`
/// body delimited by `boundary`.
///
/// The layout matches what a standards-compliant multipart writer emits: a
/// `--{boundary}` line before each part, CRLF-terminated headers, a blank line,
/// the raw value, and a trailing `--{boundary}--` close delimiter.
fn serialize_multipart(
    boundary: &str,
    fields: &[(&str, &str)],
    filename: &str,
    file_content: &[u8],
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(file_content);
    body.extend_from_slice(b"\r\n");

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

/// Chooses a multipart boundary that does not appear in any field value or in
/// the file content, so the delimiter can never collide with the payload.
///
/// Starts from a fixed token and lengthens it until it is absent from every
/// part; checking the raw token is sufficient because the boundary only ever
/// appears in the body as `--{boundary}`.
fn pick_boundary(fields: &[(&str, &str)], file_content: &[u8]) -> String {
    let mut boundary = String::from("aterm-boundary-x7Tf9pQzR2");
    while collides(boundary.as_bytes(), fields, file_content) {
        boundary.push('Z');
    }
    boundary
}

/// Returns true if `token` occurs in any field value or in the file content.
fn collides(token: &[u8], fields: &[(&str, &str)], file_content: &[u8]) -> bool {
    contains(file_content, token) || fields.iter().any(|(_, v)| contains(v.as_bytes(), token))
}

/// Substring search: does `haystack` contain `needle`?
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return needle.is_empty();
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ashirt::signing::{self, Credentials};
    use httpmock::prelude::*;
    use std::sync::{Arc, Mutex};

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

    /// Captured request fields: (Date, Authorization, Content-Type, raw body).
    type CapturedRequest = (String, String, String, Vec<u8>);

    /// Writes `bytes` to a uniquely named temp file and returns its path. The
    /// caller is responsible for removing it.
    fn write_temp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("aterm-upload-{}-{name}", std::process::id()));
        fs::write(&path, bytes).expect("temp file should write");
        path
    }

    #[test]
    fn upload_sends_multipart_fields_file_and_valid_signature() {
        let server = MockServer::start();

        // Capture the Date, Authorization, Content-Type and raw body so we can
        // recompute the signature over the exact bytes the server received.
        let captured: Arc<Mutex<Option<CapturedRequest>>> = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&captured);

        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/api/operations/op-1/evidence")
                .header_exists("Date")
                .header_exists("Authorization")
                .is_true(move |req| {
                    let h = req.headers();
                    let get = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
                    let body = req.body().to_vec();
                    if let (Some(date), Some(auth), Some(ct)) =
                        (get("date"), get("authorization"), get("content-type"))
                    {
                        *sink.lock().unwrap() = Some((date, auth, ct, body));
                    }
                    true
                });
            then.status(201)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({ "uuid": "evi-uuid-123" }));
        });

        let cast = b"{\"version\":3}\n[0.1,\"o\",\"hello\"]\n";
        let file = write_temp("recording.cast", cast);

        let client = client_for(&server);
        let evidence = Evidence {
            operation_slug: "op-1".to_string(),
            content_type: CONTENT_TYPE_TERMINAL_RECORDING.to_string(),
            description: "a recorded session".to_string(),
            tag_ids: vec![7, 42],
        };

        let created = upload_evidence(&client, &evidence, &file)
            .expect("upload should succeed and parse the 201 response");
        let _ = fs::remove_file(&file);

        mock.assert();
        assert_eq!(created.uuid, "evi-uuid-123");

        let (date, auth, content_type, body) = captured
            .lock()
            .unwrap()
            .clone()
            .expect("server must have captured the request");

        // The Content-Type advertises a multipart boundary, and that boundary
        // matches the one delimiting the captured body.
        let boundary = content_type
            .split("boundary=")
            .nth(1)
            .expect("Content-Type must declare a boundary");
        assert!(content_type.starts_with("multipart/form-data; boundary="));
        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains(&format!("--{boundary}\r\n")));
        assert!(body_str.trim_end().ends_with(&format!("--{boundary}--")));

        // All three text fields and the file part (with filename and bytes) are
        // present in the serialized body.
        assert!(body_str.contains("name=\"notes\"\r\n\r\na recorded session\r\n"));
        assert!(body_str.contains(&format!(
            "name=\"contentType\"\r\n\r\n{CONTENT_TYPE_TERMINAL_RECORDING}\r\n"
        )));
        assert!(body_str.contains("name=\"tagIds\"\r\n\r\n[7,42]\r\n"));
        let expected_filename = file.file_name().unwrap().to_str().unwrap();
        assert!(body_str.contains(&format!("name=\"file\"; filename=\"{expected_filename}\"")));
        assert!(body_str.contains("Content-Type: application/octet-stream"));
        assert!(body_str.contains("hello"));

        // The Authorization recomputed over the EXACT bytes the server received
        // must equal what the client sent — proving the signature covers the
        // serialized multipart body, not some other representation.
        let expected = signing::sign_request(
            &test_creds(),
            "POST",
            "/api/operations/op-1/evidence",
            &date,
            &body,
        )
        .expect("independent signing should succeed");
        assert_eq!(auth, expected);
        assert!(auth.starts_with("test-access-key:"));
    }

    #[test]
    fn upload_non_201_surfaces_server_error_message() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST).path("/api/operations/op-1/evidence");
            then.status(400)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({ "error": "operation not found" }));
        });

        let file = write_temp("reject.cast", b"{\"version\":3}\n");
        let client = client_for(&server);
        let evidence = Evidence {
            operation_slug: "op-1".to_string(),
            content_type: CONTENT_TYPE_TERMINAL_RECORDING.to_string(),
            description: "desc".to_string(),
            tag_ids: vec![],
        };

        let err = upload_evidence(&client, &evidence, &file)
            .expect_err("non-201 must surface as an error");
        let _ = fs::remove_file(&file);

        mock.assert();
        match err {
            UploadError::Rejected(reason) => assert_eq!(reason, "operation not found"),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn upload_non_201_without_error_field_uses_generic_reason() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST).path("/api/operations/op-1/evidence");
            then.status(500).body("not json at all");
        });

        let file = write_temp("reject-generic.cast", b"x");
        let client = client_for(&server);
        let evidence = Evidence {
            operation_slug: "op-1".to_string(),
            content_type: CONTENT_TYPE_TERMINAL_RECORDING.to_string(),
            description: "desc".to_string(),
            tag_ids: vec![],
        };

        let err = upload_evidence(&client, &evidence, &file)
            .expect_err("non-201 must surface as an error");
        let _ = fs::remove_file(&file);

        match err {
            UploadError::Rejected(reason) => assert_eq!(reason, "(unknown server error)"),
            other => panic!("expected Rejected, got {other:?}"),
        }
    }

    #[test]
    fn missing_file_reports_read_error() {
        let server = MockServer::start();
        let client = client_for(&server);
        let evidence = Evidence {
            operation_slug: "op-1".to_string(),
            content_type: CONTENT_TYPE_TERMINAL_RECORDING.to_string(),
            description: "desc".to_string(),
            tag_ids: vec![],
        };
        let missing = std::env::temp_dir().join("aterm-upload-does-not-exist-zzz.cast");
        let err = upload_evidence(&client, &evidence, &missing)
            .expect_err("a missing file must error before any request");
        assert!(matches!(err, UploadError::ReadFile { .. }));
    }

    #[test]
    fn boundary_avoids_collision_with_content() {
        // Force a collision: the file contains the default boundary token.
        let default_token = "aterm-boundary-x7Tf9pQzR2";
        let content = format!("noise {default_token} noise").into_bytes();
        let fields = [("notes", "n")];
        let boundary = pick_boundary(&fields, &content);
        assert_ne!(boundary, default_token);
        assert!(!contains(&content, boundary.as_bytes()));
    }

    #[test]
    fn contains_matches_substrings() {
        assert!(contains(b"hello world", b"o w"));
        assert!(!contains(b"hello", b"xyz"));
        assert!(contains(b"abc", b""));
        assert!(!contains(b"ab", b"abc"));
    }
}
