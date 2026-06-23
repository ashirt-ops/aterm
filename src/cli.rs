//! Command-line interface (clap v4 derive).
//!
//! Flags mirror the Go `aterm` binary so existing muscle-memory and docs keep
//! working. `--version` is provided automatically by clap via `#[command(version)]`.

use clap::Parser;

/// ASHIRT terminal recorder.
#[derive(Debug, Clone, Parser)]
#[command(name = "aterm", version, about = "ASHIRT terminal recorder")]
pub struct Cli {
    /// Shell to launch for the recording session (e.g. `/bin/zsh`).
    #[arg(short = 's', long = "shell")]
    pub shell: Option<String>,

    /// Operation slug to associate the recording with.
    #[arg(long = "operation")]
    pub operation: Option<String>,

    /// Force the interactive menu instead of recording immediately.
    #[arg(short = 'm', long = "menu")]
    pub menu: bool,

    /// Print the resolved configuration and exit.
    //
    // The Go flag was `-pc`; clap shorts are single-character, so we expose the
    // canonical `--print-config` plus a `--pc` long alias.
    #[arg(long = "print-config", visible_alias = "pc")]
    pub print_config: bool,

    /// Include secret values (the API secret key) in `--print-config` output.
    /// Off by default, so the secret is masked unless explicitly requested.
    #[arg(long = "show-secrets")]
    pub show_secrets: bool,

    /// Reset saved configuration (soft reset).
    #[arg(long = "reset")]
    pub reset: bool,

    /// Reset saved configuration AND recordings (hard reset).
    #[arg(long = "reset-hard")]
    pub reset_hard: bool,

    /// Name to give the recording.
    #[arg(short = 'n', long = "name")]
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        // Catches duplicate flags / bad arg config at test time.
        Cli::command().debug_assert();
    }
}
