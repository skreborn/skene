//! The terminal platform's integration tests, in one binary, host-only as the platform
//! is.

#![cfg(not(target_arch = "wasm32"))]

mod common;

mod r#async;
mod backdrop;
mod clusters;
mod conditions;
mod conformance;
mod context_menu;
mod controls;
mod decoration;
mod enhance;
mod entry_selection;
mod launch;
mod layout;
mod links;
mod motion;
mod overlays;
mod painted;
mod pointer;
mod pty;
mod rendering;
mod scrolling;
mod selection;
mod sizing;
mod snapshot;
mod text_entry;
mod title;
mod tokens;
mod tooltips;
mod user;
