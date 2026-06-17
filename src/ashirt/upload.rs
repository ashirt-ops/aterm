//! ASHIRT evidence upload (multipart).
//!
//! Uploads a recording (asciicast) as evidence against an operation using a
//! multipart request. Requires the `reqwest` `multipart` feature (see [`super::http`]).

use std::path::Path;

use super::http::{Client, HttpError};

/// Metadata describing a piece of evidence to upload.
#[derive(Debug, Clone)]
pub struct Evidence {
    /// Operation slug to attach the evidence to.
    pub operation_slug: String,
    /// MIME content type of the uploaded file.
    pub content_type: String,
    /// Human-readable description.
    pub description: String,
}

/// Uploads `file` as evidence described by `evidence`.
// TODO(aterm-8tn.N): build the multipart body via `reqwest::blocking::multipart`.
pub fn upload_evidence(
    _client: &Client,
    _evidence: &Evidence,
    _file: &Path,
) -> Result<(), HttpError> {
    todo!("ashirt::upload: multipart evidence upload")
}
