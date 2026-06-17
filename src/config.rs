//! Configuration model: built-in defaults, overlaid by the config file, then
//! overridden by CLI flags.
//!
//! This module is the canonical worked example of the project's error-handling
//! convention: a typed [`ConfigError`] enum built with `thiserror`. Callers at
//! the command boundary (see [`crate::app::run`]) lift these into `anyhow`.
//!
//! # Resolution
//!
//! [`Config::load`] resolves a [`Config`] from three sources, each later source
//! overriding the earlier ones **only for the values it actually provides**:
//!
//! 1. built-in [`Config::with_defaults`]
//! 2. the YAML config file (`<config>/aterm/config.yaml`)
//! 3. CLI flags ([`crate::cli::Cli`])
//!
//! A source that does not supply a value must NOT clobber a value set by an
//! earlier one. Concretely:
//!
//! * A field **absent** from the config file keeps the built-in default: the
//!   file is parsed into a partial [`ConfigFile`] whose fields are all optional.
//! * An **absent** CLI flag is skipped. CLI overrides are modelled as
//!   `Option<T>` (see [`crate::cli::Cli`]) so that clap default values never
//!   overwrite earlier values.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cli::Cli;

/// File name of the aterm config inside the platform config directory.
const CONFIG_FILE_NAME: &str = "config.yaml";

/// Errors that can occur while locating, reading, or parsing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// No per-platform configuration directory could be determined.
    #[error("could not determine a configuration directory for this platform")]
    NoConfigDir,

    /// The configuration file existed but could not be read.
    #[error("failed to read config file {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The configuration file could not be parsed as YAML.
    #[error("failed to parse config file {path}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml_ng::Error,
    },

    /// The configuration directory could not be created while writing.
    #[error("failed to create config directory {path}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The configuration file could not be written.
    #[error("failed to write config file {path}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The configuration could not be serialized to YAML.
    #[error("failed to serialize configuration")]
    Serialize {
        #[source]
        source: serde_yaml_ng::Error,
    },
}

/// Resolved aterm configuration.
///
/// The serialized field names match the Go `aterm` `config.yaml` so existing
/// files round-trip unchanged. `output_file_name` is intentionally *not*
/// persisted to the file (it is a per-run value), matching the Go `yaml:"-"`
/// tag; it can still be supplied via a CLI flag.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Schema version of the config file.
    #[serde(rename = "configVersion")]
    pub config_version: i64,
    /// Base URL of the ASHIRT API.
    #[serde(rename = "apiURL")]
    pub api_url: String,
    /// Base directory recordings are written to.
    #[serde(rename = "outputDir")]
    pub output_dir: String,
    /// API access key.
    #[serde(rename = "accessKey")]
    pub access_key: String,
    /// API secret key.
    #[serde(rename = "secretKey")]
    pub secret_key: String,
    /// File name (prefix) to give the recording. Not persisted to the file.
    #[serde(skip)]
    pub output_file_name: String,
    /// Operation slug to record against.
    #[serde(rename = "operationSlug")]
    pub operation_slug: String,
    /// Shell to launch when recording.
    #[serde(rename = "recordingShell")]
    pub recording_shell: String,
}

/// A partial view of the config file: every field is optional so that a field
/// absent from the YAML overlays *nothing*, leaving the built-in default in
/// place.
///
/// `output_file_name` is deliberately omitted: it is CLI/default-only and is
/// never sourced from the file (see [`Config::output_file_name`]).
#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigFile {
    #[serde(rename = "configVersion")]
    config_version: Option<i64>,
    #[serde(rename = "apiURL")]
    api_url: Option<String>,
    #[serde(rename = "outputDir")]
    output_dir: Option<String>,
    #[serde(rename = "accessKey")]
    access_key: Option<String>,
    #[serde(rename = "secretKey")]
    secret_key: Option<String>,
    #[serde(rename = "operationSlug")]
    operation_slug: Option<String>,
    #[serde(rename = "recordingShell")]
    recording_shell: Option<String>,
}

impl Config {
    /// Returns the built-in defaults — the lowest-precedence source.
    ///
    /// Mirrors the Go `TermRecorderConfigWithDefaults`: schema version `1` and
    /// the recording shell taken from the `SHELL` environment variable.
    pub fn with_defaults() -> Self {
        Config {
            config_version: 1,
            recording_shell: std::env::var("SHELL").unwrap_or_default(),
            ..Config::default()
        }
    }

    /// Loads configuration from the real environment: defaults, overlaid by the
    /// config file, then overridden by CLI flags.
    ///
    /// The config file is read from [`config_path`]; a missing file is not an
    /// error (the defaults still apply and CLI flags still override).
    pub fn load(cli: &Cli) -> Result<Self, ConfigError> {
        let path = config_path()?;
        Self::load_from(&path, cli)
    }

