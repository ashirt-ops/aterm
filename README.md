# ASHIRT Terminal Recorder (aterm)

aterm records a terminal session in a separate pty and lets you upload the
recording to an ASHIRT server. It is a single-binary Rust crate; recordings are
written in the [asciicast v3] format.

## Building & Running

aterm builds with the standard stable Rust toolchain (install via
[rustup](https://rustup.rs/)) on Linux, macOS, or Windows. No external build
tooling is required — `cargo` drives everything.

```bash
cargo build            # debug build   (target/debug/aterm)
cargo build --release  # release build (target/release/aterm)
cargo run              # build and run
cargo test             # run the test suite
```

Before opening a pull request, run the gates CI enforces:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## User's Guide

Run the `aterm` binary to start. The application describes what it is doing as
it goes; this section gives a quick orientation rather than exhaustive detail.

### First run

On first run, a short dialog collects the required details (API URL, Access Key,
Secret Key, etc.) and saves them. If you also use the ASHIRT desktop
application, some values can be pulled from its configuration. You can re-run
this dialog later with `--reset` or `--reset-hard`.

### Navigating menus

Move through menus with the arrow keys or `j`/`k`. Press `/` to filter options
by a case-insensitive substring; press `/` again to leave search.

### Recording

Starting `aterm` normally begins a new recording. You are prompted to select an
operation to associate with the recording, after which the pseudo terminal
starts and behaves like a normal shell. To end a recording, type `exit` or press
`Ctrl-D` at an empty prompt.

### After a recording

When a recording ends, a menu offers:

1. **Upload Recording** — supply a description and select tags, then submit to
   the server. A successful upload saves the metadata and returns to the main
   menu.
2. **Rename Recording File** — give the recording a more memorable name (normal
   filename rules apply).
3. **Discard Recording** — delete the recording.
4. **Return to Main Menu** — saves the recording metadata and returns to the
   menu.

## Configuration

Configuration is resolved in layers, each overriding the previous: built-in
defaults, then the config file, then command-line flags.

The config file follows the XDG standard and lives at
`<config>/aterm/config.yaml` (on Linux, `~/.config/aterm/config.yaml`, honoring
`$XDG_CONFIG_HOME`). Most settings can also be edited from the main menu via
"Update Settings".

| Config file key  | CLI flag                  | Meaning                                                            |
| ---------------- | ------------------------- | ----------------------------------------------------------------- |
| `outputDir`      |                           | Where to store recording files (defaults to the XDG data dir)     |
| `recordingShell` | `-s`, `--shell`           | Shell to launch (defaults to `$SHELL` on Unix, PowerShell on Windows) |
| `operationSlug`  | `--operation`             | Operation to upload to (can also be selected before recording)    |
| `apiURL`         |                           | Where the ASHIRT backend service is located                       |
| `accessKey`      |                           | Access Key for the backend (created in the frontend)              |
| `secretKey`      |                           | Secret Key for the backend (base-64; created in the frontend)     |
|                  | `-n`, `--name`            | Filename to use for the recording (locally and remotely)          |
|                  | `-m`, `--menu`            | Start in the main menu instead of recording immediately           |
|                  | `--print-config`, `--pc`  | Print the resolved configuration, then exit                       |
|                  | `--reset`                 | Re-run first-run setup, using existing values as a base           |
|                  | `--reset-hard`            | Re-run first-run setup without using existing values              |
|                  | `-h`, `--help`            | Show help                                                         |
|                  | `--version`               | Show version information                                          |

## Known issues

Exiting a recording with `^d` can leave the terminal in application
(cursor-key) mode, so arrow keys may stop working when the terminal returns.
aterm forwards PTY output verbatim and does not reset keypad mode on teardown,
so this depends on how the recorded shell cleans up. The `j`/`k` keys still
navigate; entering a new shell and exiting it with `exit` restores interactive
mode.

## Architecture

aterm is two programs in one: a pty recorder and an interactive uploader. The
crate is a thin binary (`src/main.rs`) over a library (`src/lib.rs`) so the
logic is testable without the binary target.

**Recording.** The pty is driven via `portable-pty` (blocking — no async
runtime) in `src/recorder/`, with `unix.rs` and `windows.rs` providing the
platform specifics. Output is teed to both the user's terminal and the asciicast
event pipeline. The [asciicast v3] format is implemented directly in
`src/asciicast.rs` (asciinema is not a dependency): a cast is newline-delimited
JSON whose first line is a header object and whose remaining lines are
`[interval, code, data]` event arrays.

**Uploading.** The interactive menus and prompt wrappers live in
`src/upload_menu.rs`, `src/menu.rs`, and `src/tui.rs` (thin wrappers over the
`inquire` prompt library). Talking to the ASHIRT backend — request signing,
operations/tags lookups, and the multipart upload — lives under `src/ashirt/`
(`http.rs`, `signing.rs`, `operations.rs`, `tags.rs`, `upload.rs`).

Build metadata (version, commit hash, build date) is produced at build time and
wired through CI on tagged releases; see `.github/workflows/`.

[asciicast v3]: https://docs.asciinema.org/manual/asciicast/v3/
