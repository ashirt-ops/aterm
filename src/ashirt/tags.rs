//! ASHIRT tags API.
//!
//! Port of Go `network/tagging.go`. Builds on the signed JSON helpers from
//! [`crate::ashirt::http`]:
//!   * [`list_tags`]        — `GET /operations/{slug}/tags`
//!   * [`create_tag`]       — `POST /operations/{slug}/tags`
//!   * [`random_tag_color`] — pick a color from the ASHIRT tag palette
//!
//! Errors surface as [`HttpError`], the shared error type for the ASHIRT network
//! layer; callers wrap it with `anyhow` at the application boundary.

use serde::{Deserialize, Serialize};

use super::http::{Client, HttpError};

/// A tag within an operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    /// Server-assigned tag id.
    pub id: i64,
    /// Tag label.
    pub name: String,
    /// Palette color name (see [`random_tag_color`]).
    #[serde(rename = "colorName")]
    pub color_name: String,
}

/// Request body for creating a tag. Mirrors Go's anonymous `TagInput`.
#[derive(Debug, Serialize)]
struct NewTag<'a> {
    name: &'a str,
    #[serde(rename = "colorName")]
    color_name: &'a str,
}

/// The ASHIRT tag color palette, mirroring Go `RandomTagColor`.
const TAG_COLORS: &[&str] = &[
    "blue",
    "yellow",
    "green",
    "indigo",
    "orange",
    "pink",
    "red",
    "teal",
    "vermilion",
    "violet",
    "lightBlue",
    "lightYellow",
    "lightGreen",
    "lightIndigo",
    "lightOrange",
    "lightPink",
    "lightRed",
    "lightTeal",
    "lightVermilion",
    "lightViolet",
];

/// Lists the tags defined for `operation_slug`.
pub fn list_tags(client: &Client, operation_slug: &str) -> Result<Vec<Tag>, HttpError> {
    client.get_json(&format!("/operations/{operation_slug}/tags"))
}

/// Creates a tag named `name` with palette color `color_name` on
/// `operation_slug`, returning the server's created [`Tag`] (with its id).
pub fn create_tag(
    client: &Client,
    operation_slug: &str,
    name: &str,
    color_name: &str,
) -> Result<Tag, HttpError> {
    let body = NewTag { name, color_name };
    client.post_json(&format!("/operations/{operation_slug}/tags"), &body)
}

/// Returns a random color name from the ASHIRT tag palette.
///
/// Port of Go `RandomTagColor`. Rather than pull in the `rand` crate for a
/// single pick, this draws a `u64` from [`crate::random::random_u64`] (a real OS
/// randomness source) and reduces it into the palette index.
pub fn random_tag_color() -> &'static str {
    let r = crate::random::random_u64();
    let idx = (r % TAG_COLORS.len() as u64) as usize;
    TAG_COLORS[idx]
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
    fn list_tags_deserializes() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/api/operations/op1/tags")
                .header_exists("Authorization");
            then.status(200)
                .header("content-type", "application/json")
                .json_body(serde_json::json!([
                    { "id": 1, "name": "alpha", "colorName": "blue" },
                    { "id": 2, "name": "beta", "colorName": "lightRed" },
                ]));
        });

        let client = client_for(&server);
        let tags = list_tags(&client, "op1").expect("list tags should succeed");

        mock.assert();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].id, 1);
        assert_eq!(tags[0].name, "alpha");
        assert_eq!(tags[0].color_name, "blue");
        assert_eq!(tags[1].color_name, "lightRed");
    }

    #[test]
    fn create_tag_sends_body_and_deserializes_response() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(POST)
                .path("/api/operations/op1/tags")
                .header_exists("Authorization")
                .header("Content-Type", "application/json")
                .json_body(serde_json::json!({ "name": "gamma", "colorName": "teal" }));
            then.status(201)
                .header("content-type", "application/json")
                .json_body(serde_json::json!({ "id": 42, "name": "gamma", "colorName": "teal" }));
        });

        let client = client_for(&server);
        let tag = create_tag(&client, "op1", "gamma", "teal").expect("create tag should succeed");

        mock.assert();
        assert_eq!(tag.id, 42);
        assert_eq!(tag.name, "gamma");
        assert_eq!(tag.color_name, "teal");
    }

    #[test]
    fn random_tag_color_is_in_palette() {
        // Many draws: every result must be a valid palette color.
        for _ in 0..1000 {
            let color = random_tag_color();
            assert!(
                TAG_COLORS.contains(&color),
                "random_tag_color returned {color:?}, which is not in the palette"
            );
        }
    }

    #[test]
    fn random_tag_color_varies() {
        // Sanity: the picker is not pinned to a single value. With 20 colors over
        // 200 draws, observing more than one distinct color is overwhelmingly
        // likely if the source is actually random.
        let mut seen = std::collections::HashSet::new();
        for _ in 0..200 {
            seen.insert(random_tag_color());
        }
        assert!(seen.len() > 1, "random_tag_color appears to be constant");
    }

    #[test]
    fn palette_matches_ashirt() {
        // Guards against accidental edits to the ported palette.
        assert_eq!(TAG_COLORS.len(), 20);
        assert_eq!(TAG_COLORS[0], "blue");
        assert_eq!(TAG_COLORS[19], "lightViolet");
    }
}