    /// Core load, parameterized over the config-file path so it can be exercised
    /// deterministically in tests.
    fn load_from(path: &Path, cli: &Cli) -> Result<Self, ConfigError> {
        let mut cfg = Config::with_defaults();

        // Overlay the config file over the defaults. An absent file is fine; any
        // other read/parse error is fatal.
        if let Some(file) = read_config_file(path)? {
            cfg.overlay_file(file);
        }

        // CLI flags override (highest precedence): apply each flag that is set.
        // `output_file_name` is CLI/default-only and is never sourced from the
        // file, so it is applied here alongside the file-overlaid fields.
        if let Some(operation) = &cli.operation {
            cfg.operation_slug = operation.clone();
        }
        if let Some(shell) = &cli.shell {
            cfg.recording_shell = shell.clone();
        }
        if let Some(name) = &cli.name {
            cfg.output_file_name = name.clone();
        }

        Ok(cfg)
    }

    /// Reads a full [`Config`] from a YAML file, overlaid over the defaults.
    ///
    /// Unlike [`Config::load`] this applies only the defaults and the config
    /// file. A missing file yields the defaults.
    pub fn read_file(path: &Path) -> Result<Self, ConfigError> {
        let mut cfg = Config::with_defaults();
        if let Some(file) = read_config_file(path)? {
            cfg.overlay_file(file);
        }
        Ok(cfg)
    }

    /// Parses a [`Config`] from a YAML document, overlaid over the [`Default`]
    /// values via `#[serde(default)]` on absent fields.
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(text)
    }

    /// Serializes this [`Config`] to a YAML document.
    ///
    /// `output_file_name` is omitted (it is `#[serde(skip)]`), matching the
    /// fields persisted by the Go implementation.
    pub fn to_yaml(&self) -> Result<String, ConfigError> {
        serde_yaml_ng::to_string(self).map_err(|source| ConfigError::Serialize { source })
    }

    /// Writes this [`Config`] as YAML to the platform config path, creating the
    /// config directory if necessary.
    pub fn write(&self) -> Result<(), ConfigError> {
        self.write_to(&config_path()?)
    }

    /// Writes this [`Config`] as YAML to `path`, creating parent directories.
    pub fn write_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let yaml = self.to_yaml()?;
        fs::write(path, yaml).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Overlays the parsed config file onto `self`, replacing only the fields
    /// the file actually provides.
    fn overlay_file(&mut self, file: ConfigFile) {
        if let Some(v) = file.config_version {
            self.config_version = v;
        }
        if let Some(v) = file.api_url {
            self.api_url = v;
        }
        if let Some(v) = file.output_dir {
            self.output_dir = v;
        }
        if let Some(v) = file.access_key {
            self.access_key = v;
        }
        if let Some(v) = file.secret_key {
            self.secret_key = v;
        }
        if let Some(v) = file.operation_slug {
            self.operation_slug = v;
        }
        if let Some(v) = file.recording_shell {
            self.recording_shell = v;
        }
    }
}

/// Human-readable rendering, mirroring the Go `PrintConfigTo` layout. This is
/// for display only — use [`Config::to_yaml`] for serialization.
impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Current Configuration:")?;
        writeln!(f, "\tConfig Version:  {}", self.config_version)?;
        writeln!(f, "\tAPI Host:        {}", self.api_url)?;
        writeln!(f, "\tOutput Base:     {}", self.output_dir)?;
        writeln!(f, "\tAccess Key:      {}", self.access_key)?;
        writeln!(f, "\tSecret Key:      {}", self.secret_key)?;
        writeln!(f, "\tOutput Prefix:   {}", self.output_file_name)?;
        writeln!(f, "\tOperation Slug:  {}", self.operation_slug)?;
        write!(f, "\tRecording Shell: {}", self.recording_shell)
    }
}

