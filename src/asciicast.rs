//! asciicast **v3** types and streaming writer.
//!
//! This module implements the [asciicast v3] file format directly — `asciinema`
//! is not a dependency. A cast is newline-delimited JSON:
//!
//!   * **Line 1** is the [`Header`] object: `{"version":3,"term":{...},...}`.
//!   * **Following lines** are event arrays `[interval, code, data]`.
//!   * Lines beginning with `#` are comments; the first line must not be one.
//!
//! The defining change from v2 is *relative* timing: each event's `interval` is
//! the number of seconds elapsed since the **previous** event (v2 used absolute
//! timestamps), and there is no top-level `duration`. Terminal geometry is nested
//! under `term {cols, rows}` rather than top-level `width`/`height`.
//!
//! [`Writer`] is the streaming entry point: it is fed events stamped with their
//! **absolute** time, computes the relative interval from the previous event
//! (the first interval is measured from the writer's start time), and emits the
//! header followed by event lines to any [`std::io::Write`].
//!
//! [asciicast v3]: https://docs.asciinema.org/manual/asciicast/v3/

use std::collections::BTreeMap;
use std::io::Write;

use serde::ser::{SerializeTuple, Serializer};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced while writing an asciicast stream.
#[derive(Debug, Error)]
pub enum CastError {
    /// Underlying I/O failure.
    #[error("asciicast write failed")]
    Io(#[from] std::io::Error),
    /// JSON serialization of the header or an event failed.
    #[error("asciicast serialization failed")]
    Serde(#[from] serde_json::Error),
    /// An event or comment was written before the header. The header must be
    /// the first line of a v3 stream.
    #[error("asciicast header must be written before any event or comment")]
    HeaderNotWritten,
}

/// asciicast v3 stream header (the first line of the file).
///
/// Optional fields are omitted from the JSON when unset so a minimal header
/// serializes to just `{"version":3,"term":{"cols":N,"rows":M}}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    /// Format version; always `3` for v3.
    pub version: u8,
    /// Initial terminal geometry and metadata.
    pub term: Term,
    /// Capture time as a Unix timestamp (whole seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    /// Captured environment variables (e.g. `SHELL`, `TERM`).
    ///
    /// A [`BTreeMap`] is used so serialization is deterministic, which keeps
    /// golden tests and on-disk casts stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<BTreeMap<String, String>>,
    /// Human-readable recording title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Free-form tags attached to the recording.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl Header {
    /// Creates a minimal header for a terminal of `cols` x `rows`.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            version: 3,
            term: Term::new(cols, rows),
            timestamp: None,
            env: None,
            title: None,
            tags: None,
        }
    }
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: 3,
            term: Term::default(),
            timestamp: None,
            env: None,
            title: None,
            tags: None,
        }
    }
}

/// Terminal geometry / metadata for the asciicast header (`term` object).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Term {
    /// Width in columns.
    pub cols: u16,
    /// Height in rows.
    pub rows: u16,
    /// Terminal type, e.g. `"xterm-256color"` (serialized as `type`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub term_type: Option<String>,
    /// Color theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
}

impl Term {
    /// Creates terminal metadata for a `cols` x `rows` grid with no type/theme.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            term_type: None,
            theme: None,
        }
    }
}

/// asciicast color theme (`term.theme` object).
///
/// `fg`/`bg` are `#RRGGBB` strings and `palette` is a colon-separated list of
/// such colors, matching the v3 spec; both are optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Theme {
    /// Default foreground color (`#RRGGBB`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    /// Default background color (`#RRGGBB`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// Colon-separated palette of `#RRGGBB` colors (8, 16, or 256 entries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub palette: Option<String>,
}

/// The kind of an asciicast v3 event, with its single-character code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Data printed to the terminal (`"o"`).
    Output,
    /// Data typed by the user (`"i"`).
    Input,
    /// Terminal resize (`"r"`); data is `"<cols>x<rows>"`.
    Resize,
    /// User-defined marker (`"m"`).
    Marker,
    /// Process exit (`"x"`); data is the exit status as a string.
    Exit,
}

impl EventKind {
    /// The single-character event code used in the serialized array.
    pub fn code(self) -> &'static str {
        match self {
            EventKind::Output => "o",
            EventKind::Input => "i",
            EventKind::Resize => "r",
            EventKind::Marker => "m",
            EventKind::Exit => "x",
        }
    }
}

/// A single asciicast v3 event: an interval since the previous event, a kind,
/// and its payload.
///
/// Serializes to a three-element JSON array `[interval, code, data]`, e.g.
/// `[0.5,"o","hello"]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    /// Seconds elapsed since the previous event (relative timing).
    pub interval: f64,
    /// Event kind.
    pub kind: EventKind,
    /// Event payload (output bytes, input bytes, `"<cols>x<rows>"`, exit status).
    pub data: String,
}

impl Event {
    /// Creates an event with an explicit relative `interval`.
    pub fn new(interval: f64, kind: EventKind, data: impl Into<String>) -> Self {
        Self {
            interval,
            kind,
            data: data.into(),
        }
    }

