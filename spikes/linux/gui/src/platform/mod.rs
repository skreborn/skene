//! Each platform's own vocabulary, as std's `os` holds each operating system's.
//!
//! A platform's own controls (`linux::Switch`) exist in that platform's build alone, so
//! naming one outside it fails to compile; each exposes its toolkit's options through
//! setters, under Skene's names where the meaning matches (`derive_on`, `handle_change`,
//! `set_placeholder`) and the toolkit's own in its documentation. A library wraps a toolkit
//! widget of its own through the platform's native door (`linux::NativeView`).

#[cfg(skene_platform = "linux")]
pub mod linux;
