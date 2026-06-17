//! Terminal UI prompt wrappers.
//!
//! Thin, reusable wrappers over interactive prompts so the rest of the app never
//! depends on the prompt library directly. This is the Rust replacement for the
//! Go `promptui`-based `dialog/` package plus the `fancy/` styling helpers.
//!
//! Built on [`inquire`] (over its default `crossterm` backend). The wrappers
//! cover the primitives the app needs: a searchable single-select, a
//! multiselect, a yes/no confirm, a free-text prompt, and a masked password
//! prompt, plus a handful of pure styling helpers.
//!
//! # Headless note
//!
//! Anything that actually drives a terminal (`inquire`'s `.prompt()`, raw mode,
//! etc.) requires a TTY and will fail or hang without one — so it can never run
//! in CI or under the orchestrator. To keep this module testable, ALL decision
//! logic (option filtering / fuzzy match, input validation, styling/formatting)
//! is factored into standalone pure functions that are unit-tested below; the
//! interactive wrappers are thin shells that wire those pure functions into
//! `inquire`. A runnable demo of the interactive prompts lives in
//! `examples/tui_demo.rs` and is **manual-only** (run it yourself in a real
//! terminal — it is never exercised by the test suite).

use inquire::validator::Validation;
use inquire::{Confirm, InquireError, MultiSelect, Password, PasswordDisplayMode, Select, Text};
use thiserror::Error;

/// Errors produced by interactive prompts.
#[derive(Debug, Error)]
pub enum TuiError {
    /// The user aborted the prompt (Esc / Ctrl-C / EOF).
    #[error("prompt cancelled")]
    Cancelled,

    /// The prompt could not run (e.g. no TTY) or failed mid-flight.
    #[error("prompt failed: {0}")]
    Prompt(String),
}

