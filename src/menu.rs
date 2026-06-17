//! Main menu + recording entry flow.
//!
//! This is the Rust replacement for the Go `appdialogs/main_menu.go` plus the
//! recording-start path (`recording/start_recording.go`). It ties together the
//! existing building blocks:
//!   * [`crate::tui`] — the searchable-select menu primitive;
//!   * [`crate::ashirt::ops_tags::list_operations`] — operation selection;
//!   * [`crate::config`] — output directory / file name / shell;
//!   * [`crate::recorder::record_session`] — the actual recording.
//!
//! # Two distinct phases
//!
//! The menu and the recording are kept as **separate phases**: the menu prompts
//! run to completion, then [`record_session`] takes over the terminal (raw mode)
//! for the duration of the recording, then control returns to the menu loop.
//! There is no persistent stdin router straddling both phases (the Go
//! `copyRouter` hack); [`record_session`] owns stdin only while it runs and
//! releases it cleanly on child exit.
//!
//! # Headless gate
//!
//! Interactive prompts (`inquire`) and the recorder both need a real TTY, so
//! they can never run in CI. To keep this module testable, ALL decision logic is
//! factored into the pure functions below — menu-item/action mapping
//! ([`dispatch_main_menu`], [`MAIN_MENU`]), operation → choice construction
//! ([`operation_choices`]), and recording output-path construction
//! ([`output_path`], [`resolve_output_file_name`]) — and unit-tested without a
//! terminal. The interactive [`run`] entry point and [`start_recording`] wire
//! those pure pieces into `tui`/`recorder` and are never exercised by tests.

use std::collections::hash_map::RandomState;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::ashirt::http::{Client, HttpError};
use crate::ashirt::ops_tags::{Operation, list_operations};
use crate::config::Config;
use crate::recorder::{RecorderError, record_session};
use crate::tui::{self, TuiError};
use crate::upload_menu;

