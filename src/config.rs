//! Layered configuration model: defaults < file < env < cli.
//!
//! This module is the canonical worked example of the project's error-handling
//! convention: a typed [`ConfigError`] enum built with `thiserror`. Callers at
//! the command boundary (see [`crate::app::run`]) lift these into `anyhow`.
//!
//! # Precedence
//!
//! [`Config::load`] resolves a [`Config`] by layering four sources, each later
//! source overriding the earlier ones **only for the values it actually
//! provides**:
//!
//! 1. built-in [`Config::with_defaults`]
//! 2. the YAML config file (`<config>/aterm/config.yaml`)
//! 3. environment variables (`ASHIRT_TERM_RECORDER_*`)
//! 4. CLI flags ([`crate::cli::Cli`])
//!
//! A value that a layer does not supply must NOT clobber a value set by an
//! earlier layer. Concretely:
//!
//! * An **unset** environment variable and an **absent** CLI flag are skipped.
//!   CLI overrides are modelled as `Option<T>` (see [`crate::cli::Cli`]) so that
//!   clap default values never overwrite file/env values.
//! * An environment variable set to the **empty string** counts as *not
//!   provided*: it will not overwrite a value from the file layer. This mirrors
//!   the Go implementation it replaces.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::cli::Cli;

/// Environment variable names for the env layer (`ASHIRT_TERM_RECORDER_*`).
///
/// These mirror the names produced by the Go `envconfig` integration
/// (`ASHIRT_TERM_RECORDER` prefix, `split_words` casing) so existing
/// deployments keep working.
mod env_vars {
    pub const CONFIG_VERSION: &str = "ASHIRT_TERM_RECORDER_CONFIG_VERSION";
    pub const API_URL: &str = "ASHIRT_TERM_RECORDER_API_URL";
    pub const OUTPUT_DIR: &str = "ASHIRT_TERM_RECORDER_OUTPUT_DIR";
    pub const ACCESS_KEY: &str = "ASHIRT_TERM_RECORDER_ACCESS_KEY";
    pub const SECRET_KEY: &str = "ASHIRT_TERM_RECORDER_SECRET_KEY";
    pub const OUTPUT_FILE_NAME: &str = "ASHIRT_TERM_RECORDER_OUTPUT_FILE_NAME";
    pub const OPERATION_SLUG: &str = "ASHIRT_TERM_RECORDER_OPERATION_SLUG";
    pub const RECORDING_SHELL: &str = "ASHIRT_TERM_RECORDER_RECORDING_SHELL";
}

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

    /// An environment variable held a value that could not be parsed.
    #[error("environment variable {var} has an invalid value {value:?}")]
    EnvInvalid { var: String, value: String },
}

/// Resolved aterm configuration.
///
/// The serialized field names match the Go `aterm` `config.yaml` so existing
/// files round-trip unchanged. `output_file_name` is intentionally *not*
/// persisted to the file (it is a per-run value), matching the Go `yaml:"-"`
/// tag; it can still be supplied via the env / CLI layers.
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

/// A partial configuration layer: every field is optional so that a layer can
/// override *only* the values it provides, leaving the rest untouched.
#[derive(Debug, Clone, Default, Deserialize)]
struct ConfigLayer {
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
    #[serde(skip)]
    output_file_name: Option<String>,
    #[serde(rename = "operationSlug")]
    operation_slug: Option<String>,
    #[serde(rename = "recordingShell")]
    recording_shell: Option<String>,
}

impl Config {
    /// Returns the built-in defaults — the lowest-precedence layer.
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

    /// Loads configuration from the real environment, layering
    /// defaults < file < env < cli.
    ///
    /// The config file is read from [`config_path`]; a missing file is not an
    /// error (the defaults/env/cli layers still apply).
    pub fn load(cli: &Cli) -> Result<Self, ConfigError> {
        let path = config_path()?;
        Self::load_from(&path, |key| std::env::var(key).ok(), cli)
    }

    /// Core layered load, parameterized over the config-file path and the
    /// environment source so it can be exercised deterministically in tests
    /// (without mutating the process-wide environment).
    ///
    /// `env_get` returns the raw value of an environment variable, or `None`
    /// when it is unset.
    fn load_from(
        path: &Path,
        env_get: impl Fn(&str) -> Option<String>,
        cli: &Cli,
    ) -> Result<Self, ConfigError> {
        let mut cfg = Config::with_defaults();

        // File layer: a partial config overlaid over the defaults. An absent
        // file is fine; any other read/parse error is fatal.
        if let Some(layer) = file_layer(path)? {
            cfg.apply(layer);
        }

        // Env layer.
        cfg.apply(env_layer(&env_get)?);

        // CLI layer (highest precedence).
        cfg.apply(cli_layer(cli));

        Ok(cfg)
    }

    /// Reads a full [`Config`] from a YAML file, overlaid over the defaults.
    ///
    /// Unlike [`Config::load`] this applies only the defaults and file layers.
    /// A missing file yields the defaults.
    pub fn read_file(path: &Path) -> Result<Self, ConfigError> {
        let mut cfg = Config::with_defaults();
        if let Some(layer) = file_layer(path)? {
            cfg.apply(layer);
        }
        Ok(cfg)
    }

