//! The terminal platform: [`Terminal`], its escape trait and its seams beyond the
//! [`Realization`](skene_foundation::Realization) trait.
//!
//! For a library author writing a view directly against the terminal's cells. A
//! low-level view, a `View<Active>` like any other, paints through [`CellPainter`] with
//! [`Cells`], the handle addressing its box's cells from their own origin, and the
//! module exists in the terminal's build alone, so cell-reaching code compiles for the
//! terminal alone. The terminal has no toolkit's view to adopt, so its native view is
//! [`Unadoptable`] and the native door cannot be reached there. Its text engine is the
//! [`CellEngine`], which every text lays out through and its drawn entries edit over.
//!
//! The terminal implements [`Realization`](skene_foundation::Realization) for
//! [`Terminal`] and exposes its own entry points beside it: [`install`], [`mount`] and
//! [`uninstall`] for an in-process test, and [`adopt_restoration`] and
//! [`dev_notice_view`] for a dev reload's restore and its rebuild notice. The facade's
//! root keeps [`launch`](crate::launch) and its [`Launch`](crate::Launch), which install
//! and mount for a running app.

pub use crate::{
  cells::{CellPainter, Cells},
  dev::{adopt_restoration, notice_view as dev_notice_view},
  engine::{CellEngine, StandIns},
  launch::install,
  realize::{mount, uninstall},
};

/// The terminal platform: a zero-sized
/// [`Realization`](skene_foundation::Realization).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Terminal;

/// The terminal's native view: none, so the native door cannot be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unadoptable {}
