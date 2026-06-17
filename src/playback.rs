//! Optional playback / preview support (`--features playback`).
//!
//! Drives a recorded [asciicast v3] stream through the [`avt`] virtual terminal
//! so a recording can be previewed or verified without replaying it into a real
//! TTY. Two capabilities are provided:
//!
//!   * [`snapshot`] — render the **final** screen state of a cast to plain text
//!     (a thumbnail / preview of where the session ended up), honoring resize
//!     events along the way.
//!   * [`replay`] — a simple local replay that writes each output frame to a
//!     sink, sleeping for the relative interval between frames. The pure frame
//!     schedule is exposed separately as [`frames`] so the timing logic is unit
//!     testable headlessly (no sleeping, no TTY).
//!
//! This module is **additive** and gated behind the optional `avt` dependency:
//! it is compiled only with `--features playback` and is never wired into the
//! default binary path.
//!
//! [asciicast v3]: https://docs.asciinema.org/manual/asciicast/v3/

use std::io::Write;
use std::time::Duration;

use serde_json::Value;
use thiserror::Error;

use crate::asciicast::{Event, EventKind, Header};

/// Errors produced while parsing or replaying an asciicast for playback.
#[derive(Debug, Error)]
pub enum PlaybackError {
    /// The cast was empty — it had no header line.
    #[error("playback: empty cast (missing header line)")]
    Empty,
    /// A line of the cast could not be parsed as a header or event.
    #[error("playback: failed to parse cast: {0}")]
    Parse(String),
    /// Writing a replay frame to the sink failed.
    #[error("playback write failed")]
    Io(#[from] std::io::Error),
}

/// A parsed asciicast v3 recording: its [`Header`] plus the ordered [`Event`]s.
///
/// Constructed via [`Cast::parse`]; the contained types are reused verbatim from
/// [`crate::asciicast`] so playback shares the recorder's data model.
#[derive(Debug, Clone)]
pub struct Cast {
    /// Stream header (terminal geometry and metadata).
    pub header: Header,
    /// Events in capture order, each carrying its relative interval.
    pub events: Vec<Event>,
}

impl Cast {
    /// Parses an asciicast v3 stream from its textual contents.
    ///
    /// The first line must be the JSON header object; subsequent lines are
    /// either `[interval, code, data]` event arrays, blank lines, or `#`
    /// comments (both ignored). Returns [`PlaybackError::Parse`] on the first
    /// malformed line.
    pub fn parse(input: &str) -> Result<Cast, PlaybackError> {
        let mut lines = input.lines();
        let header_line = lines.next().ok_or(PlaybackError::Empty)?;
        let header: Header = serde_json::from_str(header_line)
            .map_err(|e| PlaybackError::Parse(format!("header: {e}")))?;

        let mut events = Vec::new();
        for line in lines {
            let line = line.trim_end();
            // Blank lines and `#` comments carry no events.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            events.push(parse_event(line)?);
        }

        Ok(Cast { header, events })
    }
}

/// Parses a single `[interval, "code", "data"]` event array line.
fn parse_event(line: &str) -> Result<Event, PlaybackError> {
    let value: Value =
        serde_json::from_str(line).map_err(|e| PlaybackError::Parse(format!("event: {e}")))?;
    let arr = value
        .as_array()
        .ok_or_else(|| PlaybackError::Parse(format!("event is not an array: {line}")))?;
    if arr.len() != 3 {
        return Err(PlaybackError::Parse(format!(
            "event must have 3 elements, got {}: {line}",
            arr.len()
        )));
    }

    let interval = arr[0]
        .as_f64()
        .ok_or_else(|| PlaybackError::Parse(format!("event interval is not a number: {line}")))?;
    let code = arr[1]
        .as_str()
        .ok_or_else(|| PlaybackError::Parse(format!("event code is not a string: {line}")))?;
    let data = arr[2]
        .as_str()
        .ok_or_else(|| PlaybackError::Parse(format!("event data is not a string: {line}")))?;

    let kind = match code {
        "o" => EventKind::Output,
        "i" => EventKind::Input,
        "r" => EventKind::Resize,
        "m" => EventKind::Marker,
        "x" => EventKind::Exit,
        other => {
            return Err(PlaybackError::Parse(format!(
                "unknown event code {other:?}: {line}"
            )))
        }
    };

    Ok(Event::new(interval, kind, data))
}

/// Renders the **final** screen state of `cast` to text (a preview/thumbnail).
///
/// The virtual terminal is sized from the header's geometry, every output event
/// is fed through it, and resize events adjust the grid mid-stream. Input,
/// marker, and exit events do not affect the rendered screen and are ignored.
/// The returned string is the visible rows joined by `\n`, with trailing
/// whitespace and trailing blank lines stripped.
pub fn snapshot(cast: &Cast) -> String {
    let mut vt = avt::Vt::builder()
        .size(
            usize::from(cast.header.term.cols).max(1),
            usize::from(cast.header.term.rows).max(1),
        )
        .build();

    for event in &cast.events {
        match event.kind {
            EventKind::Output => {
                vt.feed_str(&event.data);
            }
            EventKind::Resize => {
                if let Some((cols, rows)) = parse_resize(&event.data) {
                    vt.resize(cols.max(1), rows.max(1));
                }
            }
            EventKind::Input | EventKind::Marker | EventKind::Exit => {}
        }
    }

    screen_text(&vt)
}

/// Renders raw terminal output `bytes` through a `cols` x `rows` virtual
/// terminal and returns the final screen contents.
///
/// A convenience wrapper around [`avt::Vt`] for the common case of previewing a
/// single blob of captured output (no event timing). Use [`snapshot`] to render
/// a full parsed [`Cast`] including resize handling.
pub fn render(bytes: &str, cols: usize, rows: usize) -> String {
    let mut vt = avt::Vt::builder().size(cols.max(1), rows.max(1)).build();
    vt.feed_str(bytes);
    screen_text(&vt)
}

/// A single replay frame: how long to wait before showing it and the output
/// bytes to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// Delay since the previous frame (or the start of replay for the first).
    pub delay: Duration,
    /// Output bytes to write for this frame.
    pub data: String,
}

