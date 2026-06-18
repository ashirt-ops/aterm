//! PTY recording core.
//!
//! This module drives a child shell inside a pseudo-terminal (via
//! `portable-pty`, BLOCKING — no async runtime) and tees its output to BOTH the
//! user's terminal and the asciicast event pipeline ([`crate::asciicast`]).
//!
//! # Layout
//!
//! The PTY plumbing itself is cross-platform (`portable-pty` abstracts ConPTY on
//! Windows), so the shared [`Session`] trait, its [`PtySession`] implementation,
//! the [`Recorder`] event sink, the [`RawModeGuard`], the pure event-construction
//! helpers, and the [`record_session`] orchestration all live here. Only the
//! genuinely platform-specific pieces live behind `#[cfg]` in the [`unix`] and
//! [`windows`] submodules:
//!
//!   * resize detection — Unix uses `SIGWINCH`, Windows polls the console size;
//!   * raw stdin polling — so the forwarding loop can notice the child exiting
//!     and terminate, keeping the record and menu phases cleanly separate (no
//!     persistent stdin router à la the Go `copyRouter`).
//!
//! # Raw mode
//!
//! The controlling terminal is put into raw mode through [`RawModeGuard`], an
//! RAII guard whose [`Drop`] always restores the previous mode — on normal exit,
//! on error, and on panic (unwinding runs destructors).
//!
//! # Testability
//!
//! A full PTY recording cannot be asserted without a TTY, so the pure logic —
//! UTF-8 chunking of output bytes ([`Utf8Chunker`]), resize-data formatting,
//! exit-event construction, and feeding timed events to the writer — is factored
//! into functions/types that are unit-tested below without any terminal.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use thiserror::Error;

use crate::asciicast::{CastError, Event, EventKind, Header, Writer};

#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

/// Errors produced by a recording session.
#[derive(Debug, Error)]
pub enum RecorderError {
    /// The pseudo-terminal could not be allocated.
    #[error("failed to open pty: {0}")]
    OpenPty(String),
    /// The child shell could not be spawned in the PTY.
    #[error("failed to spawn shell: {0}")]
    Spawn(String),
    /// Propagating a resize to the PTY failed.
    #[error("pty resize failed: {0}")]
    Resize(String),
    /// Waiting on / reaping the child failed.
    #[error("failed to wait for child: {0}")]
    Wait(String),
    /// The background output-pump thread panicked.
    #[error("output thread panicked")]
    OutputThreadPanicked,
    /// Underlying I/O failure (raw mode, stdin, PTY streams).
    #[error("pty i/o error")]
    Io(#[from] io::Error),
    /// Writing the asciicast stream failed.
    #[error("asciicast write failed")]
    Cast(#[from] CastError),
}

/// Outcome of a single non-blocking stdin poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdinRead {
    /// The poll timed out with no input available.
    Timeout,
    /// Standard input reached end-of-file (e.g. redirected/closed).
    Eof,
    /// `usize` bytes were read into the caller's buffer.
    Data(usize),
}

/// A recordable PTY session.
///
/// Implementations drive a child shell inside a pseudo-terminal and surface its
/// I/O streams so the recorder can tee output and forward input. Construction
/// (which spawns the child) is implementation-specific; see
/// [`PtySession::spawn`].
pub trait Session {
    /// Clones a reader over the PTY master (child output → recorder + terminal).
    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, RecorderError>;

    /// Takes the writer over the PTY master (user input → child).
    fn take_writer(&mut self) -> Result<Box<dyn Write + Send>, RecorderError>;

    /// Propagates a terminal resize to the child PTY.
    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), RecorderError>;

    /// Returns `Some(exit_code)` if the child has exited, else `None`.
    fn try_wait(&mut self) -> Result<Option<u32>, RecorderError>;
}

/// A `portable-pty`-backed [`Session`]. Cross-platform: `portable-pty` selects
/// the native backend (Unix PTY or Windows ConPTY).
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl PtySession {
    /// Spawns `shell` in a fresh PTY sized `cols` x `rows`, inheriting the
    /// current environment and working directory.
    pub fn spawn(shell: &str, cols: u16, rows: u16) -> Result<Self, RecorderError> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| RecorderError::OpenPty(e.to_string()))?;

        let mut cmd = CommandBuilder::new(shell);
        for (key, value) in std::env::vars() {
            cmd.env(key, value);
        }
        if let Ok(cwd) = std::env::current_dir() {
            cmd.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| RecorderError::Spawn(e.to_string()))?;

        // The slave handle is owned by the child now; dropping our copy means an
        // EOF on the master once the child (and any descendants) exit.
        drop(pair.slave);

        Ok(Self {
            master: pair.master,
            child,
        })
    }
}

