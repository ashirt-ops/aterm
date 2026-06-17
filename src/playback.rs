//! Optional playback support (`--features playback`).
//!
//! Drives recorded asciicast output through the `avt` virtual terminal so a
//! recording can be rendered / verified. Gated behind the optional `avt`
//! dependency and off by default.

/// Renders asciicast output `bytes` through a virtual terminal and returns the
/// final screen contents.
// TODO(aterm-8tn.N): feed events through `avt::Vt` and dump the screen.
pub fn render(_bytes: &str, _cols: usize, _rows: usize) -> String {
    let _vt = avt::Vt::builder().size(_cols, _rows).build();
    todo!("playback: drive avt and dump the rendered screen")
}