/// Errors produced by the menu / recording flow.
#[derive(Debug, Error)]
pub enum MenuError {
    /// Talking to the ASHIRT API (e.g. listing operations) failed.
    #[error("ashirt request failed")]
    Http(#[from] HttpError),

    /// An interactive prompt failed (other than a user cancel/interrupt, which
    /// the loop handles as "go back" / "quit" rather than an error).
    #[error("menu prompt failed")]
    Tui(#[from] TuiError),

    /// The recording session itself failed.
    #[error("recording failed")]
    Recorder(#[from] RecorderError),

    /// The post-recording (upload) menu failed.
    #[error("post-recording menu failed")]
    UploadMenu(#[from] upload_menu::MenuError),

    /// The recording output directory could not be created.
    #[error("failed to create recording directory {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The recording output file could not be created.
    #[error("failed to create recording file {path}")]
    CreateFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

// ---------------------------------------------------------------------------
// Pure logic (headless-testable). The interactive flow below delegates here.
// ---------------------------------------------------------------------------

/// An action the main menu can dispatch to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// Pick an operation and record a session against it.
    StartRecording,
    /// Re-fetch the operations list from the server.
    RefreshOperations,
    /// Open the settings menu to edit and persist configuration.
    Settings,
    /// Leave the application.
    Quit,
}

/// A single main-menu entry: the label shown to the user and the action it maps
/// to.
#[derive(Debug, Clone, Copy)]
pub struct MenuItem {
    /// The text rendered in the select list.
    pub label: &'static str,
    /// The action taken when this entry is chosen.
    pub action: MenuAction,
}

/// The main menu, in display order. Mirrors the core of the Go main menu, scoped
/// to the building blocks this issue owns (recording + operations); config-edit
/// and connection-test entries are left for later issues.
pub const MAIN_MENU: &[MenuItem] = &[
    MenuItem {
        label: "Start recording",
        action: MenuAction::StartRecording,
    },
    MenuItem {
        label: "Refresh operations",
        action: MenuAction::RefreshOperations,
    },
    MenuItem {
        label: "Settings",
        action: MenuAction::Settings,
    },
    MenuItem {
        label: "Quit",
        action: MenuAction::Quit,
    },
];

/// The labels for [`MAIN_MENU`], in order — the input to the select prompt.
pub fn main_menu_labels() -> Vec<String> {
    MAIN_MENU
        .iter()
        .map(|item| item.label.to_string())
        .collect()
}

/// Maps a selected main-menu label back to its [`MenuAction`].
///
/// Returns `None` for a label that is not in [`MAIN_MENU`] (defensive: the
/// select prompt only ever returns one of the labels we supplied).
pub fn dispatch_main_menu(label: &str) -> Option<MenuAction> {
    MAIN_MENU
        .iter()
        .find(|item| item.label == label)
        .map(|item| item.action)
}

/// A free-text configuration field the settings menu can edit in place.
///
/// These mirror the editable values in the Go aterm "Update Settings" guide.
/// Each variant knows how to read its current value from a [`Config`], write a
/// new value back, and whether it must be masked ([`SettingsField::is_secret`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    /// `apiURL` — base URL of the ASHIRT API.
    ApiUrl,
    /// `accessKey` — API access key.
    AccessKey,
    /// `secretKey` — API secret key (masked).
    SecretKey,
    /// `outputDir` — base directory recordings are written to.
    OutputDir,
    /// `operationSlug` — operation slug to record against.
    OperationSlug,
    /// `recordingShell` — shell launched when recording.
    RecordingShell,
}

impl SettingsField {
    /// Every editable field, in the order they appear in the settings menu.
    pub const ALL: &'static [SettingsField] = &[
        SettingsField::ApiUrl,
        SettingsField::AccessKey,
        SettingsField::SecretKey,
        SettingsField::OutputDir,
        SettingsField::OperationSlug,
        SettingsField::RecordingShell,
    ];

    /// The human-readable name used in menu labels and prompts.
    pub fn display_name(self) -> &'static str {
        match self {
            SettingsField::ApiUrl => "API URL",
            SettingsField::AccessKey => "Access Key",
            SettingsField::SecretKey => "Secret Key",
            SettingsField::OutputDir => "Output Directory",
            SettingsField::OperationSlug => "Operation Slug",
            SettingsField::RecordingShell => "Recording Shell",
        }
    }

    /// Whether this field holds a secret that must be masked (entered via a
    /// hidden password prompt and never echoed in a label).
    pub fn is_secret(self) -> bool {
        matches!(self, SettingsField::SecretKey)
    }

    /// Reads this field's current value from `cfg`.
    pub fn current_value(self, cfg: &Config) -> &str {
        match self {
            SettingsField::ApiUrl => &cfg.api_url,
            SettingsField::AccessKey => &cfg.access_key,
            SettingsField::SecretKey => &cfg.secret_key,
            SettingsField::OutputDir => &cfg.output_dir,
            SettingsField::OperationSlug => &cfg.operation_slug,
            SettingsField::RecordingShell => &cfg.recording_shell,
        }
    }

    /// Writes `value` into this field of `cfg`. This is the pure
    /// "apply-value-to-Config per field" half of an edit; persistence and
    /// rollback live in the interactive flow.
    pub fn apply(self, cfg: &mut Config, value: String) {
        match self {
            SettingsField::ApiUrl => cfg.api_url = value,
            SettingsField::AccessKey => cfg.access_key = value,
            SettingsField::SecretKey => cfg.secret_key = value,
            SettingsField::OutputDir => cfg.output_dir = value,
            SettingsField::OperationSlug => cfg.operation_slug = value,
            SettingsField::RecordingShell => cfg.recording_shell = value,
        }
    }

    /// Builds this field's settings-menu label for the current config state:
    /// `"<name>: <value>"`, with an empty value shown as `(not set)` and a
    /// secret shown only as `(set)`/`(not set)` so it is never echoed.
    pub fn label(self, cfg: &Config) -> String {
        let name = self.display_name();
        if self.is_secret() {
            let state = if self.current_value(cfg).is_empty() {
                "(not set)"
            } else {
                "(set)"
            };
            format!("{name}: {state}")
        } else {
            let value = self.current_value(cfg);
            let shown = if value.is_empty() { "(not set)" } else { value };
            format!("{name}: {shown}")
        }
    }
}

/// An action the settings menu can dispatch to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    /// Edit a free-text/secret configuration field in place.
    EditField(SettingsField),
    /// Toggle the startup auto-update check on/off.
    ToggleAutoUpdateCheck,
    /// Return to the main menu.
    Back,
}

/// The fixed label for the settings-menu "back" entry.
const SETTINGS_BACK_LABEL: &str = "Back to main menu";

/// Builds the auto-update toggle's menu label, showing its current state. The
/// label is dynamic (it reflects `enabled`) so it lives here rather than in a
/// static table.
pub fn auto_update_toggle_label(enabled: bool) -> String {
    format!(
        "Automatic update check: {} (select to toggle)",
        if enabled { "enabled" } else { "disabled" }
    )
}

