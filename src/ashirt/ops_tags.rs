//! ASHIRT operations and tags API.

use serde::{Deserialize, Serialize};

use super::http::{Client, HttpError};

/// An ASHIRT operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub slug: String,
    pub name: String,
}

/// A tag within an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
}

/// Lists the operations visible to the authenticated user.
// TODO(aterm-8tn.N): GET /api/operations via [`Client`].
pub fn list_operations(_client: &Client) -> Result<Vec<Operation>, HttpError> {
    todo!("ashirt::ops_tags: list operations")
}

/// Lists the tags defined for `operation_slug`.
// TODO(aterm-8tn.N): GET /api/operations/{slug}/tags via [`Client`].
pub fn list_tags(_client: &Client, _operation_slug: &str) -> Result<Vec<Tag>, HttpError> {
    todo!("ashirt::ops_tags: list tags")
}
