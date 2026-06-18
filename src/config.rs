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
    /// Whether to check GitHub for a newer release on startup.
    ///
    /// Positive sense: `true` means the startup update check runs. It DEFAULTS
    /// to enabled — but note the serde gotcha: a plain `bool` absent from the
    /// YAML deserializes to `false`. The enabled default is therefore set in
    /// [`Config::with_defaults`] and preserved through the partial
    /// [`ConfigFile`] overlay, so a field absent from the file keeps `true`. The
    /// in-app settings menu lets the user toggle it off (see
    /// [`crate::menu::settings_menu`]); [`crate::app`] gates the check on it.
    #[serde(rename = "autoUpdateCheck")]
    pub auto_update_check: bool,
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
    #[serde(rename = "autoUpdateCheck")]
    auto_update_check: Option<bool>,
}

impl Config {
    /// Returns the built-in defaults — the lowest-precedence source.
    ///
    /// Mirrors the Go `TermRecorderConfigWithDefaults`: schema version `1`, the
    /// recording shell taken from the `SHELL` environment variable, and the
    /// output base defaulting to aterm's per-user XDG data directory.
    pub fn with_defaults() -> Self {
        Config {
            config_version: 1,
            recording_shell: std::env::var("SHELL").unwrap_or_default(),
            // Recordings are *data*, so base them under aterm's per-user XDG
            // data directory by default (`~/.local/share/aterm` on Linux,
            // honouring `$XDG_DATA_HOME`; the platform data dir on Windows;
            // XDG-style on macOS for consistency with `config_dir`). A config
            // with no file/CLI value still gets a sensible, absolute output
            // base. If no data directory can be determined (very unlikely), this
            // falls back to the empty string, which the file/CLI can still
            // override.
            output_dir: default_output_dir(),
            // Default ENABLED. The derived `Default` gives `false` for a bool, so
            // the enabled default lives here and is preserved by the file overlay
            // (absent in file => stays `true`).
            auto_update_check: true,
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
    ///
    /// The file holds the ASHIRT API access/secret keys, so on Unix the config
    /// directory is created `0700` and the file `0600` to keep other local
    /// users from reading the credentials (CWE-312). On non-Unix targets the
    /// platform defaults apply.
    pub fn write_to(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            create_config_dir(parent).map_err(|source| ConfigError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let yaml = self.to_yaml()?;
        write_config_file(path, yaml.as_bytes()).map_err(|source| ConfigError::Write {
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
        if let Some(v) = file.auto_update_check {
            self.auto_update_check = v;
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
        writeln!(f, "\tRecording Shell: {}", self.recording_shell)?;
        write!(
            f,
            "\tUpdate Check:    {}",
            if self.auto_update_check {
                "enabled"
            } else {
                "disabled"
            }
        )
    }
}

/// Creates the config directory (and any missing parents).
///
/// On Unix the directory is created with mode `0700` so other local users
/// cannot traverse into it and read the credential file; an already-existing
/// directory keeps its current mode (matching `create_dir_all`). On non-Unix
/// targets this is a plain recursive create with the platform defaults.
fn create_config_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

/// Writes `contents` to the config file, truncating any existing file.
///
/// On Unix the file is created with mode `0600`; because that mode only applies
/// to a freshly created file, an existing file is additionally tightened to
/// `0600` via `set_permissions` so credentials are never left world-readable.
/// On non-Unix targets this is a plain `fs::write` with the platform defaults.
fn write_config_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(contents)
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents)
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
            });
        }
    };
    serde_yaml_ng::from_str(&text)
        .map(Some)
        .map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

/// Resolves aterm's XDG-style config base: `$XDG_CONFIG_HOME` when set (and
/// non-empty), otherwise `$HOME/.config`.
///
/// Kept as a pure function over its inputs so the macOS resolution can be
/// unit-tested on Linux CI without touching the real environment. Returns `None`
/// only when there is neither an XDG override nor a resolvable home directory.
#[cfg(any(target_os = "macos", test))]
fn xdg_config_base(xdg_config_home: Option<&str>, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(x) = xdg_config_home {
        if !x.is_empty() {
            return Some(PathBuf::from(x));
        }
    }
    home.map(|h| h.join(".config"))
}

/// Resolves aterm's XDG-style data base: `$XDG_DATA_HOME` when set (and
/// non-empty), otherwise `$HOME/.local/share`.
///
/// Mirrors [`xdg_config_base`]; kept as a pure function over its inputs so the
/// macOS resolution can be unit-tested on Linux CI without touching the real
/// environment. Returns `None` only when there is neither an XDG override nor a
/// resolvable home directory.
#[cfg(any(target_os = "macos", test))]
fn xdg_data_base(xdg_data_home: Option<&str>, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(x) = xdg_data_home {
        if !x.is_empty() {
            return Some(PathBuf::from(x));
        }
    }
    home.map(|h| h.join(".local/share"))
}

/// Returns the default `output_dir`: aterm's per-user XDG data directory
/// (`~/.local/share/aterm` on Linux, honouring `$XDG_DATA_HOME`; the platform
/// data dir on Windows). Recordings are data, so they live under the data dir
/// rather than the config dir or the home directory.
///
/// macOS uses the XDG-style location (`$XDG_DATA_HOME` else
/// `~/.local/share/aterm`) for consistency with [`config_dir`] rather than the
/// native `~/Library/Application Support/aterm`.
///
/// Returns the empty string when no data directory can be determined (e.g.
/// neither `$XDG_DATA_HOME` nor a home directory on macOS); the file/CLI can
/// still set `output_dir` in that case.
fn default_output_dir() -> String {
    #[cfg(target_os = "macos")]
    {
        let xdg = std::env::var("XDG_DATA_HOME").ok();
        let home = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf());
        xdg_data_base(xdg.as_deref(), home.as_deref())
            .map(|base| base.join("aterm").display().to_string())
            .unwrap_or_default()
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux resolves to `~/.local/share/aterm` via the XDG data base dir
        // (honouring `$XDG_DATA_HOME`), and Windows to its native data dir.
        directories::BaseDirs::new()
            .map(|b| b.data_dir().join("aterm").display().to_string())
            .unwrap_or_default()
    }
}

