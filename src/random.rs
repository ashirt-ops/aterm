//! Small shared randomness helper.
//!
//! Centralizes the project's one need for non-deterministic values — the random
//! recording-file suffix ([`crate::menu::default_output_file_name`]) and the
//! random tag color ([`crate::ashirt::tags::random_tag_color`]). Both previously
//! derived a `u64` from `RandomState::new().build_hasher().finish()`, which
//! relies on unspecified standard-library seeding behavior. This module pulls the
//! value from `getrandom` instead, a real OS randomness source.

/// Returns a random `u64` sourced from the operating system's randomness
/// facility via [`getrandom`].
///
/// # Panics
///
/// Panics if the OS randomness source is unavailable. Both call sites use this
/// for best-effort uniqueness (a file-name suffix and a cosmetic color), and a
/// system with no working entropy source is not a state this tool can sensibly
/// continue in, so failing loudly is preferable to silently degrading.
pub fn random_u64() -> u64 {
    let mut buf = [0u8; 8];
    getrandom::fill(&mut buf).expect("OS randomness source (getrandom) unavailable");
    u64::from_ne_bytes(buf)
}
