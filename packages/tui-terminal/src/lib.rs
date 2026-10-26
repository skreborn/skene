//! # skene-tui-terminal
//!
//! The terminal's realization of `skene-tui`: [`Terminal`], its escape trait and its
//! seams beyond the [`Realization`](skene_foundation::Realization) trait, down to the
//! terminal device.
//!
//! An app names no item here: it depends on `skene-tui`, which names [`Terminal`] as its
//! `Active`, re-exports the launch's surface at its root and this crate's own vocabulary
//! as `skene_tui::platform::terminal`, where a library author writing a view against the
//! terminal's cells reaches it. The realization lays a frame out through taffy at
//! `1.0 = 1 cell`, lays its text out through Parley in the [`CellEngine`], paints it into
//! a cell buffer and writes it to a [`Device`], and its `device`, `tty`, `parse` and
//! `escape` modules are the whole terminal layer: the device a frame is written to and
//! read from, the host terminal's raw mode and signals, the parser of what the terminal
//! sends and the escape sequences written to it.
//!
//! [`launch`] and its [`Launch`] run an app in a terminal, [`install`], [`mount`] and
//! [`uninstall`] host one in-process for a test, and [`testing`] holds the test seams
//! over them. A low-level view paints through [`CellPainter`] with [`Cells`], and the
//! terminal has no toolkit's view to adopt, so its native view is [`Unadoptable`].
//!
//! The crate compiles in a build naming `skene_platform = "terminal"` alone and is empty
//! in every other; the facade refuses a build it cannot realize.

#![cfg(skene_platform = "terminal")]

pub mod testing;

mod cells;
mod detect;
mod dev;
mod device;
mod engine;
mod escape;
mod event_loop;
mod field;
mod frame;
mod interaction;
mod launch;
mod layout;
mod motion;
mod paint;
mod parse;
mod realize;
mod records;
mod resolve;
mod selection;
mod text;
mod tty;

pub(crate) use self::realize::{try_with_records, with_records};
pub use self::{
  cells::{CellPainter, Cells},
  dev::{adopt_restoration, notice_view as dev_notice_view},
  device::{Device, Note, WindowSize},
  engine::{CellEngine, StandIns},
  launch::{Clipboard, Launch, MOUSE_KEY, install, launch},
  paint::{CellDecoration, Picture},
  realize::{mount, uninstall},
  text::CellScale,
};

/// The terminal platform: a zero-sized
/// [`Realization`](skene_foundation::Realization).
///
/// Lengths are whole cells ([`Scalar`](skene_foundation::Realization::Scalar) `i32`),
/// the style extension is [`TerminalStyle`](skene_foundation::terminal::TerminalStyle),
/// and a text lays out through the [`CellEngine`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Terminal;

/// The terminal's native view: none, so the native door cannot be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unadoptable {}
