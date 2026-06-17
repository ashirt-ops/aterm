//! Post-recording menu.
//!
//! Rust replacement for the Go `appdialogs/upload_menu.go`. Once a recording has
//! finished, this presents the operator with what to do next:
//!   * **Upload to ASHIRT** — collect a description, multiselect tags (with an
//!     option to create a brand-new tag), then submit the `.cast` file as
//!     terminal-recording evidence via [`crate::ashirt::upload::upload_evidence`].
//!   * **Rename recording** — move the file to a new name in the same directory.
//!   * **Discard recording** — delete the file (behind a yes/no confirm).
//!   * **Return to main menu** — leave the recording on disk untouched.
//!
//! # Headless note
//!
//! Following the same discipline as [`crate::tui`], every interactive prompt is a
//! thin shell around the `tui` wrappers (which need a real TTY and can never run
//! in CI). All decision logic that *can* be tested headlessly is factored into
//! standalone pure functions — the selected-label → tag-id mapping
//! ([`selected_tag_ids`]) and the file operations ([`rename_recording`],
//! [`discard_recording`]) — and unit-tested below. The interactive `*_flow`
//! helpers and [`post_recording_menu`] are never exercised by the test suite.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::ashirt::http::{Client, HttpError};
use crate::ashirt::ops_tags::{self, Tag};
use crate::ashirt::upload::{
    upload_evidence, Evidence, UploadError, CONTENT_TYPE_TERMINAL_RECORDING,
};
use crate::tui::{self, TuiError};

/// Label shown at the top of the tag multiselect to create a new tag inline.
pub const CREATE_TAG_LABEL: &str = "+ Create a new tag";

// Menu entries, kept as constants so the `select` result can be matched exactly.
const MENU_UPLOAD: &str = "Upload to ASHIRT";
const MENU_RENAME: &str = "Rename recording";
const MENU_DISCARD: &str = "Discard recording";
const MENU_RETURN: &str = "Return to main menu";