/// Builds the ordered replay schedule from a cast.
///
/// One [`Frame`] is produced per **output** event. The interval of any
/// non-output events (input, resize, marker, exit) is folded into the delay of
/// the next output frame so the wall-clock pacing of the original recording is
/// preserved even though only output is rendered. This is pure (no sleeping, no
/// I/O), which keeps it unit-testable headlessly.
pub fn frames(cast: &Cast) -> Vec<Frame> {
    let mut frames = Vec::new();
    // Accumulated time since the last emitted frame, including the intervals of
    // any skipped non-output events.
    let mut pending = 0.0_f64;

    for event in &cast.events {
        pending += event.interval.max(0.0);
        if event.kind == EventKind::Output {
            frames.push(Frame {
                delay: Duration::from_secs_f64(pending),
                data: event.data.clone(),
            });
            pending = 0.0;
        }
    }

    // If the cast ends with non-output events (e.g. a final resize or exit),
    // any `pending` interval they contributed is intentionally dropped: it would
    // be a trailing wait with nothing left to render.
    frames
}

/// Replays `cast` to `out`, writing each output frame after sleeping for its
/// relative interval.
///
/// `speed` is a playback multiplier: `1.0` is real time, `2.0` is twice as fast,
/// values `<= 0.0` are treated as `1.0`. Each frame's data is written and the
/// sink flushed so the replay appears incrementally. Pass a terminal's stdout as
/// `out` to watch the recording, or any [`Write`] sink to capture it.
pub fn replay<W: Write>(cast: &Cast, out: &mut W, speed: f64) -> Result<(), PlaybackError> {
    let speed = if speed > 0.0 { speed } else { 1.0 };

    for frame in frames(cast) {
        let secs = frame.delay.as_secs_f64() / speed;
        if secs > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(secs));
        }
        out.write_all(frame.data.as_bytes())?;
        out.flush()?;
    }

    Ok(())
}

/// Parses a v3 resize payload `"<cols>x<rows>"` into `(cols, rows)`.
///
/// The format is strict: two base-10 integers separated by a single `x` with no
/// surrounding whitespace, matching the asciicast v3 spec. Anything else returns
/// `None` and the resize is ignored.
fn parse_resize(data: &str) -> Option<(usize, usize)> {
    let (cols, rows) = data.split_once('x')?;
    Some((cols.parse().ok()?, rows.parse().ok()?))
}

