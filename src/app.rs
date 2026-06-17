//! Application orchestration — the entrypoint wiring called by `main`.
//!
//! This replaces the Go `cmd/aterm/climain.go`: parse CLI -> first-run wizard if
//! the config is missing or a reset was requested -> resolve config -> build the
//! ASHIRT client -> validate -> update check -> either the main menu or an
//! immediate recording, with the post-recording upload menu wired in afterwards.
//!
//! This is the `anyhow` BOUNDARY: typed module errors (`thiserror`) bubble up
//! here and get human-facing context via [`anyhow::Context`]. Nothing below this
//! layer should use `anyhow`.
//!
//! # Headless gate
//!
//! [`run`] is interactive (menus, recorder, network) and must never run in tests
//! or without a TTY. The two decisions that drive control flow — which startup
//! view to show ([`startup_view`]) and whether to launch the first-run wizard
//! ([`should_run_first_run`]) — are factored into pure functions and unit-tested
//! below; the interactive wiring itself is never exercised by the test suite.

use anyhow::{Context, Result};

use crate::ashirt::http::{Client, HttpError};
use crate::ashirt::signing::Credentials;
use crate::cli::Cli;
use crate::config::{self, Config};
use crate::config_setup;
use crate::menu;
use crate::update::{self, SemVer, UpgradeResult};

/// GitHub repository queried for the update check. The Go build injected these
/// via ldflags; here they are the canonical upstream coordinates.
const CODE_OWNER: &str = "ashirt-ops";
const CODE_REPO: &str = "aterm";

/// The view aterm starts in once configuration is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupView {
    /// The interactive main menu (`menu::run`).
    Menu,
    /// Record a session immediately (the default Go action).
    Record,
}

/// Whether the startup update check should run or be skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateCheck {
    /// Perform the network update check.
    Check,
    /// Skip the check entirely (no network call).
    Skip,
}

/// Decides whether to run the startup update check from the resolved config.
///
/// Gates the single [`notify_update`] seam on the `autoUpdateCheck` option
/// (aterm-8tn.20 / gh-104): enabled => check; disabled => skip, making no
/// network call. Kept pure so the gate decision is unit-tested directly.
fn update_check(auto_update_check: bool) -> UpdateCheck {
    if auto_update_check {
        UpdateCheck::Check
    } else {
        UpdateCheck::Skip
    }
}

/// Decides the startup view from the resolved flags and validation outcome.
///
/// Mirrors the Go climain: `--menu` (`ShowMenu`) **or** a failed validation force
/// the main menu so the user can react; otherwise recording starts immediately.
fn startup_view(menu_flag: bool, validation_ok: bool) -> StartupView {
    if menu_flag || !validation_ok {
        StartupView::Menu
    } else {
        StartupView::Record
    }
}

/// Decides whether the first-run setup wizard should run.
///
/// Mirrors the Go climain trigger: a missing config file, a soft `--reset`, or a
/// hard `--reset-hard` each force first-run setup. `--reset-hard` additionally
/// ignores any existing config (handled by the seed computation in [`run_first_run`]).
fn should_run_first_run(config_exists: bool, reset: bool, reset_hard: bool) -> bool {
    reset || reset_hard || !config_exists
}

/// Runs aterm end to end.
pub fn run(cli: Cli) -> Result<()> {
    // First-run setup if the config is missing or a reset was requested. This
    // writes a fresh config file before we resolve the effective configuration.
    if should_run_first_run(config_file_exists(), cli.reset, cli.reset_hard) {
        run_first_run(&cli).context("running first-run configuration")?;
    }

    // Resolve configuration: built-in defaults < config file < CLI flags. After a
    // first run this re-reads the freshly written file, so CLI flags still win.
    // Mutable because the in-app settings menu can edit and persist it.
    let mut config = Config::load(&cli).context("loading aterm configuration")?;

    // Build the ASHIRT API client from the resolved config (base URL +
    // credentials). Mirrors the Go `network.SetBaseURL` / `SetAccessKey`.
    let client = build_client(&config).context("building ASHIRT API client")?;

    // Validate the resolved config. On failure, surface every problem and fall
    // back to the main menu instead of recording.
    let validation_ok = match config_setup::validate(&config) {
        Ok(()) => true,
        Err(errs) => {
            eprintln!("{errs}");
            false
        }
    };

    // Update check, gated on the `autoUpdateCheck` option (gh-104). A single call
    // site / single decision point: when disabled we skip it entirely (no
    // network). Development builds skip inside `notify_update` regardless.
    match update_check(config.auto_update_check) {
        UpdateCheck::Check => notify_update(),
        UpdateCheck::Skip => {}
    }

    // `--print-config` prints the resolved config and exits, after validation and
    // the update notice (matching the Go ordering).
    if cli.print_config {
        println!("{config}");
        return Ok(());
    }

    match startup_view(cli.menu, validation_ok) {
        StartupView::Menu => menu::run(&mut config, &client).context("main menu")?,
        StartupView::Record => menu::record_once(&config, &client).context("recording session")?,
    }

    Ok(())
}