/// The settings-menu labels, in display order, for the current config state:
/// every editable field, then the auto-update toggle, then the back entry.
pub fn settings_menu_labels(config: &Config) -> Vec<String> {
    let mut labels: Vec<String> = SettingsField::ALL
        .iter()
        .map(|&field| field.label(config))
        .collect();
    labels.push(auto_update_toggle_label(config.auto_update_check));
    labels.push(SETTINGS_BACK_LABEL.to_string());
    labels
}

/// Maps a selected settings-menu label back to its [`SettingsAction`].
///
/// Matching is exhaustive over the labels [`settings_menu_labels`] produces for
/// `config`: the fixed back entry, the dynamic auto-update toggle (compared
/// against its rendering for the current state), and each editable field's
/// label. An unrecognized label returns `None` rather than being coerced into a
/// real action — the select prompt only ever returns a label we supplied, so
/// this is purely defensive.
pub fn dispatch_settings_menu(config: &Config, label: &str) -> Option<SettingsAction> {
    if label == SETTINGS_BACK_LABEL {
        return Some(SettingsAction::Back);
    }
    if label == auto_update_toggle_label(config.auto_update_check) {
        return Some(SettingsAction::ToggleAutoUpdateCheck);
    }
    SettingsField::ALL
        .iter()
        .find(|&&field| field.label(config) == label)
        .map(|&field| SettingsAction::EditField(field))
}

/// One selectable operation in the operation-selection prompt: the display label
/// and the slug it resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationChoice {
    /// Label shown in the select list (the operation name, with a `(Current)`
    /// marker for the configured operation).
    pub label: String,
    /// The operation slug this choice records against.
    pub slug: String,
}

/// Builds the operation-selection choices from the available operations.
///
/// Mirrors the Go `operationsToOptions`: the operation whose slug matches
/// `current_slug` (the configured operation) is marked `(Current)` and moved to
/// the front of the list so it is the default selection. When `current_slug` is
/// empty or matches none of the operations, the list keeps its server order with
/// no marker.
pub fn operation_choices(ops: &[Operation], current_slug: &str) -> Vec<OperationChoice> {
    let mut current_idx = None;
    let mut choices: Vec<OperationChoice> = ops
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let is_current = !current_slug.is_empty() && op.slug == current_slug;
            if is_current {
                current_idx = Some(i);
            }
            let label = if is_current {
                format!("{} (Current)", op.name)
            } else {
                op.name.clone()
            };
            OperationChoice {
                label,
                slug: op.slug.clone(),
            }
        })
        .collect();

    // Promote the current operation to the front, preserving the relative order
    // of the rest.
    if let Some(idx) = current_idx {
        let current = choices.remove(idx);
        choices.insert(0, current);
    }

    choices
}

/// Finds the [`OperationChoice`] whose label matches the value the select prompt
/// returned.
pub fn find_operation_by_label<'a>(
    choices: &'a [OperationChoice],
    label: &str,
) -> Option<&'a OperationChoice> {
    choices.iter().find(|choice| choice.label == label)
}

/// Resolves the recording file name: the configured name if it has content,
/// otherwise a generated default.
///
/// Mirrors the Go behaviour where an empty configured name falls back to a
/// generated `recording_*.cast` file (Go used a temp file; we generate a
/// process-random name to the same effect). Surrounding whitespace on a
/// configured name is trimmed.
pub fn resolve_output_file_name(configured: &str) -> String {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        default_output_file_name()
    } else {
        trimmed.to_string()
    }
}

/// Generates a default recording file name, `recording_<random>.cast`.
///
/// The random suffix keeps concurrent / repeated recordings from colliding (the
/// output file is created with `create_new`). Like
/// [`crate::ashirt::ops_tags::random_tag_color`], this seeds off [`RandomState`]
/// rather than pulling in the `rand` crate for a single value.
pub fn default_output_file_name() -> String {
    let r = RandomState::new().build_hasher().finish();
    format!("recording_{r:016x}.cast")
}

/// Constructs the recording output path: `<output_dir>/<slug>/<file_name>`.
///
/// This is the pure path-join half of the recording target; the directory is
/// created and the file opened by [`start_recording`].
pub fn output_path(output_dir: &str, slug: &str, file_name: &str) -> PathBuf {
    Path::new(output_dir).join(slug).join(file_name)
}