impl Session for PtySession {
    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, RecorderError> {
        self.master
            .try_clone_reader()
            .map_err(|e| RecorderError::Io(io::Error::other(e.to_string())))
    }

    fn take_writer(&mut self) -> Result<Box<dyn Write + Send>, RecorderError> {
        self.master
            .take_writer()
            .map_err(|e| RecorderError::Io(io::Error::other(e.to_string())))
    }

    fn resize(&mut self, cols: u16, rows: u16) -> Result<(), RecorderError> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| RecorderError::Resize(e.to_string()))
    }

    fn try_wait(&mut self) -> Result<Option<u32>, RecorderError> {
        match self
            .child
            .try_wait()
            .map_err(|e| RecorderError::Wait(e.to_string()))?
        {
            // `exit_code()` is a `u32`; keep it unsigned so large Windows exit
            // codes are not truncated to a negative `"x"` payload.
            Some(status) => Ok(Some(status.exit_code())),
            None => Ok(None),
        }
    }
}

/// RAII guard that puts the controlling terminal into raw mode and **always**
/// restores the previous mode on drop — including on error and panic.
///
/// Construct with [`RawModeGuard::enable`] and keep the value alive for the
/// duration of the recording; let it drop to restore.
#[must_use = "raw mode is restored when the guard is dropped; bind it to a variable"]
pub struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    /// Enables raw mode on the controlling terminal.
    pub fn enable() -> Result<Self, RecorderError> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self { active: true })
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            // Best-effort: nothing actionable if restoration itself fails, and a
            // Drop impl must not panic.
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}

/// Incrementally decodes a byte stream into UTF-8 strings suitable for asciicast
/// `"o"` event data.
///
/// PTY output is raw bytes and a multi-byte UTF-8 sequence can straddle two
/// reads. [`push`](Utf8Chunker::push) emits the longest valid prefix and carries
/// an incomplete trailing sequence (at most 3 bytes) into the next call, so
/// characters are never split across events. Genuinely invalid bytes are
/// replaced with U+FFFD rather than carried, bounding the carry buffer.
#[derive(Debug, Default)]
pub struct Utf8Chunker {
    carry: Vec<u8>,
}

impl Utf8Chunker {
    /// Feeds `bytes`, returning the decodable text (possibly empty if the input
    /// was only the start of a multi-byte sequence).
    pub fn push(&mut self, bytes: &[u8]) -> String {
        let mut combined = std::mem::take(&mut self.carry);
        combined.extend_from_slice(bytes);

        let mut out = String::new();
        let mut rest: &[u8] = &combined;
        loop {
            match std::str::from_utf8(rest) {
                Ok(s) => {
                    out.push_str(s);
                    break;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    // `valid_up_to()` guarantees `rest[..valid]` is valid UTF-8,
                    // so this re-decode never fails.
                    out.push_str(
                        std::str::from_utf8(&rest[..valid])
                            .expect("valid_up_to guarantees validity"),
                    );
                    match e.error_len() {
                        // Incomplete trailing sequence: carry it for next time.
                        None => {
                            self.carry.extend_from_slice(&rest[valid..]);
                            break;
                        }
                        // Invalid byte(s): emit a replacement char and continue.
                        Some(len) => {
                            out.push('\u{FFFD}');
                            rest = &rest[valid + len..];
                        }
                    }
                }
            }
        }
        out
    }

    /// Flushes any carried (incomplete) bytes as lossy UTF-8. Call once at the
    /// end of the stream.
    pub fn flush(&mut self) -> String {
        if self.carry.is_empty() {
            return String::new();
        }
        let s = String::from_utf8_lossy(&self.carry).into_owned();
        self.carry.clear();
        s
    }
}

/// Formats an exit status code as asciicast `"x"` event data.
pub fn exit_status_data(code: u32) -> String {
    code.to_string()
}

/// Streams timed asciicast events to a [`Writer`], stamping each event with the
/// elapsed time since the recorder started.
///
/// The event-construction methods also accept an explicit elapsed time (`at`)
/// so the pure timing/formatting logic can be unit-tested without a real clock;
/// live callers pass [`elapsed`](Recorder::elapsed).
pub struct Recorder<W: Write> {
    writer: Writer<W>,
    start: Instant,
}