impl From<InquireError> for TuiError {
    fn from(err: InquireError) -> Self {
        match err {
            // Treat Esc and Ctrl-C uniformly as a user-initiated cancel; this
            // mirrors the Go `dialog` package collapsing ErrInterrupt/ErrEOF
            // into a single "kill signal".
            InquireError::OperationCanceled | InquireError::OperationInterrupted => {
                TuiError::Cancelled
            }
            other => TuiError::Prompt(other.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Pure logic (headless-testable). The interactive wrappers below delegate here.
// ---------------------------------------------------------------------------

/// Case-insensitive substring score used to filter select/multiselect options.
///
/// Returns `Some(score)` when `input` appears (ignoring case) somewhere in
/// `label`, and `None` otherwise. `inquire` keeps any option scoring `Some` and
/// drops those scoring `None`; a constant score preserves the original option
/// order among matches. This intentionally reproduces the Go
/// `SearcherContainsCI` behaviour (plain substring match) rather than the fuzzy
/// matching `inquire` ships by default, so menus filter predictably.
pub fn score_contains_ci(input: &str, label: &str) -> Option<i64> {
    if input.is_empty() {
        return Some(0);
    }
    if label.to_lowercase().contains(&input.to_lowercase()) {
        Some(0)
    } else {
        None
    }
}

/// Whether `input` is acceptable as a "required" free-text answer.
///
/// Mirrors the validation we attach to required prompts: input is rejected if it
/// is empty or only whitespace.
pub fn is_non_empty(input: &str) -> bool {
    !input.trim().is_empty()
}

// ---------------------------------------------------------------------------
// Styling helpers (pure ANSI string decoration).
// ---------------------------------------------------------------------------

const ANSI_RESET: &str = "\x1b[0m";

fn wrap(code: &str, s: &str) -> String {
    format!("\x1b[{code}m{s}{ANSI_RESET}")
}

/// Wraps `s` in ANSI bold.
pub fn bold(s: &str) -> String {
    wrap("1", s)
}

/// Wraps `s` in ANSI green.
pub fn green(s: &str) -> String {
    wrap("32", s)
}

/// Wraps `s` in ANSI red.
pub fn red(s: &str) -> String {
    wrap("31", s)
}

/// A green check mark, used to denote success / a "yes" choice.
pub fn green_check() -> String {
    green("\u{2713}") // ✓
}

/// A red cross, used to denote failure / a "no" choice.
pub fn red_cross() -> String {
    red("\u{2717}") // ✗
}

// ---------------------------------------------------------------------------
// Interactive wrappers. MANUAL/TTY-ONLY: never call these from tests or any
// default (headless) code path — `.prompt()` enables raw mode and will hang or
// error without a real terminal.
// ---------------------------------------------------------------------------

/// Prompts the user to choose one of `options` via a searchable select list.
///
/// Filtering is case-insensitive substring matching (see [`score_contains_ci`]).
pub fn select(message: &str, options: &[String]) -> Result<String, TuiError> {
    Select::new(message, options.to_vec())
        .with_scorer(&|input, _opt, value, _idx| score_contains_ci(input, value))
        .prompt()
        .map_err(TuiError::from)
}

/// Prompts the user to choose zero or more of `options` via a searchable
/// multiselect list. Returns the selected labels in display order.
pub fn multiselect(message: &str, options: &[String]) -> Result<Vec<String>, TuiError> {
    MultiSelect::new(message, options.to_vec())
        .with_scorer(&|input, _opt, value, _idx| score_contains_ci(input, value))
        .prompt()
        .map_err(TuiError::from)
}

/// Asks a yes/no question, returning `true` for yes. `default` is the answer
/// applied when the user submits an empty response.
pub fn confirm(message: &str, default: bool) -> Result<bool, TuiError> {
    Confirm::new(message)
        .with_default(default)
        .prompt()
        .map_err(TuiError::from)
}

/// Prompts the user for a line of free-text input.
pub fn input(message: &str) -> Result<String, TuiError> {
    Text::new(message).prompt().map_err(TuiError::from)
}

/// Prompts for free-text input, pre-filling `default` (used when the user
/// submits an empty response).
pub fn input_with_default(message: &str, default: &str) -> Result<String, TuiError> {
    Text::new(message)
        .with_default(default)
        .prompt()
        .map_err(TuiError::from)
}

/// Prompts for free-text input that must be non-empty (see [`is_non_empty`]).
pub fn required_input(message: &str) -> Result<String, TuiError> {
    Text::new(message)
        .with_validator(|s: &str| {
            if is_non_empty(s) {
                Ok(Validation::Valid)
            } else {
                Ok(Validation::Invalid("a value is required".into()))
            }
        })
        .prompt()
        .map_err(TuiError::from)
}

/// Prompts for a secret value. Input is masked and no confirmation re-entry is
/// required (this is an entry prompt, not a "set a new password" flow).
pub fn password(message: &str) -> Result<String, TuiError> {
    Password::new(message)
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation()
        .prompt()
        .map_err(TuiError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_matches_case_insensitively() {
        assert!(score_contains_ci("ash", "ASHIRT").is_some());
        assert!(score_contains_ci("HIRT", "ashirt").is_some());
        assert!(score_contains_ci("shi", "ashirt").is_some());
    }

    #[test]
    fn score_rejects_non_substring() {
        assert!(score_contains_ci("xyz", "ashirt").is_none());
        assert!(score_contains_ci("ashirtx", "ashirt").is_none());
    }

    #[test]
    fn empty_input_matches_everything() {
        assert!(score_contains_ci("", "anything").is_some());
        assert!(score_contains_ci("", "").is_some());
    }

    #[test]
    fn score_filter_keeps_only_matching_options() {
        let options = ["San Antonio", "San Diego", "Dallas", "San Jose", "Austin"];
        let kept: Vec<&str> = options
            .iter()
            .copied()
            .filter(|label| score_contains_ci("san", label).is_some())
            .collect();
        assert_eq!(kept, ["San Antonio", "San Diego", "San Jose"]);
    }

    #[test]
    fn non_empty_validation() {
        assert!(is_non_empty("hello"));
        assert!(is_non_empty("  x  "));
        assert!(!is_non_empty(""));
        assert!(!is_non_empty("   "));
        assert!(!is_non_empty("\t\n"));
    }

    #[test]
    fn styling_wraps_and_resets() {
        assert_eq!(bold("hi"), "\x1b[1mhi\x1b[0m");
        assert_eq!(green("ok"), "\x1b[32mok\x1b[0m");
        assert_eq!(red("no"), "\x1b[31mno\x1b[0m");
    }

    #[test]
    fn glyph_helpers_are_colored() {
        let check = green_check();
        assert!(check.contains('\u{2713}'));
        assert!(check.starts_with("\x1b[32m"));
        assert!(check.ends_with(ANSI_RESET));

        let cross = red_cross();
        assert!(cross.contains('\u{2717}'));
        assert!(cross.starts_with("\x1b[31m"));
        assert!(cross.ends_with(ANSI_RESET));
    }
}
