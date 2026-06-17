//! asciicast v3 types and writer trait.
//!
//! Surface only — the concrete streaming writer lands in aterm-8tn.2.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors produced while writing an asciicast stream.
#[derive(Debug, Error)]
pub enum CastError {
    /// Underlying I/O failure.
    #[error("asciicast write failed")]
    Io(#[from] std::io::Error),
}

/// asciicast v3 stream header (first line of the file).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    /// Format version; always 3 for v3.
    pub version: u8,
    /// Initial terminal geometry.
    pub term: Term,
    // TODO(aterm-8tn.2): timestamp, env, idle_time_limit, title, theme...
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: 3,
            term: Term::default(),
        }
    }
}

/// Terminal geometry / metadata for the asciicast header.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Term {
    pub cols: u16,
    pub rows: u16,
}

/// The kind of an asciicast v3 event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    /// Data printed to the terminal (`"o"`).
    Output,
    /// Data typed by the user (`"i"`).
    Input,
    /// Terminal resize (`"r"`).
    Resize,
    /// User-defined marker (`"m"`).
    Marker,
}

/// A single asciicast v3 event: an interval since the previous event, a kind,
/// and its payload.
#[derive(Debug, Clone)]
pub struct Event {
    /// Seconds elapsed since the previous event.
    pub interval: f64,
    /// Event kind.
    pub kind: EventKind,
    /// Event payload (output bytes, input bytes, `"<cols>x<rows>"`, etc.).
    pub data: String,
}

/// Writes an asciicast v3 stream: one header followed by zero or more events.
pub trait CastWriter {
    /// Writes the stream header. Must be called exactly once, first.
    fn write_header(&mut self, header: &Header) -> Result<(), CastError>;

    /// Appends a single event to the stream.
    fn write_event(&mut self, event: &Event) -> Result<(), CastError>;
}
