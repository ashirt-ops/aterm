//! Config validation, ASHIRT import, and the first-run setup wizard.
//!
//! This module replaces the Go config validation (`config.ValidateConfig`) and
//! the `appdialogs/first_run_prompt.go` wizard. It is split into three concerns,
//! and the first two are deliberately written as **pure functions** so they can
//! be unit-tested without a terminal:
//!
//! 1. [`validate`] — checks a resolved [`Config`] and aggregates *every* problem
//!    it finds (not just the first), mirroring the Go `go-multierror` behaviour.
//! 2. [`import_ashirt`] / [`AshirtImport`] — reads the ASHIRT desktop app's
//!    `config.json` and seeds empty aterm fields from it.
//! 3. [`wizard_seed`] — computes the starting values for the wizard given the
//!    `--reset` / `--reset-hard` flags.
//!
//! Only [`run_first_run_wizard`] actually touches a terminal. It is **manual /
//! TTY-only**: it drives `inquire` prompts (raw mode) and must never run in
//! tests or any headless/default code path. The headless gate is what keeps the
//! crate testable — see the module-level note in [`crate::tui`].

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::ashirt::signing;
use crate::config::{Config, ConfigError};

/// A single configuration problem found by [`validate`].
///
/// Messages mirror the Go `cmd/aterm/config/errors.go` strings so user-facing
/// output is unchanged from the implementation this replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    /// `accessKey` is empty.
    AccessKeyNotSet,
    /// `secretKey` is empty.
    SecretKeyNotSet,
    /// `secretKey` is set but is not valid base64 (cannot be decoded).
    SecretKeyMalformed,
    /// `apiURL` could not be parsed as a URL.
    ApiUrlUnparsable,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            ValidationError::AccessKeyNotSet => "Access Key has not been specified",
            ValidationError::SecretKeyNotSet => "Secret Key has not been specified",
            ValidationError::SecretKeyMalformed => "Secret Key is malformed",
            ValidationError::ApiUrlUnparsable => "Unable to parse API URL",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for ValidationError {}

/// The aggregate of every [`ValidationError`] found in a single [`validate`]
/// pass. Non-empty by construction: [`validate`] only returns `Err` when at
/// least one problem exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationErrors(Vec<ValidationError>);

impl ValidationErrors {
    /// The individual problems, in the order they were discovered.
    pub fn errors(&self) -> &[ValidationError] {
        &self.0
    }
}

impl fmt::Display for ValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} configuration problem(s):", self.0.len())?;
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "  * {e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationErrors {}

/// Validates a resolved [`Config`], aggregating **all** problems found.
///
/// The checks mirror the Go `ValidateConfig`:
///
/// * `accessKey` must be set.
/// * `secretKey` must be set **and** base64-decodable (decoded via the same
///   [`signing::decode_secret_key`] the request signer uses, so the two never
///   disagree about what a valid secret is).
/// * `apiURL` must parse as a URL.
///
/// Returns `Ok(())` when the config is usable, or `Err(ValidationErrors)`
/// carrying every problem (never just the first).
pub fn validate(cfg: &Config) -> Result<(), ValidationErrors> {
    let mut problems = Vec::new();

    if cfg.access_key.is_empty() {
        problems.push(ValidationError::AccessKeyNotSet);
    }

    if cfg.secret_key.is_empty() {
        problems.push(ValidationError::SecretKeyNotSet);
    } else if signing::decode_secret_key(&cfg.secret_key).is_err() {
        problems.push(ValidationError::SecretKeyMalformed);
    }

    if url::Url::parse(&cfg.api_url).is_err() {
        problems.push(ValidationError::ApiUrlUnparsable);
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(ValidationErrors(problems))
    }
}

/// Values importable from the ASHIRT desktop application's `config.json`.
///
/// Only the fields aterm can seed from are modelled; any other keys in the file
/// are ignored. Field names match the ASHIRT desktop config JSON, including the
/// `evidenceRepo` key that maps onto aterm's `outputDir` (mirroring the Go
/// `TermRecorderConfigOverrides` json tags).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AshirtImport {
    #[serde(rename = "apiURL")]
    pub api_url: Option<String>,
    #[serde(rename = "accessKey")]
    pub access_key: Option<String>,
    #[serde(rename = "secretKey")]
    pub secret_key: Option<String>,
    /// The ASHIRT desktop "evidence repo" directory, used to seed `outputDir`.
    #[serde(rename = "evidenceRepo")]
    pub output_dir: Option<String>,
}