/// Errors surfaced by the post-recording menu.
#[derive(Debug, Error)]
pub enum MenuError {
    /// An interactive prompt was cancelled, interrupted, or failed.
    #[error(transparent)]
    Tui(#[from] TuiError),
    /// A tags API call (list/create) failed.
    #[error(transparent)]
    Http(#[from] HttpError),
    /// The evidence upload failed.
    #[error(transparent)]
    Upload(#[from] UploadError),
    /// A filesystem operation (rename/discard) failed.
    #[error("file operation failed on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

// ---------------------------------------------------------------------------
// Pure logic (headless-testable).
// ---------------------------------------------------------------------------

/// Builds the multiselect option list: the create-new-tag sentinel first, then
/// each available tag's name in order.
pub fn tag_options(tags: &[Tag]) -> Vec<String> {
    let mut options = Vec::with_capacity(tags.len() + 1);
    options.push(CREATE_TAG_LABEL.to_string());
    options.extend(tags.iter().map(|tag| tag.name.clone()));
    options
}

/// Returns whether the create-new-tag sentinel is among the selected labels.
pub fn wants_new_tag(selected: &[String]) -> bool {
    selected.iter().any(|label| label == CREATE_TAG_LABEL)
}

/// Maps selected display labels back to their tag ids.
///
/// The create-new-tag sentinel and any label that does not match a known tag are
/// ignored; matching tag ids are returned in selection order. This is the pure
/// core of the upload flow's tag handling.
pub fn selected_tag_ids(selected: &[String], tags: &[Tag]) -> Vec<i64> {
    selected
        .iter()
        .filter(|label| label.as_str() != CREATE_TAG_LABEL)
        .filter_map(|label| tags.iter().find(|tag| &tag.name == label).map(|tag| tag.id))
        .collect()
}

/// Computes the destination path for a rename: a sibling of `original` named
/// `new_name`. If `new_name` carries no extension, `original`'s extension (if
/// any) is preserved so a bare name like `session` keeps its `.cast` suffix.
pub fn rename_target(original: &Path, new_name: &str) -> PathBuf {
    let parent = original.parent().unwrap_or_else(|| Path::new(""));
    let mut target = parent.join(new_name);
    if target.extension().is_none() {
        if let Some(ext) = original.extension() {
            target.set_extension(ext);
        }
    }
    target
}

/// Renames the recording at `original` to a sibling named `new_name` (see
/// [`rename_target`]), returning the new path.
pub fn rename_recording(original: &Path, new_name: &str) -> std::io::Result<PathBuf> {
    let target = rename_target(original, new_name);
    std::fs::rename(original, &target)?;
    Ok(target)
}

/// Deletes the recording at `path`.
pub fn discard_recording(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

// ---------------------------------------------------------------------------
// Interactive flow. MANUAL/TTY-ONLY: never call from tests or headless paths.
// ---------------------------------------------------------------------------

/// Presents the post-recording menu, looping until the operator returns to the
/// main menu or discards the recording.
///
/// `recording` is the path to the just-finished `.cast` file; `client` and
/// `operation_slug` address the ASHIRT operation to upload evidence against.
pub fn post_recording_menu(
    client: &Client,
    operation_slug: &str,
    recording: &Path,
) -> Result<(), MenuError> {
    let mut path = recording.to_path_buf();
    let options = vec![
        MENU_UPLOAD.to_string(),
        MENU_RENAME.to_string(),
        MENU_DISCARD.to_string(),
        MENU_RETURN.to_string(),
    ];

    loop {
        let prompt = format!(
            "Recording saved to {}. What would you like to do?",
            path.display()
        );
        match tui::select(&prompt, &options)?.as_str() {
            MENU_UPLOAD => upload_flow(client, operation_slug, &path)?,
            MENU_RENAME => path = rename_flow(&path)?,
            MENU_DISCARD => {
                if discard_flow(&path)? {
                    return Ok(());
                }
            }
            MENU_RETURN => return Ok(()),
            // `select` only ever returns one of the options we passed in.
            _ => unreachable!("select returned an option that was not offered"),
        }
    }
}

/// Collects a description and tags, then uploads the recording as evidence.
fn upload_flow(client: &Client, operation_slug: &str, path: &Path) -> Result<(), MenuError> {
    let description = tui::input("Description for this evidence")?;

    let available = ops_tags::list_tags(client, operation_slug)?;
    let selected = tui::multiselect(
        "Select tags (space to toggle, enter to confirm)",
        &tag_options(&available),
    )?;

    let mut tag_ids = selected_tag_ids(&selected, &available);
    if wants_new_tag(&selected) {
        let name = tui::required_input("New tag name")?;
        let created =
            ops_tags::create_tag(client, operation_slug, &name, ops_tags::random_tag_color())?;
        tag_ids.push(created.id);
    }

    let evidence = Evidence {
        operation_slug: operation_slug.to_string(),
        content_type: CONTENT_TYPE_TERMINAL_RECORDING.to_string(),
        description,
        tag_ids,
    };

    let created = upload_evidence(client, &evidence, path)?;
    println!("{} Uploaded evidence {}", tui::green_check(), created.uuid);
    Ok(())
}

/// Prompts for a new name and renames the recording, returning the new path.
fn rename_flow(path: &Path) -> Result<PathBuf, MenuError> {
    let current = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("recording.cast");
    let new_name = tui::input_with_default("New recording name", current)?;
    let target = rename_recording(path, &new_name).map_err(|source| MenuError::Io {
        path: path.display().to_string(),
        source,
    })?;
    println!("Renamed to {}", target.display());
    Ok(target)
}

/// Confirms and deletes the recording. Returns `true` if it was discarded.
fn discard_flow(path: &Path) -> Result<bool, MenuError> {
    let prompt = format!("Delete {}? This cannot be undone.", path.display());
    if !tui::confirm(&prompt, false)? {
        return Ok(false);
    }
    discard_recording(path).map_err(|source| MenuError::Io {
        path: path.display().to_string(),
        source,
    })?;
    println!("Recording discarded.");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn tag(id: i64, name: &str) -> Tag {
        Tag {
            id,
            name: name.to_string(),
            color_name: "blue".to_string(),
        }
    }

    /// Allocates a unique temp path so parallel tests never collide (mirrors the
    /// pattern used elsewhere; avoids a `tempfile` dependency).
    fn temp_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "aterm-upload-menu-{}-{}-{}.cast",
            tag,
            std::process::id(),
            n
        ))
    }

    #[test]
    fn tag_options_lists_sentinel_then_names() {
        let tags = [tag(1, "alpha"), tag(2, "beta")];
        let options = tag_options(&tags);
        assert_eq!(options, [CREATE_TAG_LABEL, "alpha", "beta"]);
    }

    #[test]
    fn selected_tag_ids_maps_names_to_ids_in_order() {
        let tags = [tag(1, "alpha"), tag(2, "beta"), tag(3, "gamma")];
        let selected = vec!["gamma".to_string(), "alpha".to_string()];
        assert_eq!(selected_tag_ids(&selected, &tags), vec![3, 1]);
    }

    #[test]
    fn selected_tag_ids_ignores_sentinel_and_unknown_labels() {
        let tags = [tag(1, "alpha"), tag(2, "beta")];
        let selected = vec![
            CREATE_TAG_LABEL.to_string(),
            "beta".to_string(),
            "does-not-exist".to_string(),
        ];
        assert_eq!(selected_tag_ids(&selected, &tags), vec![2]);
    }

    #[test]
    fn wants_new_tag_detects_sentinel() {
        assert!(wants_new_tag(&[CREATE_TAG_LABEL.to_string()]));
        assert!(wants_new_tag(&[
            "alpha".to_string(),
            CREATE_TAG_LABEL.to_string()
        ]));
        assert!(!wants_new_tag(&["alpha".to_string(), "beta".to_string()]));
        assert!(!wants_new_tag(&[]));
    }

    #[test]
    fn rename_target_preserves_extension_for_bare_name() {
        let original = Path::new("/tmp/recordings/session-1.cast");
        assert_eq!(
            rename_target(original, "renamed"),
            PathBuf::from("/tmp/recordings/renamed.cast")
        );
    }

    #[test]
    fn rename_target_keeps_explicit_extension() {
        let original = Path::new("/tmp/recordings/session-1.cast");
        assert_eq!(
            rename_target(original, "renamed.json"),
            PathBuf::from("/tmp/recordings/renamed.json")
        );
    }

    #[test]
    fn rename_recording_moves_the_file() {
        let from = temp_path("rename-from");
        std::fs::write(&from, b"{\"version\":3}\n").expect("seed recording");

        let target = rename_recording(&from, "renamed").expect("rename should succeed");

        assert!(!from.exists(), "original path must be gone after rename");
        assert!(target.exists(), "renamed file must exist");
        assert_eq!(target.extension().and_then(|e| e.to_str()), Some("cast"));
        assert_eq!(
            std::fs::read(&target).expect("read renamed"),
            b"{\"version\":3}\n"
        );

        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn rename_recording_missing_source_errors() {
        let missing = temp_path("rename-missing");
        let err = rename_recording(&missing, "renamed").expect_err("missing source must error");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn discard_recording_deletes_the_file() {
        let path = temp_path("discard");
        std::fs::write(&path, b"x").expect("seed recording");
        assert!(path.exists());

        discard_recording(&path).expect("discard should succeed");

        assert!(!path.exists(), "file must be deleted after discard");
    }

    #[test]
    fn discard_recording_missing_file_errors() {
        let missing = temp_path("discard-missing");
        let err = discard_recording(&missing).expect_err("missing file must error");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
