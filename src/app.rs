//! Application orchestration — the entrypoint wiring called by `main`.
//!
//! This is the `anyhow` BOUNDARY: typed module errors (`thiserror`) bubble up
//! here and get human-facing context via [`anyhow::Context`]. Nothing below this
//! layer should use `anyhow`.

use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::config::Config;

/// Runs aterm end to end.
pub fn run(cli: Cli) -> Result<()> {
    if cli.reset || cli.reset_hard {
        // TODO(aterm-8tn.4): clear saved config (and recordings if --reset-hard).
        return Ok(());
    }

    // Example of the typed-error -> anyhow boundary: `Config::load` returns a
    // `ConfigError` (thiserror); `.context(..)` lifts it into `anyhow::Error`.
    let config = Config::load(&cli).context("loading aterm configuration")?;

    if cli.print_config {
        println!("{config}");
        return Ok(());
    }

    // TODO(aterm-8tn.7+): build a recorder, run the session, stream asciicast,
    // then upload the recording as ASHIRT evidence.
    todo!("app::run: wire recorder -> asciicast -> ashirt::upload")
}