/// Reads and parses the config file into a partial [`ConfigFile`].
///
/// Returns `Ok(None)` when the file does not exist (not an error: the defaults
/// still apply and CLI flags still override).
fn read_config_file(path: &Path) -> Result<Option<ConfigFile>, ConfigError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ConfigError::Read {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    serde_yaml_ng::from_str(&text)
        .map(Some)
        .map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

/// Returns the platform configuration directory for aterm.
///
/// A plain `aterm` directory under the per-user config base on every platform:
/// `~/.config/aterm` (Linux), `~/Library/Application Support/aterm` (macOS),
/// `%APPDATA%\aterm` (Windows). This matches the Go `aterm` layout; we use
/// [`directories::BaseDirs`] rather than `ProjectDirs` because the latter would
/// reverse-DNS the directory to `com.ashirt.aterm` on macOS.
pub fn config_dir() -> Result<PathBuf, ConfigError> {
    let base = directories::BaseDirs::new().ok_or(ConfigError::NoConfigDir)?;
    Ok(base.config_dir().join("aterm"))
}

/// Returns the full path to the aterm config file
/// (`<config>/aterm/config.yaml`).
pub fn config_path() -> Result<PathBuf, ConfigError> {
    Ok(config_dir()?.join(CONFIG_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn empty_cli() -> Cli {
        Cli::parse_from(["aterm"])
    }

    /// Allocates a unique temp path so parallel tests never collide (we avoid a
    /// `tempfile` dependency and `Math.random`-style nondeterminism).
    fn temp_path(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "aterm-cfg-test-{}-{}-{}.yaml",
            tag,
            std::process::id(),
            n
        ))
    }

    #[test]
    fn empty_yaml_overlays_over_defaults() {
        let cfg = Config::from_yaml("{}").expect("empty map parses");
        assert!(cfg.api_url.is_empty());
    }

    /// REQUIRED: defaults, file, and CLI each set the same field to distinct
    /// values; the CLI value must win end-to-end (defaults < file < cli).
    #[test]
    fn precedence_cli_overrides_file_overrides_default() {
        let path = temp_path("precedence");
        // The config file sets recording_shell (and operation_slug, exercised
        // below).
        fs::write(
            &path,
            "recordingShell: file-shell\noperationSlug: file-op\napiURL: file-url\n",
        )
        .expect("write temp config");

        // The CLI overrides recording_shell only.
        let cli = Cli::parse_from(["aterm", "--shell", "cli-shell"]);

        let cfg = Config::load_from(&path, &cli).expect("load");

        // The contested field: file and CLI both set it; CLI wins.
        assert_eq!(cfg.recording_shell, "cli-shell", "cli must win for shell");
        // Only the file set this one; it survives untouched (cli absent).
        assert_eq!(
            cfg.operation_slug, "file-op",
            "file value survives when cli absent"
        );
        // Only the file set this one; it survives untouched.
        assert_eq!(
            cfg.api_url, "file-url",
            "file value survives when cli absent"
        );
        // Nothing touched this one; the built-in default survives.
        assert_eq!(
            cfg.config_version, 1,
            "default survives when nothing overrides"
        );

        fs::remove_file(&path).ok();
    }

    /// `output_file_name` is CLI/default-only: a YAML key for it is ignored, but
    /// `--name` sets it.
    #[test]
    fn output_file_name_is_cli_only_never_file_sourced() {
        let path = temp_path("output-name-asymmetry");
        // Even if the file tries to set output_file_name (under any plausible
        // key), it must not be sourced from the file.
        fs::write(
            &path,
            "outputFileName: from-file\noutput_file_name: from-file\n",
        )
        .expect("write temp config");

        // No --name flag: output_file_name stays the default (empty).
        let cfg = Config::load_from(&path, &empty_cli()).expect("load");
        assert!(
            cfg.output_file_name.is_empty(),
            "file must not source output_file_name"
        );

        // --name sets it.
        let cli = Cli::parse_from(["aterm", "--name", "cli-name"]);
        let cfg = Config::load_from(&path, &cli).expect("load");
        assert_eq!(
            cfg.output_file_name, "cli-name",
            "--name sets output_file_name"
        );

        fs::remove_file(&path).ok();
    }

    /// A missing config file is not an error; defaults/cli still apply.
    #[test]
    fn missing_file_is_not_an_error() {
        let path = temp_path("missing");
        let cfg = Config::load_from(&path, &empty_cli()).expect("missing file is ok");
        assert_eq!(cfg.config_version, 1);
    }

    /// REQUIRED: YAML round-trip — write a config to disk, read it back, and
    /// confirm equality.
    #[test]
    fn yaml_round_trip() {
        let original = Config {
            config_version: 1,
            api_url: "https://ashirt.example".to_string(),
            output_dir: "/tmp/recordings".to_string(),
            access_key: "AKID".to_string(),
            secret_key: "c2VjcmV0".to_string(),
            // Not persisted (serde skip); left default so equality holds.
            output_file_name: String::new(),
            operation_slug: "op-slug".to_string(),
            recording_shell: "/bin/zsh".to_string(),
        };

        let path = temp_path("round-trip");
        original.write_to(&path).expect("write config");
        let read_back = Config::read_file(&path).expect("read config");

        assert_eq!(original, read_back);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn output_file_name_is_not_persisted() {
        let cfg = Config {
            output_file_name: "should-not-appear".to_string(),
            ..Config::default()
        };
        let yaml = cfg.to_yaml().expect("serialize");
        assert!(!yaml.contains("should-not-appear"));
        // The field key must be absent too (only `outputDir` should carry
        // "output").
        assert!(!yaml.contains("outputFileName"));
        assert!(!yaml.contains("output_file_name"));
    }

    #[test]
    fn config_path_ends_with_expected_segments() {
        // Don't assert the platform prefix, just the trailing structure.
        let path = config_path().expect("config path");
        assert!(path.ends_with("aterm/config.yaml") || path.ends_with("aterm\\config.yaml"));
    }
}