impl AshirtImport {
    /// Parses an [`AshirtImport`] from the ASHIRT desktop `config.json` text.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// Seeds the importable fields of `cfg` from this import, **only** filling
    /// fields that are currently empty. An imported value that is itself empty
    /// (or absent) never overwrites anything.
    ///
    /// This is the "seed defaults from another ASHIRT application" behaviour of
    /// the Go first-run flow, expressed as a pure mutation so it is testable.
    pub fn seed_empty(&self, cfg: &mut Config) {
        seed_if_empty(&mut cfg.api_url, self.api_url.as_deref());
        seed_if_empty(&mut cfg.access_key, self.access_key.as_deref());
        seed_if_empty(&mut cfg.secret_key, self.secret_key.as_deref());
        seed_if_empty(&mut cfg.output_dir, self.output_dir.as_deref());
    }
}

/// Fills `target` with `value` only when `target` is empty and `value` is a
/// non-empty string.
fn seed_if_empty(target: &mut String, value: Option<&str>) {
    if target.is_empty() {
        if let Some(v) = value {
            if !v.is_empty() {
                *target = v.to_string();
            }
        }
    }
}

/// Path to the ASHIRT desktop application's config (`<config>/ashirt/config.json`).
///
/// Uses the same per-user config base as aterm's own config (see
/// [`crate::config::config_dir`]) but under the sibling `ashirt` directory,
/// matching the Go `ASHIRTConfigPath`.
pub fn ashirt_config_path() -> Result<PathBuf, ConfigError> {
    let base = directories::BaseDirs::new().ok_or(ConfigError::NoConfigDir)?;
    Ok(base.config_dir().join("ashirt").join("config.json"))
}

/// Reads and parses the ASHIRT desktop `config.json` at `path`.
///
/// Returns `Ok(None)` when the file does not exist (importing is best-effort:
/// an absent ASHIRT install is not an error). Read or parse failures are
/// surfaced so the caller can decide whether to warn.
pub fn read_ashirt_import(path: &Path) -> Result<Option<AshirtImport>, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    // An ASHIRT config that fails to parse is treated as "nothing to import"
    // rather than fatal: the wizard can still collect values by hand.
    Ok(AshirtImport::from_json(&text).ok())
}

/// Convenience: import from the ASHIRT desktop config and seed `cfg`'s empty
/// fields. A missing or unparsable ASHIRT config simply leaves `cfg` unchanged.
pub fn import_ashirt(cfg: &mut Config) -> Result<(), ConfigError> {
    if let Some(import) = read_ashirt_import(&ashirt_config_path()?)? {
        import.seed_empty(cfg);
    }
    Ok(())
}

/// Computes the starting values the first-run wizard pre-fills.
///
/// * `--reset` (soft): seed from the `existing` config so the user can tweak
///   individual values — every saved value is offered as the default.
/// * `--reset-hard`: ignore the existing config entirely and start from the
///   built-in defaults (empty credentials), forcing fresh entry.
/// * first run (no existing config): pass `existing = None`; behaves like a
///   hard reset (defaults only).
///
/// This is pure so the reset-vs-reset-hard merge is unit-tested directly.
pub fn wizard_seed(existing: Option<&Config>, reset_hard: bool) -> Config {
    match (existing, reset_hard) {
        (Some(cfg), false) => cfg.clone(),
        _ => Config::with_defaults(),
    }
}

// ---------------------------------------------------------------------------
// Interactive wizard. MANUAL / TTY-ONLY — never call from tests or any headless
// code path. `inquire` prompts enable raw mode and will hang or error without a
// real terminal. All decision logic above is pure and tested; this function is
// only the thin shell that wires those values into prompts.
// ---------------------------------------------------------------------------