/// Creates the recording's parent directory and opens the `.cast` file for
/// writing, returning the open handle.
///
/// Security (CWE-732, gh-143): a terminal recording can contain sensitive
/// session data, so on Unix the directory is created `0700` and the file `0600`
/// — owner-only — with the modes applied at creation time (no chmod-after-create
/// race). On non-Unix targets the platform defaults are used. `create_new`
/// mirrors the Go `O_EXCL`: an existing recording is never clobbered.
///
/// This is factored out of [`start_recording`] (which is TTY-only) so the
/// permission behaviour can be unit-tested without a terminal.
fn create_recording_file(path: &Path) -> Result<File, MenuError> {
    if let Some(parent) = path.parent() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(parent)
            .map_err(|source| MenuError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path).map_err(|source| MenuError::CreateFile {
        path: path.to_path_buf(),
        source,
    })
}

// ---------------------------------------------------------------------------
// Interactive flow. MANUAL/TTY-ONLY: never call from tests or any headless path
// — the prompts and the recorder both require a real terminal.
// ---------------------------------------------------------------------------

/// Runs the main menu loop. This is the entry point the application entrypoint
/// (aterm-8tn.14) calls after configuration is resolved; `client` is built by the
/// entrypoint from the resolved config and shared with the recording flow.
///
/// Operations are fetched once up front (a failure is reported but non-fatal, so
/// the menu still opens and the user can retry via "Refresh operations"). Each
/// loop iteration prompts the main menu and dispatches the chosen action;
/// recording returns here when it (and its post-recording menu) finishes, keeping
/// menu and recording as separate phases. The loop exits on "Quit" or when the
/// user cancels the menu prompt (Esc / Ctrl-C).
pub fn run(config: &mut Config, client: &Client) -> Result<(), MenuError> {
    let mut operations = match list_operations(client) {
        Ok(ops) => ops,
        Err(err) => {
            eprintln!("Unable to retrieve operations list: {err}");
            Vec::new()
        }
    };

    loop {
        let labels = main_menu_labels();
        let selection = match tui::select("What do you want to do?", &labels) {
            Ok(selection) => selection,
            // Treat an aborted menu prompt as "quit" rather than an error.
            Err(TuiError::Cancelled) | Err(TuiError::Interrupted) => break,
            Err(err) => return Err(err.into()),
        };

        match dispatch_main_menu(&selection) {
            Some(MenuAction::StartRecording) => start_recording(config, client, &operations)?,
            Some(MenuAction::RefreshOperations) => match list_operations(client) {
                Ok(ops) => {
                    println!("Updated operations ({} total)", ops.len());
                    operations = ops;
                }
                Err(err) => eprintln!("Unable to retrieve operations list: {err}"),
            },
            Some(MenuAction::Settings) => settings_menu(config)?,
            Some(MenuAction::Quit) => break,
            None => eprintln!("Unrecognized menu selection: {selection}"),
        }
    }

    Ok(())
}

