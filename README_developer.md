# Developer Notes

## Overview

This program is two programs masquerading as one. The first sub-application is a
pty recorder: it presents a terminal to the user and captures everything the pty
session emits. The second is a file uploader with a fair amount of interactive
CLI. Most of the underlying complexity lives in the libraries; what follows is
the wiring that ties them together.

## Requirements

* Operating System: Linux, macOS, or Windows.
* A stable Rust toolchain (install via [rustup](https://rustup.rs/)).

No external build tooling (Make, etc.) is required — `cargo` drives everything.

## Building

ATerm is a single-binary crate. Use the standard `cargo` workflow from the
repository root:

```bash
cargo build            # debug build (target/debug/aterm)
cargo build --release  # optimized build (target/release/aterm)
cargo run              # build and run
cargo test             # run the test suite
```

The optional terminal-playback path (built on the `avt` virtual terminal) is
gated behind the `playback` feature and is off by default:

```bash
cargo build --features playback
cargo test  --features playback
```

Before opening a pull request, run the same gates CI enforces:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### Versioning

Build details (version, commit hash, build date) are surfaced to the user at
runtime. These are produced at build time and wired through CI on tagged
releases; see `.github/workflows/ci.yml` and `.github/workflows/WORKFLOWS.md`.

## Project Structure

The crate is organized as a thin binary (`src/main.rs`) over a library
(`src/lib.rs`) so the bulk of the logic is testable without the binary target.

### Phase 1: Terminal recording

The core idea is to start a pty console in a way that lets us store its output.
The pty is driven (via `portable-pty`, blocking — no async runtime) in
`src/recorder/`, with `unix.rs` and `windows.rs` providing the platform
specifics. Output is teed to both the user's terminal and the asciicast event
pipeline. The [asciicast v3] format is implemented directly in
`src/asciicast.rs` (asciinema is not a dependency): a cast is newline-delimited
JSON whose first line is a header object and whose following lines are
`[interval, code, data]` event arrays.

### Phase 2: Uploading

The upload flow is largely interactive CLI. The menus and prompt wrappers live
in `src/upload_menu.rs`, `src/menu.rs`, and `src/tui.rs` (thin wrappers over the
`inquire` prompt library). Talking to the ASHIRT backend — request signing,
operations/tags lookups, and the multipart upload itself — lives under
`src/ashirt/` (`http.rs`, `signing.rs`, `ops_tags.rs`, `upload.rs`).

[asciicast v3]: https://docs.asciinema.org/manual/asciicast/v3/
