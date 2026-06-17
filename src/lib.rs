//! aterm — ASHIRT terminal recorder (Rust rewrite, Option B).
//!
//! This crate is a SINGLE binary package (`aterm`). A thin `main.rs` parses the
//! CLI and hands off to [`app::run`]; everything else lives in this library so
//! downstream rewrite issues (aterm-8tn.2 .. aterm-8tn.N) can slot in without
//! touching the binary target.
//!
//! Conventions established here and inherited by every other module:
//!   * Runtime is BLOCKING everywhere — no tokio / async. HTTP will use
//!     `reqwest::blocking`; PTY work will use `portable-pty` (blocking).
//!   * Error handling: each module defines a typed error enum with `thiserror`.
//!     `anyhow::Result` is used ONLY at the command boundary ([`app::run`] and
//!     `main`). See [`config`] for a worked thiserror example and [`app`] for the
//!     anyhow boundary.

pub mod app;
pub mod asciicast;
pub mod ashirt;
pub mod cli;
pub mod config;
pub mod config_setup;
pub mod recorder;
pub mod tui;
pub mod update;

// Optional playback support built on the `avt` virtual terminal. Off by default;
// enable with `--features playback`. Kept out of the default build so the
// scaffold (and CI) stay lean.
#[cfg(feature = "playback")]
pub mod playback;