/// Runs the first-run setup wizard, returning the collected [`Config`].
///
/// `seed` supplies the default for each prompt (see [`wizard_seed`] and
/// [`AshirtImport::seed_empty`]); pressing enter accepts the bracketed default.
/// The collected values are layered onto `seed` so non-prompted fields (config
/// version, recording shell, operation slug) are preserved.
///
/// MANUAL-ONLY: requires a TTY; do not invoke from tests.
pub fn run_first_run_wizard(seed: Config) -> Result<Config, crate::tui::TuiError> {
    use crate::tui;

    println!(
        "Hi and welcome to the ASHIRT Terminal Recorder.\n\n\
         Before we begin recording, we need to configure this application.\n\
         If the value in [brackets] looks good, simply press enter to accept it."
    );

    let mut cfg = seed;

    // `input_with_default` pre-fills the seed value, so an empty seed simply
    // means the user types from scratch and a non-empty seed is accepted with
    // enter — exactly the "value in [brackets]" behaviour of the Go wizard.
    cfg.api_url = tui::input_with_default("ASHIRT API URL", &cfg.api_url)?;

    println!(
        "I need your credentials to talk to the ASHIRT servers. \
         You can generate a new key from your account settings on the ASHIRT website."
    );
    cfg.access_key = tui::input_with_default("Access key", &cfg.access_key)?;
    // Secret key is masked, so we cannot pre-fill it; an empty entry keeps any
    // existing seeded value rather than clearing it.
    let entered_secret = tui::password("Secret key")?;
    if !entered_secret.is_empty() {
        cfg.secret_key = entered_secret;
    }

    cfg.output_dir = tui::input_with_default("Recording output directory", &cfg.output_dir)?;

    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A config that passes [`validate`] cleanly. The secret is valid base64.
    fn valid_config() -> Config {
        Config {
            api_url: "https://ashirt.example".to_string(),
            access_key: "AKID".to_string(),
            secret_key: "c2VjcmV0".to_string(), // base64("secret")
            ..Config::with_defaults()
        }
    }

    #[test]
    fn validate_accepts_a_good_config() {
        assert!(validate(&valid_config()).is_ok());
    }

    /// REQUIRED: missing/invalid fields are aggregated — every problem at once,
    /// not just the first.
    #[test]
    fn validate_aggregates_all_problems() {
        let cfg = Config {
            api_url: String::new(),    // unparsable (empty)
            access_key: String::new(), // not set
            secret_key: String::new(), // not set
            ..Config::with_defaults()
        };
        let err = validate(&cfg).expect_err("an empty config must fail validation");
        let problems = err.errors();
        assert!(problems.contains(&ValidationError::AccessKeyNotSet));
        assert!(problems.contains(&ValidationError::SecretKeyNotSet));
        assert!(problems.contains(&ValidationError::ApiUrlUnparsable));
        assert_eq!(problems.len(), 3, "all three problems aggregated");
    }

    #[test]
    fn validate_flags_malformed_secret_not_missing() {
        let cfg = Config {
            secret_key: "not valid base64!!!".to_string(),
            ..valid_config()
        };
        let err = validate(&cfg).expect_err("malformed secret must fail");
        assert_eq!(err.errors(), &[ValidationError::SecretKeyMalformed]);
        // A malformed secret is distinct from a missing one.
        assert!(!err.errors().contains(&ValidationError::SecretKeyNotSet));
    }

    #[test]
    fn validate_decode_matches_signer() {
        // The secret the signer's golden vector uses must validate cleanly,
        // proving validation and signing share one notion of "valid secret".
        let cfg = Config {
            secret_key: "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=".to_string(),
            ..valid_config()
        };
        assert!(validate(&cfg).is_ok());
    }

    #[test]
    fn validation_errors_display_lists_each_problem() {
        let cfg = Config {
            api_url: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            ..Config::with_defaults()
        };
        let err = validate(&cfg).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("Access Key has not been specified"));
        assert!(text.contains("Secret Key has not been specified"));
        assert!(text.contains("Unable to parse API URL"));
    }

    /// REQUIRED: parse a sample ASHIRT desktop config.json and confirm the
    /// `evidenceRepo`->`output_dir` remap plus the credential fields.
    #[test]
    fn ashirt_json_import_parses_sample() {
        let json = r#"
            {
              "apiURL": "https://ashirt.example/api",
              "accessKey": "ashirt-access",
              "secretKey": "YXNoaXJ0LXNlY3JldA==",
              "evidenceRepo": "/home/user/evidence",
              "someUnknownKey": 42
            }
        "#;
        let import = AshirtImport::from_json(json).expect("sample ASHIRT config parses");
        assert_eq!(
            import.api_url.as_deref(),
            Some("https://ashirt.example/api")
        );
        assert_eq!(import.access_key.as_deref(), Some("ashirt-access"));
        assert_eq!(import.secret_key.as_deref(), Some("YXNoaXJ0LXNlY3JldA=="));
        assert_eq!(import.output_dir.as_deref(), Some("/home/user/evidence"));
    }

    #[test]
    fn import_seeds_only_empty_fields() {
        let import = AshirtImport {
            api_url: Some("https://imported.example".to_string()),
            access_key: Some("imported-access".to_string()),
            secret_key: Some("aW1wb3J0ZWQ=".to_string()),
            output_dir: Some("/imported/dir".to_string()),
        };

        // api_url already set: import must NOT clobber it. The remaining fields
        // are cleared so the "seed only empty fields" behaviour is exercised —
        // note `with_defaults` now pre-fills `output_dir` with the home dir, so
        // it is emptied here to test seeding into an empty field.
        let mut cfg = Config {
            api_url: "https://existing.example".to_string(),
            output_dir: String::new(),
            ..Config::with_defaults()
        };
        import.seed_empty(&mut cfg);

        assert_eq!(
            cfg.api_url, "https://existing.example",
            "existing value kept"
        );
        assert_eq!(cfg.access_key, "imported-access", "empty field seeded");
        assert_eq!(cfg.secret_key, "aW1wb3J0ZWQ=");
        assert_eq!(cfg.output_dir, "/imported/dir");
    }

    #[test]
    fn import_ignores_empty_and_absent_values() {
        let import = AshirtImport {
            api_url: Some(String::new()), // present but empty: not a seed
            access_key: None,             // absent: not a seed
            secret_key: Some("c2VjcmV0".to_string()),
            output_dir: None,
        };
        let mut cfg = Config::with_defaults();
        import.seed_empty(&mut cfg);

        assert!(cfg.api_url.is_empty(), "empty import value does not seed");
        assert!(
            cfg.access_key.is_empty(),
            "absent import value does not seed"
        );
        assert_eq!(cfg.secret_key, "c2VjcmV0");
    }

    #[test]
    fn read_ashirt_import_missing_file_is_none() {
        let path = std::env::temp_dir().join(format!(
            "aterm-ashirt-missing-{}-{}.json",
            std::process::id(),
            "x"
        ));
        let got = read_ashirt_import(&path).expect("missing file is not an error");
        assert!(got.is_none());
    }

    /// REQUIRED: reset vs reset-hard merge behaviour.
    #[test]
    fn reset_seeds_from_existing_hard_reset_does_not() {
        let existing = Config {
            api_url: "https://saved.example".to_string(),
            access_key: "saved-access".to_string(),
            secret_key: "c2F2ZWQ=".to_string(),
            output_dir: "/saved/dir".to_string(),
            operation_slug: "saved-op".to_string(),
            ..Config::with_defaults()
        };

        // Soft reset: existing values carried into the seed.
        let soft = wizard_seed(Some(&existing), false);
        assert_eq!(soft.api_url, "https://saved.example");
        assert_eq!(soft.access_key, "saved-access");
        assert_eq!(soft.output_dir, "/saved/dir");

        // Hard reset: existing ignored, back to defaults (empty creds). The
        // saved output_dir is dropped in favour of the built-in default (the
        // home directory), not the saved "/saved/dir".
        let hard = wizard_seed(Some(&existing), true);
        assert!(hard.api_url.is_empty());
        assert!(hard.access_key.is_empty());
        assert!(hard.secret_key.is_empty());
        assert_ne!(
            hard.output_dir, "/saved/dir",
            "hard reset drops the saved output_dir"
        );
        assert_eq!(hard.output_dir, Config::with_defaults().output_dir);
        assert_eq!(hard, Config::with_defaults());
    }

    #[test]
    fn wizard_seed_first_run_uses_defaults() {
        let seed = wizard_seed(None, false);
        assert_eq!(seed, Config::with_defaults());
    }

    #[test]
    fn ashirt_config_path_ends_with_expected_segments() {
        let path = ashirt_config_path().expect("ashirt config path");
        assert!(
            path.ends_with("ashirt/config.json") || path.ends_with("ashirt\\config.json"),
            "unexpected ashirt config path: {}",
            path.display()
        );
    }
}