impl<W: Write> Recorder<W> {
    /// Creates a recorder over `sink`, writing `header` immediately and starting
    /// the clock.
    pub fn start(sink: W, header: &Header) -> Result<Self, RecorderError> {
        let mut writer = Writer::new(sink, 0.0);
        writer.write_header(header)?;
        Ok(Self {
            writer,
            start: Instant::now(),
        })
    }

    /// Seconds elapsed since the recorder started (monotonic).
    pub fn elapsed(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// Records a `"o"` output event at elapsed time `at`.
    pub fn output(&mut self, at: f64, data: &str) -> Result<(), RecorderError> {
        self.writer.write_event(at, EventKind::Output, data)?;
        Ok(())
    }

    /// Records a `"r"` resize event (data `"<cols>x<rows>"`) at elapsed time `at`.
    pub fn resize(&mut self, at: f64, cols: u16, rows: u16) -> Result<(), RecorderError> {
        self.writer
            .write_event(at, EventKind::Resize, Event::resize_data(cols, rows))?;
        Ok(())
    }

    /// Records the final `"x"` exit event at elapsed time `at`.
    pub fn exit(&mut self, at: f64, code: u32) -> Result<(), RecorderError> {
        self.writer
            .write_event(at, EventKind::Exit, exit_status_data(code))?;
        Ok(())
    }

    /// Flushes and returns the underlying sink.
    pub fn finish(self) -> Result<W, RecorderError> {
        Ok(self.writer.into_inner()?)
    }
}

/// Returns the controlling terminal's size, defaulting to 80x24 when it cannot
/// be queried (e.g. no TTY).
fn terminal_size() -> (u16, u16) {
    crossterm::terminal::size().unwrap_or((80, 24))
}

/// Builds an asciicast header capturing the initial geometry plus `TERM`/`SHELL`
/// environment and a capture timestamp.
fn build_header(cols: u16, rows: u16, shell: &str) -> Header {
    let mut header = Header::new(cols, rows);

    let term = std::env::var("TERM").ok();
    header.term.term_type = term.clone();

    let mut env = BTreeMap::new();
    if let Some(term) = term {
        env.insert("TERM".to_string(), term);
    }
    env.insert("SHELL".to_string(), shell.to_string());
    header.env = Some(env);

    header.timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64);

    header
}

/// Locks the shared recorder, mapping a poisoned mutex to a [`RecorderError`]
/// instead of panicking — these call sites are all on `Result`-returning paths.
fn lock_recorder<W: Write>(
    recorder: &Arc<Mutex<Recorder<W>>>,
) -> Result<MutexGuard<'_, Recorder<W>>, RecorderError> {
    recorder
        .lock()
        .map_err(|_| RecorderError::Io(io::Error::other("recorder mutex poisoned")))
}

/// Pumps child PTY output to both the user's terminal (raw bytes) and the
/// recorder (timestamped `"o"` events) until EOF.
fn pump_output<W: Write>(
    mut reader: Box<dyn Read + Send>,
    recorder: &Arc<Mutex<Recorder<W>>>,
) -> Result<(), RecorderError> {
    let mut chunker = Utf8Chunker::default();
    let mut buf = [0u8; 8192];
    let stdout = io::stdout();

    loop {
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
            // A closed PTY master surfaces as an error (EIO on Linux) rather than
            // a clean 0-byte read; treat any read error as end-of-stream.
            Err(_) => break,
        };

        {
            let mut lock = stdout.lock();
            lock.write_all(&buf[..n])?;
            lock.flush()?;
        }

        let text = chunker.push(&buf[..n]);
        if !text.is_empty() {
            let mut rec = lock_recorder(recorder)?;
            let at = rec.elapsed();
            rec.output(at, &text)?;
        }
    }

    let tail = chunker.flush();
    if !tail.is_empty() {
        let mut rec = lock_recorder(recorder)?;
        let at = rec.elapsed();
        rec.output(at, &tail)?;
    }

    Ok(())
}