    /// Formats a resize payload as `"<cols>x<rows>"`.
    pub fn resize_data(cols: u16, rows: u16) -> String {
        format!("{cols}x{rows}")
    }
}

impl Serialize for Event {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&self.interval)?;
        tuple.serialize_element(self.kind.code())?;
        tuple.serialize_element(&self.data)?;
        tuple.end()
    }
}

/// Streaming asciicast v3 writer over any [`Write`].
///
/// Construct with a session start time, [`write_header`](Writer::write_header)
/// exactly once, then [`write_event`](Writer::write_event) for each event using
/// its **absolute** timestamp. The writer tracks the previous timestamp and
/// emits the relative interval; the first event's interval is measured from the
/// start time supplied at construction.
pub struct Writer<W: Write> {
    inner: W,
    /// Absolute time of the most recent event (or the start time before any).
    last_time: f64,
    header_written: bool,
}

impl<W: Write> Writer<W> {
    /// Creates a writer whose first event interval is measured from `start_time`
    /// (absolute seconds, same clock as the timestamps passed to
    /// [`write_event`](Writer::write_event)).
    pub fn new(inner: W, start_time: f64) -> Self {
        Self {
            inner,
            last_time: start_time,
            header_written: false,
        }
    }

    /// Writes the stream header. Must be called exactly once, before any event.
    pub fn write_header(&mut self, header: &Header) -> Result<(), CastError> {
        serde_json::to_writer(&mut self.inner, header)?;
        self.inner.write_all(b"\n")?;
        self.header_written = true;
        Ok(())
    }

    /// Writes an event stamped with its absolute `time`, emitting the interval
    /// relative to the previous event (or the start time for the first event).
    ///
    /// Returns [`CastError::HeaderNotWritten`] if called before the header.
    pub fn write_event(
        &mut self,
        time: f64,
        kind: EventKind,
        data: impl Into<String>,
    ) -> Result<(), CastError> {
        if !self.header_written {
            return Err(CastError::HeaderNotWritten);
        }
        let interval = time - self.last_time;
        self.last_time = time;
        let event = Event::new(interval, kind, data);
        serde_json::to_writer(&mut self.inner, &event)?;
        self.inner.write_all(b"\n")?;
        Ok(())
    }

    /// Writes a pre-computed [`Event`] verbatim (its `interval` is used as-is and
    /// the internal clock is not advanced). Prefer
    /// [`write_event`](Writer::write_event) for live recording.
    pub fn write_raw_event(&mut self, event: &Event) -> Result<(), CastError> {
        if !self.header_written {
            return Err(CastError::HeaderNotWritten);
        }
        serde_json::to_writer(&mut self.inner, event)?;
        self.inner.write_all(b"\n")?;
        Ok(())
    }

    /// Writes a comment line (`# <text>`). The header must already be written so
    /// that the first line of the stream is never a comment.
    pub fn write_comment(&mut self, text: &str) -> Result<(), CastError> {
        if !self.header_written {
            return Err(CastError::HeaderNotWritten);
        }
        writeln!(self.inner, "# {text}")?;
        Ok(())
    }

    /// Flushes and returns the underlying writer.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Borrows the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn header_serializes_with_nested_term_and_version_3() {
        let header = Header::new(80, 24);
        let json: Value = serde_json::from_str(&serde_json::to_string(&header).unwrap()).unwrap();

