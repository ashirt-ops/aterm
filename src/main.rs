//! aterm binary entrypoint. Thin on purpose: parse the CLI, hand off to
//! [`aterm::app::run`], and render any error at the `anyhow` boundary as a clean
//! message before exiting non-zero.

use clap::Parser;

fn main() {
    let cli = aterm::cli::Cli::parse();
    if let Err(err) = aterm::app::run(cli) {
        // `{:#}` renders the full anyhow context chain on one line, without the
        // Debug backtrace noise the default `fn main() -> Result` would print.
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
