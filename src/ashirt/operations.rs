//! ASHIRT operations API.
//!
//! Port of Go `network/get_operations.go`. Builds on the signed JSON helpers
//! from [`crate::ashirt::http`]:
//!   * [`list_operations`] — `GET /operations`
//!
//! Errors surface as [`HttpError`], the shared error type for the ASHIRT network
//! layer; callers wrap it with `anyhow` at the application boundary.

use serde::{Deserialize, Serialize};

use super::http::{Client, HttpError};

/// An ASHIRT operation, as returned by `GET /operations`.
///
/// Only the fields aterm consumes are modeled; serde ignores any additional
/// fields the server may include, so the struct stays forward-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// URL-safe identifier used to address the operation in API paths.
    pub slug: String,
    /// Human-readable operation name.
    pub name: String,
    /// Numeric operation id. Defaulted when absent so partial payloads still
    /// deserialize.
    #[serde(default)]
    pub id: i64,
    /// Operation status code. Defaulted when absent.
    #[serde(default)]
    pub status: i64,
    /// Number of users with access to the operation. Defaulted when absent.
    #[serde(default, rename = "numUsers")]
    pub num_users: i64,
}

/// Lists the operations visible to the authenticated user.
pub fn list_operations(client: &Client) -> Result<Vec<Operation>, HttpError> {
    client.get_json("/operations")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ashirt::signing::Credentials;
    use httpmock::prelude::*;

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
    fn list_operations_deserializes() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/operations")
                .header_exists("Authorization")
                .header_exists("Date");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!([
                    { "slug": "s1", "name": "Jack", "numUsers": 1024, "status": 7, "id": 3 },
                    { "slug": "s2", "name": "Jill", "numUsers": 2048, "status": 2, "id": 10 },
                ]));
        });

        let client = client_for(&server);
        let ops = list_operations(&client).expect("list operations should succeed");

        mock.assert();
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].slug, "s1");
        assert_eq!(ops[0].name, "Jack");
        assert_eq!(ops[0].id, 3);
        assert_eq!(ops[0].status, 7);
        assert_eq!(ops[0].num_users, 1024);
        assert_eq!(ops[1].slug, "s2");
        assert_eq!(ops[1].num_users, 2048);
    }

    #[test]
    fn list_operations_tolerates_missing_optional_fields() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET).path("/api/operations");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!([{ "slug": "s1", "name": "Jack" }]));
        });

        let client = client_for(&server);
        let ops = list_operations(&client).expect("minimal payload should deserialize");

        mock.assert();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].slug, "s1");
        assert_eq!(ops[0].id, 0);
        assert_eq!(ops[0].num_users, 0);
    }
}