        assert_eq!(json["version"], 3);
        // v3 nests geometry under `term` — there is no top-level width/height.
        assert_eq!(json["term"]["cols"], 80);
        assert_eq!(json["term"]["rows"], 24);
        assert!(json.get("width").is_none());
        assert!(json.get("height").is_none());
        // No `duration` field exists in v3.
        assert!(json.get("duration").is_none());
        // Unset optionals are omitted.
        assert!(json.get("timestamp").is_none());
        assert!(json.get("env").is_none());
    }

    #[test]
    fn header_serializes_optional_fields() {
        let mut env = BTreeMap::new();
        env.insert("SHELL".to_string(), "/bin/zsh".to_string());
        env.insert("TERM".to_string(), "xterm-256color".to_string());
        let header = Header {
            version: 3,
            term: Term {
                cols: 120,
                rows: 40,
                term_type: Some("xterm-256color".to_string()),
                theme: Some(Theme {
                    fg: Some("#ffffff".to_string()),
                    bg: Some("#000000".to_string()),
                    palette: None,
                }),
            },
            timestamp: Some(1_700_000_000),
            env: Some(env),
            title: Some("demo".to_string()),
            tags: Some(vec!["a".to_string(), "b".to_string()]),
        };
        let json: Value = serde_json::from_str(&serde_json::to_string(&header).unwrap()).unwrap();

        assert_eq!(json["term"]["type"], "xterm-256color");
        assert_eq!(json["term"]["theme"]["fg"], "#ffffff");
        assert!(json["term"]["theme"].get("palette").is_none());
        assert_eq!(json["timestamp"], 1_700_000_000);
        assert_eq!(json["env"]["SHELL"], "/bin/zsh");
        assert_eq!(json["title"], "demo");
        assert_eq!(json["tags"][1], "b");
    }

    #[test]
    fn event_serializes_as_interval_code_data_array() {
        let event = Event::new(0.5, EventKind::Output, "hello");
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"[0.5,"o","hello"]"#
        );

        // Round-trips back into a 3-element JSON array.
        let json: Value = serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 3);
        assert_eq!(json[0], 0.5);
        assert_eq!(json[1], "o");
        assert_eq!(json[2], "hello");
    }

    #[test]
    fn event_codes_map_to_single_chars() {
        assert_eq!(EventKind::Output.code(), "o");
        assert_eq!(EventKind::Input.code(), "i");
        assert_eq!(EventKind::Resize.code(), "r");
        assert_eq!(EventKind::Marker.code(), "m");
        assert_eq!(EventKind::Exit.code(), "x");
    }

    #[test]
    fn writer_computes_relative_intervals_not_absolute_times() {
        // Absolute timestamps; start at t=0 so the first interval is the first
        // timestamp itself, and each subsequent interval is a delta.
        let mut buf = Vec::new();
        {
            let mut w = Writer::new(&mut buf, 0.0);
            w.write_header(&Header::new(80, 24)).unwrap();
            w.write_event(1.0, EventKind::Output, "a").unwrap();
            w.write_event(1.5, EventKind::Output, "b").unwrap();
            w.write_event(3.0, EventKind::Input, "c").unwrap();
        }
        let text = String::from_utf8(buf).unwrap();
        let mut lines = text.lines();

        // Line 1 is the header (not an event).
        let header: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(header["version"], 3);

        let intervals: Vec<f64> = lines
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()[0]
                    .as_f64()
                    .unwrap()
            })
            .collect();
        // Deltas: 1.0-0.0, 1.5-1.0, 3.0-1.5 — NOT the absolute 1.0/1.5/3.0.
        assert_eq!(intervals, vec![1.0, 0.5, 1.5]);
    }

    #[test]
    fn writer_first_interval_measured_from_start_time() {
        // A non-zero start time: the first event's interval is relative to it.
        let mut buf = Vec::new();
        {
            let mut w = Writer::new(&mut buf, 100.0);
            w.write_header(&Header::new(80, 24)).unwrap();
            w.write_event(100.25, EventKind::Output, "x").unwrap();
        }
        let text = String::from_utf8(buf).unwrap();
        let first_event: Value = serde_json::from_str(text.lines().nth(1).unwrap()).unwrap();
        assert_eq!(first_event[0], 0.25);
    }

    #[test]
    fn resize_and_exit_events_render_correctly() {
        let mut buf = Vec::new();
        {
            let mut w = Writer::new(&mut buf, 0.0);
            w.write_header(&Header::new(80, 24)).unwrap();
            w.write_event(0.5, EventKind::Resize, Event::resize_data(132, 50))
                .unwrap();
            w.write_event(1.0, EventKind::Exit, "0").unwrap();
        }
        let text = String::from_utf8(buf).unwrap();
        let resize: Value = serde_json::from_str(text.lines().nth(1).unwrap()).unwrap();
        let exit: Value = serde_json::from_str(text.lines().nth(2).unwrap()).unwrap();

        assert_eq!(resize[1], "r");
        assert_eq!(resize[2], "132x50");
        assert_eq!(exit[1], "x");
        assert_eq!(exit[2], "0");
    }

    #[test]
    fn full_stream_matches_golden_bytes() {
        // Exact-bytes golden: a minimal cast with a header line, two output
        // events, and a comment. Intervals chosen to be exact in binary f64.
        let mut buf = Vec::new();
        {
            let mut w = Writer::new(&mut buf, 0.0);
            w.write_header(&Header::new(80, 24)).unwrap();
            w.write_comment("recorded by aterm").unwrap();
            w.write_event(0.5, EventKind::Output, "hi").unwrap();
            w.write_event(1.0, EventKind::Output, "bye").unwrap();
        }
        let text = String::from_utf8(buf).unwrap();
        let expected = concat!(
            "{\"version\":3,\"term\":{\"cols\":80,\"rows\":24}}\n",
            "# recorded by aterm\n",
            "[0.5,\"o\",\"hi\"]\n",
            "[0.5,\"o\",\"bye\"]\n",
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn first_line_is_header_never_a_comment() {
        // Writing a comment or event before the header is rejected, guaranteeing
        // the first line of the stream is the header.
        let mut buf = Vec::new();
        let mut w = Writer::new(&mut buf, 0.0);
        assert!(matches!(
            w.write_comment("nope"),
            Err(CastError::HeaderNotWritten)
        ));
        assert!(matches!(
            w.write_event(0.0, EventKind::Output, "nope"),
            Err(CastError::HeaderNotWritten)
        ));
    }
}