/// Runs the first-run setup wizard and writes the resulting configuration.
///
/// The wizard is seeded per the reset flags (mirroring the Go `FirstRun`):
/// `--reset-hard` ignores any saved config and starts from defaults; `--reset`
/// (and a plain first run) seed from the existing config when one is present.
/// Empty fields are then seeded from the ASHIRT desktop config (best-effort).
fn run_first_run(cli: &Cli) -> Result<()> {
    let existing = if cli.reset_hard {
        None
    } else {
        config::config_path()
            .ok()
            .filter(|path| path.exists())
            .and_then(|path| Config::read_file(&path).ok())
    };

    let mut seed = config_setup::wizard_seed(existing.as_ref(), cli.reset_hard);

    // Seeding from another ASHIRT application is best-effort: a missing or
    // unreadable desktop config simply leaves the seed unchanged.
    let _ = config_setup::import_ashirt(&mut seed);

    let config = config_setup::run_first_run_wizard(seed).context("first-run setup wizard")?;
    config.write().context("writing configuration")?;
    Ok(())
}

/// Builds the ASHIRT API client from the resolved configuration.
fn build_client(config: &Config) -> Result<Client, HttpError> {
    let creds = Credentials {
        access_key: config.access_key.clone(),
        secret_key: config.secret_key.clone(),
    };
    Client::new(&config.api_url, creds)
}

/// Returns whether the aterm config file currently exists. A failure to even
/// determine the path is treated as "missing" so first-run setup still triggers.
fn config_file_exists() -> bool {
    config::config_path()
        .map(|path| path.exists())
        .unwrap_or(false)
}

/// Checks GitHub for a newer release and notifies the user.
///
/// Single call site for the update check (see [`run`]). Development builds
/// (version `0.0.0`) skip the network check, mirroring the Go `NotifyUpdate`
/// guard for unversioned/development binaries.
fn notify_update() {
    let current = update::parse_version(env!("CARGO_PKG_VERSION"));
    if current == SemVer::default() {
        println!("This appears to be a development release; skipping update check.");
        return;
    }

    match update::check_version(CODE_OWNER, CODE_REPO, &current) {
        Ok(result) if result.has_upgrade() => report_upgrades(&result),
        Ok(_) => {}
        Err(err) => eprintln!("Unable to check for updates: {err}"),
    }
}

/// Prints the available upgrades (newest of each kind) to stdout.
fn report_upgrades(result: &UpgradeResult) {
    println!("There is an update available.");
    for (kind, upgrade) in [
        ("major", &result.major),
        ("minor", &result.minor),
        ("patch", &result.patch),
    ] {
        if let Some(upgrade) = upgrade {
            println!("  {kind} upgrade: {} ({})", upgrade.version, upgrade.tag);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- update-check gate decision -------------------------------------------

    #[test]
    fn update_check_runs_when_enabled() {
        assert_eq!(update_check(true), UpdateCheck::Check);
    }

    #[test]
    fn update_check_skips_when_disabled() {
        assert_eq!(update_check(false), UpdateCheck::Skip);
    }

    /// The gate reads the resolved config: a default config checks; disabling the
    /// option skips.
    #[test]
    fn update_check_follows_config_flag() {
        let mut cfg = Config::with_defaults();
        assert_eq!(update_check(cfg.auto_update_check), UpdateCheck::Check);
        cfg.auto_update_check = false;
        assert_eq!(update_check(cfg.auto_update_check), UpdateCheck::Skip);
    }

    // --- startup-view decision ------------------------------------------------

    #[test]
    fn startup_view_records_when_valid_and_menu_not_requested() {
        assert_eq!(startup_view(false, true), StartupView::Record);
    }

    #[test]
    fn startup_view_shows_menu_when_menu_flag_set() {
        // The menu flag forces the menu even when the config is valid.
        assert_eq!(startup_view(true, true), StartupView::Menu);
    }

    #[test]
    fn startup_view_shows_menu_when_validation_failed() {
        // A failed validation forces the menu regardless of the menu flag.
        assert_eq!(startup_view(false, false), StartupView::Menu);
        assert_eq!(startup_view(true, false), StartupView::Menu);
    }

    // --- first-run trigger decision -------------------------------------------

    #[test]
    fn first_run_triggers_when_config_missing() {
        assert!(should_run_first_run(false, false, false));
    }

    #[test]
    fn first_run_skipped_when_config_present_and_no_reset() {
        assert!(!should_run_first_run(true, false, false));
    }

    #[test]
    fn first_run_triggers_on_soft_reset_even_with_config_present() {
        assert!(should_run_first_run(true, true, false));
    }

    #[test]
    fn first_run_triggers_on_hard_reset_even_with_config_present() {
        assert!(should_run_first_run(true, false, true));
    }

    #[test]
    fn first_run_triggers_when_missing_regardless_of_reset() {
        // Every combination with a missing config triggers first-run.
        for reset in [false, true] {
            for reset_hard in [false, true] {
                assert!(should_run_first_run(false, reset, reset_hard));
            }
        }
    }
}