/// Runs the in-app settings menu, letting the user edit and persist
/// configuration: the API URL / access key / secret key / output directory /
/// operation slug / recording shell, plus the startup auto-update toggle
/// (gh-104). Each change is written back via [`Config::write`] so it survives
/// the next launch, mirroring the Go "Update Settings" flow.
///
/// MANUAL/TTY-ONLY: drives `inquire` select/text/password prompts; the
/// label/dispatch/apply logic it relies on ([`settings_menu_labels`],
/// [`dispatch_settings_menu`], [`SettingsField::apply`]) is pure and
/// unit-tested. Backing out (Esc / Ctrl-C) or "Back" returns to the caller.
pub fn settings_menu(config: &mut Config) -> Result<(), MenuError> {
    loop {
        let labels = settings_menu_labels(config);
        let selection = match tui::select("Settings", &labels) {
            Ok(selection) => selection,
            // Backing out of settings returns to the main menu, not an error.
            Err(TuiError::Cancelled) | Err(TuiError::Interrupted) => break,
            Err(err) => return Err(err.into()),
        };

        match dispatch_settings_menu(config, &selection) {
            Some(SettingsAction::EditField(field)) => edit_field(config, field)?,
            Some(SettingsAction::ToggleAutoUpdateCheck) => {
                config.auto_update_check = !config.auto_update_check;
                // Persist immediately so the choice survives a restart.
                match config.write() {
                    Ok(()) => println!(
                        "Automatic update check {}.",
                        if config.auto_update_check {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    ),
                    Err(err) => {
                        // Roll back the in-memory change so the menu keeps showing
                        // the actual persisted state.
                        config.auto_update_check = !config.auto_update_check;
                        eprintln!("Failed to save configuration: {err}");
                    }
                }
            }
            Some(SettingsAction::Back) => break,
            // The select prompt only returns labels we supplied; ignore anything
            // unexpected rather than coercing it into an edit.
            None => eprintln!("Unrecognized settings selection: {selection}"),
        }
    }

    Ok(())
}

/// Prompts for a new value for `field`, persists it via [`Config::write`], and
/// re-validates — rolling the in-memory value back on a write failure and only
/// warning (never blocking the save) when validation finds a problem.
///
/// MANUAL/TTY-ONLY: drives `inquire` text/password prompts. Backing out of the
/// edit prompt (Esc / Ctrl-C) leaves the field unchanged and returns to the
/// settings menu. The secret field is entered via a masked prompt that cannot
/// be pre-seeded, so an empty entry is treated as "leave unchanged" rather than
/// clearing the stored secret (mirroring the first-run wizard).
fn edit_field(config: &mut Config, field: SettingsField) -> Result<(), MenuError> {
    let name = field.display_name();

    let entered = if field.is_secret() {
        match tui::password(name) {
            Ok(value) => value,
            Err(TuiError::Cancelled) | Err(TuiError::Interrupted) => return Ok(()),
            Err(err) => return Err(err.into()),
        }
    } else {
        let current = field.current_value(config).to_string();
        match tui::input_with_default(name, &current) {
            Ok(value) => value,
            Err(TuiError::Cancelled) | Err(TuiError::Interrupted) => return Ok(()),
            Err(err) => return Err(err.into()),
        }
    };

    // A masked secret cannot be pre-filled, so an empty entry means "no change".
    if field.is_secret() && entered.is_empty() {
        println!("{name} left unchanged.");
        return Ok(());
    }

    let previous = field.current_value(config).to_string();
    field.apply(config, entered);
    match config.write() {
        Ok(()) => {
            println!("{name} updated.");
            // Save first, then warn (mirror Go): a bad value is persisted but the
            // user is told about any problems it introduces.
            if let Err(problems) = crate::config_setup::validate(config) {
                eprintln!("Saved, but the configuration still has problems:\n{problems}");
            }
        }
        Err(err) => {
            // Roll back so the menu keeps showing the actual persisted state.
            field.apply(config, previous);
            eprintln!("Failed to save configuration: {err}");
        }
    }

    Ok(())
}

/// Records a single session immediately, then presents the post-recording menu.
///
/// This is the default startup view the entrypoint (aterm-8tn.14) uses when not
/// forced into the main menu — mirroring the Go `MenuViewRecording` start view.
/// Operations are fetched up front; a fetch failure is reported but non-fatal, so
/// the user still reaches the (possibly empty) operation prompt.
pub fn record_once(config: &Config, client: &Client) -> Result<(), MenuError> {
    let operations = match list_operations(client) {
        Ok(ops) => ops,
        Err(err) => {
            eprintln!("Unable to retrieve operations list: {err}");
            Vec::new()
        }
    };
    start_recording(config, client, &operations)
}

/// Prompts for an operation and records a session against it, writing the cast
/// to `<output_dir>/<slug>/<file_name>`, then presents the post-recording upload
/// menu before returning to the caller (the menu loop or the entrypoint).
fn start_recording(
    config: &Config,
    client: &Client,
    operations: &[Operation],
) -> Result<(), MenuError> {
    if operations.is_empty() {
        println!("Unable to record: no operations available (try \"Refresh operations\").");
        return Ok(());
    }

    let choices = operation_choices(operations, &config.operation_slug);
    let labels: Vec<String> = choices.iter().map(|choice| choice.label.clone()).collect();

    let selected = match tui::select("Select an operation", &labels) {
        Ok(selected) => selected,
        // Backing out of operation selection returns to the menu, not an error.
        Err(TuiError::Cancelled) | Err(TuiError::Interrupted) => {
            println!("Cancelled; returning to menu.");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let slug = match find_operation_by_label(&choices, &selected) {
        Some(choice) => choice.slug.clone(),
        // The prompt only returns labels we supplied, so this is unreachable in
        // practice; bail back to the menu rather than panic if it ever happens.
        None => return Ok(()),
    };

    let file_name = resolve_output_file_name(&config.output_file_name);
    let path = output_path(&config.output_dir, &slug, &file_name);

    let file = create_recording_file(&path)?;

    println!("Recording to {}", path.display());
    let code = record_session(&recording_shell(config), BufWriter::new(file))?;
    println!(
        "Recording finished (exit code {code}); saved to {}",
        path.display()
    );

    // Present the post-recording menu (upload / rename / discard). Backing out of
    // it (Esc / Ctrl-C) returns to the caller rather than surfacing as an error.
    match upload_menu::post_recording_menu(client, &slug, &path) {
        Ok(()) => {}
        Err(upload_menu::MenuError::Tui(TuiError::Cancelled | TuiError::Interrupted)) => {}
        Err(err) => return Err(err.into()),
    }

    Ok(())
}

/// Picks the shell to record: the configured shell, falling back to `$SHELL`,
/// then `/bin/sh`. The configured value is normally already seeded from `$SHELL`
/// by [`Config::with_defaults`]; this keeps a sensible last resort.
fn recording_shell(config: &Config) -> String {
    if !config.recording_shell.trim().is_empty() {
        return config.recording_shell.clone();
    }
    match std::env::var("SHELL") {
        Ok(shell) if !shell.trim().is_empty() => shell,
        _ => "/bin/sh".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(slug: &str, name: &str) -> Operation {
        Operation {
            slug: slug.to_string(),
            name: name.to_string(),
            id: 0,
            status: 0,
            num_users: 0,
        }
    }

    #[test]
    fn main_menu_labels_match_items_in_order() {
        let labels = main_menu_labels();
        assert_eq!(
            labels,
            ["Start recording", "Refresh operations", "Settings", "Quit"]
        );
    }

    #[test]
    fn dispatch_main_menu_maps_each_label() {
        assert_eq!(
            dispatch_main_menu("Start recording"),
            Some(MenuAction::StartRecording)
        );
        assert_eq!(
            dispatch_main_menu("Refresh operations"),
            Some(MenuAction::RefreshOperations)
        );
        assert_eq!(dispatch_main_menu("Settings"), Some(MenuAction::Settings));
        assert_eq!(dispatch_main_menu("Quit"), Some(MenuAction::Quit));
    }

    // --- settings menu --------------------------------------------------------

    #[test]
    fn auto_update_toggle_label_reflects_state() {
        assert!(auto_update_toggle_label(true).contains("enabled"));
        assert!(auto_update_toggle_label(false).contains("disabled"));
    }

    #[test]
    fn settings_menu_labels_list_fields_then_toggle_then_back() {
        let cfg = Config::with_defaults();
        let labels = settings_menu_labels(&cfg);

        // One label per editable field, in order, then the toggle, then back.
        let expected_fields: Vec<String> =
            SettingsField::ALL.iter().map(|f| f.label(&cfg)).collect();
        assert_eq!(&labels[..SettingsField::ALL.len()], &expected_fields[..]);
        assert_eq!(
            labels[SettingsField::ALL.len()],
            auto_update_toggle_label(cfg.auto_update_check)
        );
        assert_eq!(labels.last().unwrap(), "Back to main menu");
        assert_eq!(labels.len(), SettingsField::ALL.len() + 2);
    }

    #[test]
    fn settings_menu_labels_track_toggle_state() {
        let mut cfg = Config::with_defaults();
        assert!(
            settings_menu_labels(&cfg).contains(&auto_update_toggle_label(true)),
            "enabled toggle label present"
        );
        cfg.auto_update_check = false;
        assert!(
            settings_menu_labels(&cfg).contains(&auto_update_toggle_label(false)),
            "disabled toggle label present"
        );
    }

    #[test]
    fn settings_field_label_masks_secret_and_marks_empty() {
        let mut cfg = Config::with_defaults();
        cfg.api_url = "https://ashirt.example".to_string();
        cfg.secret_key = "c2VjcmV0".to_string();
        cfg.operation_slug = String::new();

        assert_eq!(
            SettingsField::ApiUrl.label(&cfg),
            "API URL: https://ashirt.example"
        );
        // A set secret shows "(set)" and never echoes the value.
        let secret_label = SettingsField::SecretKey.label(&cfg);
        assert_eq!(secret_label, "Secret Key: (set)");
        assert!(!secret_label.contains("c2VjcmV0"));
        // An empty field is shown as "(not set)".
        assert_eq!(
            SettingsField::OperationSlug.label(&cfg),
            "Operation Slug: (not set)"
        );

        cfg.secret_key = String::new();
        assert_eq!(
            SettingsField::SecretKey.label(&cfg),
            "Secret Key: (not set)"
        );
    }

    #[test]
    fn settings_field_apply_sets_only_its_own_field() {
        let mut cfg = Config::with_defaults();
        SettingsField::ApiUrl.apply(&mut cfg, "https://api.example".to_string());
        SettingsField::AccessKey.apply(&mut cfg, "AKID".to_string());
        SettingsField::SecretKey.apply(&mut cfg, "c2VjcmV0".to_string());
        SettingsField::OutputDir.apply(&mut cfg, "/tmp/out".to_string());
        SettingsField::OperationSlug.apply(&mut cfg, "op".to_string());
        SettingsField::RecordingShell.apply(&mut cfg, "/bin/zsh".to_string());

        assert_eq!(cfg.api_url, "https://api.example");
        assert_eq!(cfg.access_key, "AKID");
        assert_eq!(cfg.secret_key, "c2VjcmV0");
        assert_eq!(cfg.output_dir, "/tmp/out");
        assert_eq!(cfg.operation_slug, "op");
        assert_eq!(cfg.recording_shell, "/bin/zsh");
    }

    #[test]
    fn settings_field_current_value_round_trips_apply() {
        for &field in SettingsField::ALL {
            let mut cfg = Config::with_defaults();
            field.apply(&mut cfg, "edited-value".to_string());
            assert_eq!(field.current_value(&cfg), "edited-value");
        }
    }

    #[test]
    fn only_secret_key_field_is_secret() {
        for &field in SettingsField::ALL {
            assert_eq!(
                field.is_secret(),
                field == SettingsField::SecretKey,
                "{field:?} secret-ness"
            );
        }
    }

    #[test]
    fn dispatch_settings_menu_maps_back_and_toggle() {
        let cfg = Config::with_defaults();
        assert_eq!(
            dispatch_settings_menu(&cfg, "Back to main menu"),
            Some(SettingsAction::Back)
        );
        // The toggle label for the current state maps to the toggle action.
        assert_eq!(
            dispatch_settings_menu(&cfg, &auto_update_toggle_label(cfg.auto_update_check)),
            Some(SettingsAction::ToggleAutoUpdateCheck)
        );
    }

    #[test]
    fn dispatch_settings_menu_maps_each_field_label() {
        let mut cfg = Config::with_defaults();
        cfg.api_url = "https://ashirt.example".to_string();
        cfg.secret_key = "c2VjcmV0".to_string();
        for &field in SettingsField::ALL {
            assert_eq!(
                dispatch_settings_menu(&cfg, &field.label(&cfg)),
                Some(SettingsAction::EditField(field)),
                "{field:?} label dispatch"
            );
        }
    }

    #[test]
    fn dispatch_settings_menu_rejects_unknown_label() {
        let cfg = Config::with_defaults();
        assert_eq!(dispatch_settings_menu(&cfg, "Nope"), None);
        assert_eq!(dispatch_settings_menu(&cfg, ""), None);
    }

    #[test]
    fn every_settings_label_round_trips_through_dispatch() {
        // Guards against drift between the label list and dispatch: every label
        // the menu would show maps to some action.
        let mut cfg = Config::with_defaults();
        cfg.api_url = "https://ashirt.example".to_string();
        cfg.secret_key = "c2VjcmV0".to_string();
        for label in settings_menu_labels(&cfg) {
            assert!(
                dispatch_settings_menu(&cfg, &label).is_some(),
                "label {label:?} did not dispatch"
            );
        }
    }

    #[test]
    fn dispatch_main_menu_rejects_unknown_label() {
        assert_eq!(dispatch_main_menu("Nope"), None);
        assert_eq!(dispatch_main_menu(""), None);
    }

    #[test]
    fn every_menu_label_round_trips_through_dispatch() {
        // Guards against a label/action drift between the constant and dispatch.
        for item in MAIN_MENU {
            assert_eq!(dispatch_main_menu(item.label), Some(item.action));
        }
    }

    #[test]
    fn operation_choices_preserve_order_without_current() {
        let ops = [op("s1", "Alpha"), op("s2", "Beta"), op("s3", "Gamma")];
        let choices = operation_choices(&ops, "");
        let labels: Vec<&str> = choices.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Alpha", "Beta", "Gamma"]);
        let slugs: Vec<&str> = choices.iter().map(|c| c.slug.as_str()).collect();
        assert_eq!(slugs, ["s1", "s2", "s3"]);
    }

    #[test]
    fn operation_choices_promote_and_mark_current() {
        let ops = [op("s1", "Alpha"), op("s2", "Beta"), op("s3", "Gamma")];
        let choices = operation_choices(&ops, "s2");

        // Current operation is first and marked.
        assert_eq!(choices[0].slug, "s2");
        assert_eq!(choices[0].label, "Beta (Current)");
        // The rest keep their original relative order, unmarked.
        let rest: Vec<&str> = choices[1..].iter().map(|c| c.label.as_str()).collect();
        assert_eq!(rest, ["Alpha", "Gamma"]);
    }

    #[test]
    fn operation_choices_unknown_current_slug_is_unmarked() {
        let ops = [op("s1", "Alpha"), op("s2", "Beta")];
        let choices = operation_choices(&ops, "missing");
        let labels: Vec<&str> = choices.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Alpha", "Beta"]);
        assert!(choices.iter().all(|c| !c.label.contains("(Current)")));
    }

    #[test]
    fn operation_choices_empty_input_is_empty() {
        assert!(operation_choices(&[], "s1").is_empty());
    }

    #[test]
    fn find_operation_by_label_round_trips_selection() {
        let ops = [op("s1", "Alpha"), op("s2", "Beta")];
        let choices = operation_choices(&ops, "s2");
        // The label the prompt would return for the current op maps back to its
        // slug, including the "(Current)" marker.
        let found = find_operation_by_label(&choices, "Beta (Current)").expect("label present");
        assert_eq!(found.slug, "s2");
        let found = find_operation_by_label(&choices, "Alpha").expect("label present");
        assert_eq!(found.slug, "s1");
    }

    #[test]
    fn find_operation_by_label_missing_is_none() {
        let choices = operation_choices(&[op("s1", "Alpha")], "");
        assert!(find_operation_by_label(&choices, "Beta").is_none());
    }

    #[test]
    fn resolve_output_file_name_uses_configured_value() {
        assert_eq!(resolve_output_file_name("session.cast"), "session.cast");
    }

    #[test]
    fn resolve_output_file_name_trims_whitespace() {
        assert_eq!(resolve_output_file_name("  session.cast  "), "session.cast");
    }

    #[test]
    fn resolve_output_file_name_defaults_when_empty() {
        let name = resolve_output_file_name("");
        assert!(name.starts_with("recording_"), "got {name:?}");
        assert!(name.ends_with(".cast"), "got {name:?}");
    }

    #[test]
    fn resolve_output_file_name_defaults_when_whitespace_only() {
        let name = resolve_output_file_name("   ");
        assert!(name.starts_with("recording_") && name.ends_with(".cast"));
    }

    #[test]
    fn default_output_file_name_shape_and_variation() {
        let a = default_output_file_name();
        assert!(a.starts_with("recording_"));
        assert!(a.ends_with(".cast"));
        // Overwhelmingly likely to differ across draws if the source is random.
        let mut seen = std::collections::HashSet::new();
        for _ in 0..50 {
            seen.insert(default_output_file_name());
        }
        assert!(seen.len() > 1, "default file name appears constant");
    }

    #[test]
    fn output_path_joins_dir_slug_and_name() {
        let path = output_path("/tmp/out", "op-slug", "rec.cast");
        assert_eq!(path, PathBuf::from("/tmp/out/op-slug/rec.cast"));
    }

    #[test]
    fn output_path_composes_with_resolved_name() {
        // The two pure helpers compose into the documented target layout.
        let name = resolve_output_file_name("my-recording.cast");
        let path = output_path("/recordings", "alpha", &name);
        assert_eq!(path, PathBuf::from("/recordings/alpha/my-recording.cast"));
    }

    // --- recording file permissions (CWE-732, gh-143) -------------------------

    #[cfg(unix)]
    #[test]
    fn create_recording_file_restricts_unix_modes() {
        use std::os::unix::fs::PermissionsExt;

        // Build a unique temp target using the same path layout the recorder
        // uses (<output_dir>/<slug>/<file>) so the test exercises the real
        // create-dir + create-file path.
        let base = std::env::temp_dir().join(format!(
            "aterm-perms-{:016x}",
            RandomState::new().build_hasher().finish()
        ));
        let path = output_path(
            base.to_str().expect("temp dir path is valid UTF-8"),
            "op-slug",
            "recording.cast",
        );

        let file = create_recording_file(&path).expect("create recording file");
        drop(file);

        let dir = path.parent().expect("recording path has a parent");
        let dir_mode = fs::metadata(dir)
            .expect("dir metadata")
            .permissions()
            .mode();
        let file_mode = fs::metadata(&path)
            .expect("file metadata")
            .permissions()
            .mode();

        assert_eq!(dir_mode & 0o777, 0o700, "recording dir must be 0700");
        assert_eq!(file_mode & 0o777, 0o600, "recording file must be 0600");

        // Clean up the whole temp tree regardless of assertion order above.
        fs::remove_dir_all(&base).ok();
    }
}
