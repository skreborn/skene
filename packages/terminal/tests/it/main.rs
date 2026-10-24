//! The terminal platform's integration tests, in one binary, built in the terminal's
//! build alone as the platform is.

#![cfg(skene_platform = "terminal")]

mod common;

mod r#async;
mod backdrop;
mod clock;
mod clusters;
mod conditions;
mod conformance;
mod container;
mod context_menu;
mod controls;
mod decoration;
mod dispatch;
mod enhance;
mod entry_selection;
mod incremental;
mod keymap;
mod launch;
mod layout;
mod links;
mod motion;
mod names;
mod overlays;
mod painted;
mod picker;
mod platform;
mod pointer;
mod pty;
mod rendering;
mod scrolling;
mod selection;
mod sizing;
mod snapshot;
mod structure;
mod text;
mod text_entry;
mod title;
mod tokens;
mod tooltips;
mod user;
