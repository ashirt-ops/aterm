//! ASHIRT API integration.
//!
//! Split into focused submodules so downstream issues land independently:
//!   * [`signing`]  — HMAC-SHA256 request signing (aterm-8tn.3)
//!   * [`http`]     — blocking HTTP client base (aterm-8tn.8)
//!   * [`upload`]     — multipart evidence upload
//!   * [`operations`] — operations API
//!   * [`tags`]       — tags API

pub mod http;
pub mod operations;
pub mod signing;
pub mod tags;
pub mod upload;