/// Records a live shell session to `sink` as an asciicast v3 stream and returns
/// the child's exit code.
///
/// This is the orchestration entry point: it spawns `shell` in a PTY, puts the
/// controlling terminal into raw mode (restored via [`RawModeGuard`]), tees the
/// child's output to the terminal and to the recorder, forwards user input to
/// the child, propagates resizes, and emits a final `"x"` exit event.
///
/// The stdin-forwarding loop polls with a timeout so it terminates as soon as
/// the child exits — record and menu stay separate phases with no persistent
/// stdin router.
pub fn record_session<W: Write + Send + 'static>(
    shell: &str,
    sink: W,
) -> Result<u32, RecorderError> {
    let (cols, rows) = terminal_size();
    let header = build_header(cols, rows, shell);
    let recorder = Arc::new(Mutex::new(Recorder::start(sink, &header)?));

    let mut session = PtySession::spawn(shell, cols, rows)?;
    let reader = session.try_clone_reader()?;
    let mut writer = session.take_writer()?;

    // Raw mode is entered only after the PTY is up so an early failure leaves the
    // terminal untouched. The guard restores cooked mode on any exit path.
    let _raw = RawModeGuard::enable()?;
    let mut resize_watcher = platform::ResizeWatcher::install()?;
    let mut stdin_poller = platform::StdinPoller::new()?;

    let pump_recorder = Arc::clone(&recorder);
    let pump = thread::spawn(move || pump_output(reader, &pump_recorder));

    // The I/O loop runs in a closure so an error inside it never `?`-returns past
    // the teardown below. The pump thread MUST be joined on every exit path —
    // otherwise it would keep writing PTY output to stdout and the sink after
    // `record_session` has returned (spurious writes, terminal already restored).
    let loop_outcome = (|| -> Result<u32, RecorderError> {
        let mut stdin_buf = [0u8; 8192];
        let mut stdin_open = true;
        loop {
            if resize_watcher.take_pending() {
                let (cols, rows) = terminal_size();
                session.resize(cols, rows)?;
                let mut rec = lock_recorder(&recorder)?;
                let at = rec.elapsed();
                rec.resize(at, cols, rows)?;
            }

            if stdin_open {
                match stdin_poller.poll(&mut stdin_buf, Duration::from_millis(50))? {
                    StdinRead::Data(n) => {
                        writer.write_all(&stdin_buf[..n])?;
                        writer.flush()?;
                    }
                    StdinRead::Eof => stdin_open = false,
                    StdinRead::Timeout => {}
                }
            } else {
                thread::sleep(Duration::from_millis(50));
            }

            if let Some(code) = session.try_wait()? {
                return Ok(code);
            }
        }
    })();

    // Teardown, run on EVERY exit path (success or error): stop forwarding input,
    // drain + join the pump thread, then flush the cast tail.
    drop(writer);
    let pump_result = match pump.join() {
        Ok(result) => result,
        Err(_) => Err(RecorderError::OutputThreadPanicked),
    };

    // Recover the recorder now that the pump is gone, tolerating a poisoned mutex
    // (a panicked pump) so we can still flush the tail.
    let mut recorder = Arc::into_inner(recorder)
        .expect("all other Arc holders joined")
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    // Append the final `"x"` event only on a clean run; on an error path we still
    // flush whatever was captured but emit no bogus exit code.
    let exit_record = match &loop_outcome {
        Ok(code) => {
            let at = recorder.elapsed();
            recorder.exit(at, *code)
        }
        Err(_) => Ok(()),
    };
    let finish_result = recorder.finish();

    // Surface errors in priority order; teardown above already ran regardless.
    let code = loop_outcome?;
    pump_result?;
    exit_record?;
    finish_result?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn chunker_passes_ascii_through() {
        let mut chunker = Utf8Chunker::default();
        assert_eq!(chunker.push(b"hello world"), "hello world");
        assert_eq!(chunker.flush(), "");
    }

    #[test]
    fn chunker_carries_split_multibyte_sequence() {
        let mut chunker = Utf8Chunker::default();
        // "é" is 0xC3 0xA9 — split across two reads.
        assert_eq!(chunker.push(&[0xC3]), "");
        assert_eq!(chunker.push(&[0xA9]), "é");
        assert_eq!(chunker.flush(), "");
    }

    #[test]
    fn chunker_handles_multibyte_split_with_trailing_ascii() {
        let mut chunker = Utf8Chunker::default();
        // Emoji "😀" (F0 9F 98 80) split, with ascii following the completion.
        assert_eq!(chunker.push(&[0xF0, 0x9F]), "");
        assert_eq!(chunker.push(&[0x98, 0x80, b'!']), "😀!");
    }

    #[test]
    fn chunker_replaces_invalid_bytes_and_keeps_going() {
        let mut chunker = Utf8Chunker::default();
        // 0xFF is never valid UTF-8; surrounding ascii must survive.
        let out = chunker.push(&[b'a', 0xFF, b'b']);
        assert_eq!(out, "a\u{FFFD}b");
        // Nothing should be carried for a definitively-invalid byte.
        assert_eq!(chunker.flush(), "");
    }

    #[test]
    fn chunker_flushes_incomplete_tail_lossily() {
        let mut chunker = Utf8Chunker::default();
        // A lone lead byte with no continuation: held, then flushed as U+FFFD.
        assert_eq!(chunker.push(&[0xE2, 0x82]), "");
        assert_eq!(chunker.flush(), "\u{FFFD}");
    }

    #[test]
    fn exit_status_data_is_decimal_code() {
        assert_eq!(exit_status_data(0), "0");
        assert_eq!(exit_status_data(1), "1");
        assert_eq!(exit_status_data(130), "130");
    }

    #[test]
    fn recorder_emits_header_then_relative_interval_events() {
        let mut rec = Recorder::start(Vec::new(), &Header::new(80, 24)).unwrap();
        // Explicit elapsed times so timing is deterministic without a TTY.
        rec.output(0.0, "hi").unwrap();
        rec.resize(0.5, 120, 40).unwrap();
        rec.exit(1.25, 0).unwrap();
        let bytes = rec.finish().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let mut lines = text.lines();

        let header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(header["version"], 3);
        assert_eq!(header["term"]["cols"], 80);

        let out: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(out[0], 0.0);
        assert_eq!(out[1], "o");
        assert_eq!(out[2], "hi");

        let resize: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(resize[0], 0.5); // 0.5 - 0.0
        assert_eq!(resize[1], "r");
        assert_eq!(resize[2], "120x40");

        let exit: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(exit[0], 0.75); // 1.25 - 0.5
        assert_eq!(exit[1], "x");
        assert_eq!(exit[2], "0");

        assert!(lines.next().is_none());
    }

    #[test]
    fn build_header_captures_geometry_and_shell_env() {
        let header = build_header(132, 50, "/bin/zsh");
        assert_eq!(header.term.cols, 132);
        assert_eq!(header.term.rows, 50);
        assert_eq!(header.env.as_ref().unwrap()["SHELL"], "/bin/zsh");
        assert!(header.timestamp.is_some());
    }

    // PTY smoke test: spawns a trivial command in a real pseudo-terminal and
    // asserts the produced cast has an `"o"` event and a final `"x"` event.
    // Skips gracefully when a PTY cannot be allocated so headless CI never fails.
    #[cfg(unix)]
    #[test]
    fn pty_smoke_produces_output_and_exit_events() {
        let pty_system = native_pty_system();
        let pair = match pty_system.openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("skipping pty smoke test: cannot allocate pty: {e}");
                return;
            }
        };

        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.arg("-c");
        cmd.arg("printf 'hi\\n'");
        let mut child = match pair.slave.spawn_command(cmd) {
            Ok(child) => child,
            Err(e) => {
                eprintln!("skipping pty smoke test: cannot spawn shell: {e}");
                return;
            }
        };
        let mut reader = pair.master.try_clone_reader().unwrap();
        drop(pair.slave);

        let mut rec = Recorder::start(Vec::new(), &Header::new(80, 24)).unwrap();
        let mut chunker = Utf8Chunker::default();
        let mut buf = [0u8; 1024];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let text = chunker.push(&buf[..n]);
                    if !text.is_empty() {
                        let at = rec.elapsed();
                        rec.output(at, &text).unwrap();
                    }
                }
                Err(_) => break,
            }
        }
        let tail = chunker.flush();
        if !tail.is_empty() {
            let at = rec.elapsed();
            rec.output(at, &tail).unwrap();
        }

        let code = child.wait().unwrap().exit_code();
        let at = rec.elapsed();
        rec.exit(at, code).unwrap();

        let text = String::from_utf8(rec.finish().unwrap()).unwrap();
        let mut lines = text.lines();

        // Line 1 is the header.
        let header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(header["version"], 3);

        let events: Vec<Value> = lines
            .filter(|l| !l.is_empty())
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert!(
            events.iter().any(|e| e[1] == "o"),
            "expected at least one output event, cast was: {text}"
        );
        let last = events.last().expect("expected a final event");
        assert_eq!(last[1], "x", "last event must be the exit event");
        assert_eq!(last[2], "0");
    }
}