/// Collects the virtual terminal's **visible** viewport (the final screen, not
/// scrollback) into a single string, trimming trailing whitespace per row and
/// dropping trailing blank rows so a mostly-empty screen renders compactly.
fn screen_text(vt: &avt::Vt) -> String {
    let mut rows: Vec<String> = vt
        .view()
        .map(|line| line.text().trim_end().to_owned())
        .collect();
    while rows.last().is_some_and(|r| r.is_empty()) {
        rows.pop();
    }
    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small synthetic cast: prints "hello", a CRLF, then "world", with a
    /// couple of intervals and a comment line.
    const SAMPLE: &str = concat!(
        "{\"version\":3,\"term\":{\"cols\":20,\"rows\":5}}\n",
        "# recorded by aterm\n",
        "[0.5,\"o\",\"hello\"]\n",
        "[0.5,\"o\",\"\\r\\n\"]\n",
        "[1.0,\"o\",\"world\"]\n",
    );

    #[test]
    fn parse_reads_header_and_events_skipping_comments() {
        let cast = Cast::parse(SAMPLE).unwrap();
        assert_eq!(cast.header.term.cols, 20);
        assert_eq!(cast.header.term.rows, 5);
        // Comment line is skipped; three output events remain.
        assert_eq!(cast.events.len(), 3);
        assert_eq!(cast.events[0].kind, EventKind::Output);
        assert_eq!(cast.events[0].data, "hello");
        assert_eq!(cast.events[1].data, "\r\n");
    }

    #[test]
    fn parse_rejects_empty_input() {
        assert!(matches!(Cast::parse(""), Err(PlaybackError::Empty)));
    }

    #[test]
    fn parse_rejects_bad_header() {
        assert!(matches!(
            Cast::parse("not json\n"),
            Err(PlaybackError::Parse(_))
        ));
    }

    #[test]
    fn parse_rejects_malformed_event() {
        let input = concat!(
            "{\"version\":3,\"term\":{\"cols\":20,\"rows\":5}}\n",
            "[0.5,\"o\"]\n",
        );
        assert!(matches!(Cast::parse(input), Err(PlaybackError::Parse(_))));
    }

    #[test]
    fn parse_reads_resize_and_other_event_kinds() {
        let input = concat!(
            "{\"version\":3,\"term\":{\"cols\":20,\"rows\":5}}\n",
            "[0.1,\"i\",\"x\"]\n",
            "[0.2,\"r\",\"80x24\"]\n",
            "[0.3,\"m\",\"chapter 1\"]\n",
            "[0.4,\"x\",\"0\"]\n",
        );
        let cast = Cast::parse(input).unwrap();
        let kinds: Vec<EventKind> = cast.events.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EventKind::Input,
                EventKind::Resize,
                EventKind::Marker,
                EventKind::Exit,
            ]
        );
    }

    #[test]
    fn snapshot_renders_final_screen_text() {
        let cast = Cast::parse(SAMPLE).unwrap();
        let screen = snapshot(&cast);
        // "hello" on row 0, "world" on row 1; trailing blank rows trimmed.
        assert_eq!(screen, "hello\nworld");
    }

    #[test]
    fn snapshot_reflects_overwrites_and_cursor_moves() {
        // \r returns to column 0 so "ABC" overwrites the first three of "hello".
        let input = concat!(
            "{\"version\":3,\"term\":{\"cols\":20,\"rows\":3}}\n",
            "[0.1,\"o\",\"hello\"]\n",
            "[0.1,\"o\",\"\\rABC\"]\n",
        );
        let cast = Cast::parse(input).unwrap();
        assert_eq!(snapshot(&cast), "ABClo");
    }

    #[test]
    fn snapshot_honors_resize_events() {
        // Resize down to two rows, then print four lines. With only two rows the
        // terminal scrolls and the snapshot shows the last two; at the original
        // five rows all four would remain. This proves the resize took effect.
        let input = concat!(
            "{\"version\":3,\"term\":{\"cols\":20,\"rows\":5}}\n",
            "[0.1,\"r\",\"20x2\"]\n",
            "[0.1,\"o\",\"a\\r\\nb\\r\\nc\\r\\nd\"]\n",
        );
        let cast = Cast::parse(input).unwrap();
        assert_eq!(snapshot(&cast), "c\nd");
    }

    #[test]
    fn render_feeds_raw_bytes() {
        assert_eq!(render("hi\r\nthere", 20, 3), "hi\nthere");
    }

    #[test]
    fn frames_emit_one_per_output_event_with_relative_delays() {
        let cast = Cast::parse(SAMPLE).unwrap();
        let frames = frames(&cast);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].delay, Duration::from_secs_f64(0.5));
        assert_eq!(frames[0].data, "hello");
        assert_eq!(frames[1].delay, Duration::from_secs_f64(0.5));
        assert_eq!(frames[2].delay, Duration::from_secs_f64(1.0));
        assert_eq!(frames[2].data, "world");
    }

    #[test]
    fn frames_fold_non_output_intervals_into_next_frame() {
        // The input event's 0.25s interval must be added to the following
        // output frame's delay so wall-clock pacing is preserved.
        let input = concat!(
            "{\"version\":3,\"term\":{\"cols\":20,\"rows\":5}}\n",
            "[0.25,\"i\",\"x\"]\n",
            "[0.50,\"o\",\"hi\"]\n",
        );
        let cast = Cast::parse(input).unwrap();
        let frames = frames(&cast);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].delay, Duration::from_secs_f64(0.75));
        assert_eq!(frames[0].data, "hi");
    }

    #[test]
    fn frames_drive_a_sleepless_write_loop_in_order() {
        // `replay()` is a thin wrapper that sleeps for each frame's delay then
        // writes its data. To keep the test path free of real timing we drive
        // the pure `frames()` schedule directly with no sleeping, asserting the
        // exact bytes and ordering `replay()` would produce.
        let cast = Cast::parse(SAMPLE).unwrap();
        let mut out = Vec::new();
        for frame in frames(&cast) {
            out.write_all(frame.data.as_bytes()).unwrap();
        }
        assert_eq!(String::from_utf8(out).unwrap(), "hello\r\nworld");
    }

    #[test]
    fn parse_resize_handles_valid_and_invalid() {
        assert_eq!(parse_resize("80x24"), Some((80, 24)));
        // Strict format: surrounding whitespace is rejected, not trimmed.
        assert_eq!(parse_resize("132 x 50"), None);
        assert_eq!(parse_resize("nope"), None);
        assert_eq!(parse_resize("80x"), None);
    }
}
