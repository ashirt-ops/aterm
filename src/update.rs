//! GitHub release update check.
//!
//! Checks whether a newer aterm release is published on GitHub. Lands in
//! aterm-8tn.6 and will use `octocrab` for the release lookup.

use thiserror::Error;

/// Errors produced while checking for updates.
#[derive(Debug, Error)]
pub enum UpdateError {
    /// The latest-release query failed.
    #[error("failed to query latest release")]
    Query,
}

/// A release available on GitHub.
#[derive(Debug, Clone)]
pub struct Release {
    /// Release tag (e.g. `v1.2.3`).
    pub tag: String,
    /// Browser URL for the release.
    pub url: String,
}

/// Returns the newest release if it is newer than `current`, else `None`.
// TODO(aterm-8tn.6): query the latest GitHub release via `octocrab`.
pub fn check_for_update(_current: &str) -> Result<Option<Release>, UpdateError> {
    todo!("aterm-8tn.6: octocrab release check")
}