/// Returns the platform configuration directory for aterm.
///
/// A plain `aterm` directory under the per-user config base:
/// `~/.config/aterm` (Linux and macOS), `%APPDATA%\aterm` (Windows).
///
/// macOS intentionally uses the XDG-style `~/.config/aterm` location rather than
/// the [`directories`]-crate native `~/Library/Application Support/aterm`, for
/// CLI ergonomics and Linux/macOS consistency. This switch is deliberate and
/// **unmigrated**: the old Application Support path is *not* read or copied, so
/// existing macOS users re-run first-run setup. (The ASHIRT desktop-app import
/// keeps its own Application Support lookup — see
/// [`crate::config_setup::ashirt_config_path`].)
///
/// On Linux/Windows we use [`directories::BaseDirs`] rather than `ProjectDirs`
/// because the latter would reverse-DNS the directory to `com.ashirt.aterm` on
/// macOS; this matches the Go `aterm` layout.
pub fn config_dir() -> Result<PathBuf, ConfigError> {
    #[cfg(target_os = "macos")]
    {
        let xdg = std::env::var("XDG_CONFIG_HOME").ok();
        let home = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf());
        let base =
            xdg_config_base(xdg.as_deref(), home.as_deref()).ok_or(ConfigError::NoConfigDir)?;
        Ok(base.join("aterm"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        // Linux already resolves to `~/.config/aterm` via the XDG base dir, and
        // Windows to `%APPDATA%\aterm`; both keep their current behaviour.
        let base = directories::BaseDirs::new().ok_or(ConfigError::NoConfigDir)?;
        Ok(base.config_dir().join("aterm"))
    }
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
            auto_update_check: true,
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

    /// The built-in default for the update check is ENABLED.
    #[test]
    fn auto_update_check_defaults_enabled() {
        assert!(Config::with_defaults().auto_update_check);
    }

    /// The built-in default for `output_dir` is a non-empty, sensible base —
    /// aterm's per-user XDG data directory — so recordings have an output base
    /// even with no file/CLI value. Recordings are data, so this is the data dir
    /// (`~/.local/share/aterm` on Linux, honouring `$XDG_DATA_HOME`), NOT the
    /// home directory. (CI/dev environments always have a resolvable data dir.)
    #[test]
    fn output_dir_defaults_to_xdg_data_dir() {
        let cfg = Config::with_defaults();
        assert!(!cfg.output_dir.is_empty(), "output_dir default must be set");
        assert!(
            cfg.output_dir.ends_with("aterm"),
            "output_dir default must be under an aterm/ directory, got {}",
            cfg.output_dir
        );

        // On non-macOS the default is the `directories` data dir joined with
        // `aterm` (Linux: the XDG data dir honouring `$XDG_DATA_HOME`).
        #[cfg(not(target_os = "macos"))]
        {
            let expected = directories::BaseDirs::new()
                .map(|b| b.data_dir().join("aterm").display().to_string())
                .expect("a data directory is resolvable in the test environment");
            assert_eq!(
                cfg.output_dir, expected,
                "output_dir default must be the XDG data dir joined with aterm"
            );
            // On Linux with no `$XDG_DATA_HOME` override this is the canonical
            // `~/.local/share/aterm`. This literal is Linux-only: Windows (also
            // matched by `not(macos)`) resolves the data dir to `%APPDATA%`, so
            // gate it to Linux. (Skip the literal check if the test environment
            // injected an override.)
            #[cfg(target_os = "linux")]
            if std::env::var_os("XDG_DATA_HOME").is_none() {
                assert!(
                    cfg.output_dir
                        .replace('\\', "/")
                        .ends_with(".local/share/aterm"),
                    "default must be ~/.local/share/aterm, got {}",
                    cfg.output_dir
                );
            }
        }

        // On macOS the `directories` data dir is `Application Support`, but
        // since .31 aterm's own `default_output_dir` uses the XDG data base
        // (`$XDG_DATA_HOME` else `$HOME/.local/share`) — so the default also
        // resolves to `~/.local/share/aterm`. Mirror the Linux assertion here.
        // (Skip the literal check if the test environment injected an override.)
        #[cfg(target_os = "macos")]
        if std::env::var_os("XDG_DATA_HOME").is_none() {
            assert!(
                cfg.output_dir
                    .replace('\\', "/")
                    .ends_with(".local/share/aterm"),
                "default must be ~/.local/share/aterm, got {}",
                cfg.output_dir
            );
        }
    }

    /// A config-file `outputDir` overrides the built-in data-directory default.
    #[test]
    fn output_dir_file_overrides_default() {
        let path = temp_path("output-dir-override");
        fs::write(&path, "outputDir: /tmp/custom-recordings\n").expect("write temp config");

        let cfg = Config::load_from(&path, &empty_cli()).expect("load");
        assert_eq!(
            cfg.output_dir, "/tmp/custom-recordings",
            "file outputDir must override the data-directory default"
        );

        fs::remove_file(&path).ok();
    }

    /// REQUIRED (serde gotcha): a config file that omits `autoUpdateCheck` must
    /// keep the enabled default rather than deserializing the bool to `false`.
    /// This exercises the real load path (defaults + partial file overlay).
    #[test]
    fn absent_auto_update_field_stays_enabled() {
        let path = temp_path("auto-update-absent");
        // A non-empty file that does NOT mention autoUpdateCheck at all.
        fs::write(&path, "apiURL: https://ashirt.example\n").expect("write temp config");

        let cfg = Config::read_file(&path).expect("read config");
        assert!(
            cfg.auto_update_check,
            "absent autoUpdateCheck must stay enabled"
        );
        // And via the full load path (defaults < file < cli) too.
        let cfg = Config::load_from(&path, &empty_cli()).expect("load");
        assert!(cfg.auto_update_check, "absent => enabled through load_from");

        fs::remove_file(&path).ok();
    }

    /// The field can be turned OFF from the config file, overriding the default.
    #[test]
    fn auto_update_check_disabled_via_file() {
        let path = temp_path("auto-update-off");
        fs::write(&path, "autoUpdateCheck: false\n").expect("write temp config");

        let cfg = Config::read_file(&path).expect("read config");
        assert!(
            !cfg.auto_update_check,
            "autoUpdateCheck: false must disable the check"
        );

        fs::remove_file(&path).ok();
    }

    /// The field is persisted to the config file under its camelCase key.
    #[test]
    fn auto_update_check_is_persisted() {
        let yaml = Config::with_defaults().to_yaml().expect("serialize");
        assert!(yaml.contains("autoUpdateCheck"), "got {yaml}");
    }

    /// REQUIRED (security, CWE-312): on Unix `write_to` must create the config
    /// directory `0700` and the credential file `0600` so other local users
    /// cannot read the ASHIRT API keys.
    #[cfg(unix)]
    #[test]
    fn write_to_restricts_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("aterm-cfg-perm-{}-{}", std::process::id(), n));
        let path = dir.join("config.yaml");

        Config::with_defaults()
            .write_to(&path)
            .expect("write config");

        let file_mode = fs::metadata(&path)
            .expect("file metadata")
            .permissions()
            .mode();
        assert_eq!(file_mode & 0o777, 0o600, "config file must be 0600");

        let dir_mode = fs::metadata(&dir)
            .expect("dir metadata")
            .permissions()
            .mode();
        assert_eq!(dir_mode & 0o777, 0o700, "config dir must be 0700");

        fs::remove_file(&path).ok();
        fs::remove_dir(&dir).ok();
    }

    #[test]
    fn config_path_ends_with_expected_segments() {
        // Don't assert the platform prefix, just the trailing structure.
        let path = config_path().expect("config path");
        assert!(path.ends_with("aterm/config.yaml") || path.ends_with("aterm\\config.yaml"));
    }

    /// macOS uses the XDG base, honouring `$XDG_CONFIG_HOME` when set. Exercised
    /// through the pure [`xdg_config_base`] helper so it runs on Linux CI too;
    /// `config_dir()` joins `aterm` onto this base on macOS.
    #[test]
    fn macos_config_base_honors_xdg_config_home() {
        let home = PathBuf::from("/Users/alice");
        let base = xdg_config_base(Some("/custom/xdg"), Some(&home)).expect("base resolves");
        assert_eq!(base, PathBuf::from("/custom/xdg"));
        assert_eq!(base.join("aterm"), PathBuf::from("/custom/xdg/aterm"));
    }

    /// With no `$XDG_CONFIG_HOME`, macOS falls back to `$HOME/.config` — i.e.
    /// `config_dir()` resolves to `~/.config/aterm`.
    #[test]
    fn macos_config_base_falls_back_to_home_dot_config() {
        let home = PathBuf::from("/Users/alice");
        let base = xdg_config_base(None, Some(&home)).expect("base resolves");
        assert_eq!(
            base.join("aterm"),
            PathBuf::from("/Users/alice/.config/aterm")
        );
    }

    /// An empty `$XDG_CONFIG_HOME` is treated as unset (per the XDG spec) and
    /// falls back to `$HOME/.config`.
    #[test]
    fn macos_config_base_empty_xdg_falls_back_to_home() {
        let home = PathBuf::from("/Users/alice");
        let base = xdg_config_base(Some(""), Some(&home)).expect("base resolves");
        assert_eq!(base, PathBuf::from("/Users/alice/.config"));
    }

    /// With neither an XDG override nor a home directory, no base can be
    /// resolved (surfaces as `ConfigError::NoConfigDir` in `config_dir`).
    #[test]
    fn macos_config_base_none_when_no_home_no_xdg() {
        assert!(xdg_config_base(None, None).is_none());
    }

    /// The data base honours `$XDG_DATA_HOME` when set — `default_output_dir`
    /// joins `aterm` onto this base on macOS. Exercised through the pure
    /// [`xdg_data_base`] helper so it runs on Linux CI too (and lets us inject
    /// the override without mutating the process environment).
    #[test]
    fn xdg_data_base_honors_xdg_data_home() {
        let home = PathBuf::from("/Users/alice");
        let base = xdg_data_base(Some("/custom/data"), Some(&home)).expect("base resolves");
        assert_eq!(base, PathBuf::from("/custom/data"));
        assert_eq!(base.join("aterm"), PathBuf::from("/custom/data/aterm"));
    }

    /// With no `$XDG_DATA_HOME`, the data base falls back to
    /// `$HOME/.local/share` — i.e. the default `output_dir` resolves to
    /// `~/.local/share/aterm`.
    #[test]
    fn xdg_data_base_falls_back_to_home_local_share() {
        let home = PathBuf::from("/Users/alice");
        let base = xdg_data_base(None, Some(&home)).expect("base resolves");
        assert_eq!(
            base.join("aterm"),
            PathBuf::from("/Users/alice/.local/share/aterm")
        );
    }

    /// An empty `$XDG_DATA_HOME` is treated as unset (per the XDG spec) and
    /// falls back to `$HOME/.local/share`.
    #[test]
    fn xdg_data_base_empty_xdg_falls_back_to_home() {
        let home = PathBuf::from("/Users/alice");
        let base = xdg_data_base(Some(""), Some(&home)).expect("base resolves");
        assert_eq!(base, PathBuf::from("/Users/alice/.local/share"));
    }

    /// With neither an XDG override nor a home directory, no data base can be
    /// resolved (the default `output_dir` then falls back to the empty string).
    #[test]
    fn xdg_data_base_none_when_no_home_no_xdg() {
        assert!(xdg_data_base(None, None).is_none());
    }
}