    /// Parses a [`Config`] from a YAML document (the "file" layer), overlaid
    /// over the [`Default`] values via `#[serde(default)]` on absent fields.
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

    /// Overlays a [`ConfigLayer`] onto `self`, replacing only the fields the
    /// layer provides.
    fn apply(&mut self, layer: ConfigLayer) {
        if let Some(v) = layer.config_version {
            self.config_version = v;
        }
        if let Some(v) = layer.api_url {
            self.api_url = v;
        }
        if let Some(v) = layer.output_dir {
            self.output_dir = v;
        }
        if let Some(v) = layer.access_key {
            self.access_key = v;
        }
        if let Some(v) = layer.secret_key {
            self.secret_key = v;
        }
        if let Some(v) = layer.output_file_name {
            self.output_file_name = v;
        }
        if let Some(v) = layer.operation_slug {
            self.operation_slug = v;
        }
        if let Some(v) = layer.recording_shell {
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

/// Reads and parses the config file into a partial layer.
///
/// Returns `Ok(None)` when the file does not exist (not an error: the other
/// layers still apply).
fn file_layer(path: &Path) -> Result<Option<ConfigLayer>, ConfigError> {
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

/// Builds the env layer from an environment source.
///
/// An unset variable, or one set to the empty string, is treated as *not
/// provided* and yields `None` for that field.
fn env_layer(env_get: &impl Fn(&str) -> Option<String>) -> Result<ConfigLayer, ConfigError> {
    // Treat unset OR empty as "not provided".
    let get = |key: &str| env_get(key).filter(|v| !v.is_empty());

    let config_version = match get(env_vars::CONFIG_VERSION) {
        Some(raw) => Some(raw.parse::<i64>().map_err(|_| ConfigError::EnvInvalid {
            var: env_vars::CONFIG_VERSION.to_string(),
            value: raw,
        })?),
        None => None,
    };

    Ok(ConfigLayer {
        config_version,
        api_url: get(env_vars::API_URL),
        output_dir: get(env_vars::OUTPUT_DIR),
        access_key: get(env_vars::ACCESS_KEY),
        secret_key: get(env_vars::SECRET_KEY),
        output_file_name: get(env_vars::OUTPUT_FILE_NAME),
        operation_slug: get(env_vars::OPERATION_SLUG),
        recording_shell: get(env_vars::RECORDING_SHELL),
    })
}

/// Builds the CLI layer. Absent flags (`None`) do not override earlier layers.
fn cli_layer(cli: &Cli) -> ConfigLayer {
    ConfigLayer {
        operation_slug: cli.operation.clone(),
        recording_shell: cli.shell.clone(),
        output_file_name: cli.name.clone(),
        ..ConfigLayer::default()
    }
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
    fn empty_yaml_layers_over_defaults() {
        let cfg = Config::from_yaml("{}").expect("empty map parses");
        assert!(cfg.api_url.is_empty());
    }

    /// REQUIRED: all four layers set the same field to four distinct values;
    /// the CLI value must win end-to-end (defaults < file < env < cli).
    #[test]
    fn four_layer_precedence_cli_wins() {
        let path = temp_path("precedence");
        // File layer sets recording_shell (and operation_slug, exercised below).
        fs::write(
            &path,
            "recordingShell: file-shell\noperationSlug: file-op\napiURL: file-url\n",
        )
        .expect("write temp config");

        // Env layer overrides recording_shell (and operation_slug).
        let env = |key: &str| match key {
            env_vars::RECORDING_SHELL => Some("env-shell".to_string()),
            env_vars::OPERATION_SLUG => Some("env-op".to_string()),
            _ => None,
        };

        // CLI layer overrides recording_shell only.
        let cli = Cli::parse_from(["aterm", "--shell", "cli-shell"]);

        let cfg = Config::load_from(&path, env, &cli).expect("layered load");

        // The contested field: every layer set it; CLI wins.
        assert_eq!(cfg.recording_shell, "cli-shell", "cli must win for shell");
        // Only file+env set this one; env wins over file (cli absent).
        assert_eq!(
            cfg.operation_slug, "env-op",
            "env beats file when cli absent"
        );
        // Only the file set this one; it survives untouched.
        assert_eq!(
            cfg.api_url, "file-url",
            "file value survives when env/cli absent"
        );
        // No layer touched this one; the built-in default survives.
        assert_eq!(
            cfg.config_version, 1,
            "default survives when no layer overrides"
        );

        fs::remove_file(&path).ok();
    }

    /// An env var set to the empty string must NOT overwrite a file value.
    #[test]
    fn empty_env_does_not_override_file() {
        let path = temp_path("empty-env");
        fs::write(&path, "recordingShell: file-shell\n").expect("write temp config");

        let env = |key: &str| match key {
            env_vars::RECORDING_SHELL => Some(String::new()), // empty == not provided
            _ => None,
        };

        let cfg = Config::load_from(&path, env, &empty_cli()).expect("layered load");
        assert_eq!(cfg.recording_shell, "file-shell");

        fs::remove_file(&path).ok();
    }

    /// A missing config file is not an error; defaults/env/cli still apply.
    #[test]
    fn missing_file_is_not_an_error() {
        let path = temp_path("missing");
        let cfg = Config::load_from(&path, |_| None, &empty_cli()).expect("missing file is ok");
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
