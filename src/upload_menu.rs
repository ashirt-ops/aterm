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
//! standalone pure functions — the selected-index → tag-id mapping
//! ([`selected_tag_ids`]) and the file operations ([`rename_target`],
//! [`rename_recording`], [`discard_recording`]) — and unit-tested below. The
//! interactive `*_flow` helpers and [`post_recording_menu`] are never exercised
//! by the test suite.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::ashirt::http::{Client, HttpError};
use crate::ashirt::ops_tags::{self, Tag};
use crate::ashirt::upload::{
    upload_evidence, Evidence, UploadError, CONTENT_TYPE_TERMINAL_RECORDING,
};
use crate::tui::{self, TuiError};

/// Label shown at the top of the tag multiselect to create a new tag inline.
///
/// Selection is resolved by *position*, not by matching this string (see
/// [`selected_tag_ids`] / [`wants_new_tag`]), so this label may safely coincide
/// with a real tag name without affecting behaviour — it is display-only.
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
    /// The requested new name is not a safe bare filename: it is empty, contains
    /// a path separator or `..`, or would otherwise escape the recording's own
    /// directory. Rejected to prevent path traversal.
    #[error(
        "invalid recording name {name:?}: must be a bare filename within the recording directory"
    )]
    InvalidName { name: String },
    /// The recording's current filename is not valid UTF-8, so it cannot be
    /// shown for editing or safely renamed (substituting a fixed name could
    /// silently overwrite another file).
    #[error("recording filename is not valid UTF-8: {path}")]
    NonUtf8Name { path: String },
}

// ---------------------------------------------------------------------------
// Pure logic (headless-testable).
// ---------------------------------------------------------------------------

/// Builds the multiselect option list: the create-new-tag sentinel first, then
/// each available tag's name in order. The sentinel therefore always lives at
/// index 0, and tag `i` lives at index `i + 1` (see [`selected_tag_ids`]).
pub fn tag_options(tags: &[Tag]) -> Vec<String> {
    let mut options = Vec::with_capacity(tags.len() + 1);
    options.push(CREATE_TAG_LABEL.to_string());
    options.extend(tags.iter().map(|tag| tag.name.clone()));
    options
}

/// Returns whether the create-new-tag sentinel (index 0) is among the selected
/// option indices.
pub fn wants_new_tag(selected: &[usize]) -> bool {
    selected.contains(&0)
}

/// Maps selected option indices back to their tag ids.
///
/// Index 0 is the create-new-tag sentinel and is skipped; index `i >= 1` maps to
/// `tags[i - 1]`. Out-of-range indices are ignored. Because the mapping is purely
/// positional, a real tag whose name equals [`CREATE_TAG_LABEL`] is handled
/// correctly. Ids are returned in selection order.
pub fn selected_tag_ids(selected: &[usize], tags: &[Tag]) -> Vec<i64> {
    selected
        .iter()
        .filter(|&&index| index != 0)
        .filter_map(|&index| tags.get(index - 1))
        .map(|tag| tag.id)
        .collect()
}

/// Computes the destination path for a rename: a sibling of `original` named
/// `new_name`, **within `original`'s own directory**.
///
/// `new_name` is treated strictly as a bare filename. It is rejected (with
/// [`MenuError::InvalidName`]) if it is empty/whitespace, contains a path
/// separator (`/` or `\`), or contains `..` — this prevents a user from steering
/// the rename (and a later discard) outside the recording's directory. If the
/// resulting name carries no extension, `original`'s extension (if any) is
/// preserved so a bare name like `session` keeps its `.cast` suffix. As a final
/// guard the computed target's parent must equal `original`'s parent.
pub fn rename_target(original: &Path, new_name: &str) -> Result<PathBuf, MenuError> {
    let trimmed = new_name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
    {
        return Err(MenuError::InvalidName {
            name: new_name.to_string(),
        });
    }

    let parent = original.parent().unwrap_or_else(|| Path::new(""));
    let mut target = parent.join(trimmed);
    if target.extension().is_none() {
        if let Some(ext) = original.extension() {
            target.set_extension(ext);
        }
    }

    // Defense in depth: the rename must never leave the original's directory.
    if target.parent() != original.parent() {
        return Err(MenuError::InvalidName {
            name: new_name.to_string(),
        });
    }

    Ok(target)
}

/// Renames the recording at `original` to a sibling named `new_name` (see
/// [`rename_target`]), returning the new path.
///
/// Beyond the lexical checks in [`rename_target`], this canonicalizes both
/// directories (when they exist) and refuses the rename if they differ, so the
/// target can never resolve outside the recording's directory.
pub fn rename_recording(original: &Path, new_name: &str) -> Result<PathBuf, MenuError> {
    let target = rename_target(original, new_name)?;

    // Canonicalized guard against any residual traversal (symlinks, etc.). Both
    // paths refer to the same directory by construction, so a mismatch — or an
    // original directory that cannot be canonicalized — is treated as unsafe.
    let orig_dir = original.parent().unwrap_or_else(|| Path::new("."));
    let target_dir = target.parent().unwrap_or_else(|| Path::new("."));
    if let (Ok(a), Ok(b)) = (orig_dir.canonicalize(), target_dir.canonicalize()) {
        if a != b {
            return Err(MenuError::InvalidName {
                name: new_name.to_string(),
            });
        }
    }

    std::fs::rename(original, &target).map_err(|source| MenuError::Io {
        path: original.display().to_string(),
        source,
    })?;
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
            // `select` only ever returns one of the options we passed in; on any
            // unexpected value just re-present the menu rather than panicking.
            _ => continue,
        }
    }
}

