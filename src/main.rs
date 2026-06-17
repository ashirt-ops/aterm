//! aterm binary entrypoint. Thin on purpose: parse the CLI, hand off to
//! [`aterm::app::run`], and let `anyhow` render any error at the boundary.

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = aterm::cli::Cli::parse();
    aterm::app::run(cli)
}
