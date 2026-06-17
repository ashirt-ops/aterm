//! Layered configuration model: defaults < file < env < cli.
//!
//! This module is the canonical worked example of the project's error-handling
//! convention: a typed [`ConfigError`] enum built with `thiserror`. Callers at
//! the command boundary (see [`crate::app::run`]) lift these into `anyhow`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
}

/// Resolved aterm configuration.
///
/// `#[serde(default)]` lets partial config files layer cleanly over the
/// [`Default`] values (the "defaults" layer).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Base URL of the ASHIRT API.
    pub api_url: String,
    /// API access key.
    pub access_key: String,
    /// API secret key.
    pub secret_key: String,
    /// Operation slug to record against.
    pub operation_slug: String,
    /// Shell to launch when recording.
    pub recording_shell: String,
}

impl Config {
    /// Loads configuration, layering defaults < file < env < cli.
    // TODO(aterm-8tn.4): implement the full layered load (read [`config_dir`],
    // parse via [`Config::from_yaml`], overlay env vars, then overlay CLI flags).
    pub fn load() -> Result<Self, ConfigError> {
        todo!("aterm-8tn.4: layered config loading (defaults < file < env < cli)")
    }

    /// Parses a [`Config`] from a YAML document (the "file" layer).
    pub fn from_yaml(text: &str) -> Result<Self, serde_yaml_ng::Error> {
        serde_yaml_ng::from_str(text)
    }
}

/// Returns the platform configuration directory for aterm
/// (e.g. `~/.config/aterm` on Linux).
pub fn config_dir() -> Result<PathBuf, ConfigError> {
    let dirs =
        directories::ProjectDirs::from("com", "ashirt", "aterm").ok_or(ConfigError::NoConfigDir)?;
    Ok(dirs.config_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_yaml_layers_over_defaults() {
        let cfg = Config::from_yaml("{}").expect("empty map parses");
        assert!(cfg.api_url.is_empty());
    }
}