/// Collects a description and tags, then uploads the recording as evidence.
fn upload_flow(client: &Client, operation_slug: &str, path: &Path) -> Result<(), MenuError> {
    let description = tui::required_input("Description for this evidence")?;

    let available = ops_tags::list_tags(client, operation_slug)?;
    let selected = tui::multiselect_indexed(
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
    // Refuse to proceed on a non-UTF-8 name rather than substituting a fixed
    // default, which could silently overwrite an existing file.
    let current = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| MenuError::NonUtf8Name {
            path: path.display().to_string(),
        })?;
    let new_name = tui::input_with_default("New recording name", current)?;
    let target = rename_recording(path, &new_name)?;
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
    fn selected_tag_ids_maps_indices_to_ids_in_order() {
        let tags = [tag(1, "alpha"), tag(2, "beta"), tag(3, "gamma")];
        // Indices into tag_options: 1=alpha, 2=beta, 3=gamma.
        let selected = vec![3usize, 1usize];
        assert_eq!(selected_tag_ids(&selected, &tags), vec![3, 1]);
    }

    #[test]
    fn selected_tag_ids_skips_sentinel_and_out_of_range() {
        let tags = [tag(1, "alpha"), tag(2, "beta")];
        // 0 = create sentinel, 2 = beta, 9 = out of range.
        let selected = vec![0usize, 2usize, 9usize];
        assert_eq!(selected_tag_ids(&selected, &tags), vec![2]);
    }

    #[test]
    fn selected_tag_ids_handles_tag_named_like_sentinel() {
        // A real tag whose name collides with the sentinel string must still be
        // resolved by position and never dropped or treated as "create new".
        let tags = [tag(1, CREATE_TAG_LABEL), tag(2, "beta")];
        // Index 1 is that real tag; index 0 would be the sentinel.
        assert_eq!(selected_tag_ids(&[1usize], &tags), vec![1]);
        assert!(!wants_new_tag(&[1usize]));
        assert!(wants_new_tag(&[0usize]));
    }

    #[test]
    fn wants_new_tag_detects_sentinel_index() {
        assert!(wants_new_tag(&[0usize]));
        assert!(wants_new_tag(&[2usize, 0usize]));
        assert!(!wants_new_tag(&[1usize, 2usize]));
        assert!(!wants_new_tag(&[]));
    }

    #[test]
    fn rename_target_preserves_extension_for_bare_name() {
        let original = Path::new("/tmp/recordings/session-1.cast");
        assert_eq!(
            rename_target(original, "renamed").expect("bare name is valid"),
            PathBuf::from("/tmp/recordings/renamed.cast")
        );
    }

    #[test]
    fn rename_target_keeps_explicit_extension() {
        let original = Path::new("/tmp/recordings/session-1.cast");
        assert_eq!(
            rename_target(original, "renamed.json").expect("explicit ext is valid"),
            PathBuf::from("/tmp/recordings/renamed.json")
        );
    }

    #[test]
    fn rename_target_rejects_path_traversal() {
        let original = Path::new("/tmp/recordings/session-1.cast");
        for evil in ["../escape", "../../etc/passwd", "..", "sub/dir", "a\\b", ""] {
            let err = rename_target(original, evil)
                .expect_err("traversal / non-bare names must be rejected");
            assert!(
                matches!(err, MenuError::InvalidName { .. }),
                "expected InvalidName for {evil:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn rename_recording_moves_the_file_within_its_dir() {
        let from = temp_path("rename-from");
        std::fs::write(&from, b"{\"version\":3}\n").expect("seed recording");

        // Use a unique destination filename (its own temp path's file name) so
        // concurrent test processes never target the same path.
        let to = temp_path("rename-to");
        let new_name = to.file_name().and_then(|n| n.to_str()).expect("utf-8 name");

        let target = rename_recording(&from, new_name).expect("rename should succeed");

        assert_eq!(target, to);
        assert!(!from.exists(), "original path must be gone after rename");
        assert!(target.exists(), "renamed file must exist");
        assert_eq!(
            std::fs::read(&target).expect("read renamed"),
            b"{\"version\":3}\n"
        );

        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn rename_recording_rejects_traversal_without_touching_fs() {
        let from = temp_path("rename-guard");
        std::fs::write(&from, b"x").expect("seed recording");

        let err = rename_recording(&from, "../escaped").expect_err("traversal must error");
        assert!(matches!(err, MenuError::InvalidName { .. }));
        // The source file must be untouched by a rejected rename.
        assert!(from.exists(), "source must survive a rejected rename");

        std::fs::remove_file(&from).ok();
    }

    #[test]
    fn rename_recording_missing_source_errors() {
        let missing = temp_path("rename-missing");
        let unique = temp_path("rename-missing-dest");
        let new_name = unique.file_name().and_then(|n| n.to_str()).unwrap();
        let err = rename_recording(&missing, new_name).expect_err("missing source must error");
        match err {
            MenuError::Io { source, .. } => {
                assert_eq!(source.kind(), std::io::ErrorKind::NotFound)
            }
            other => panic!("expected Io error, got {other:?}"),
        }
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
