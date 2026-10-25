//! The terminal platform: [`Terminal`] and its escape trait.
//!
//! For a library author writing a view directly against the terminal's cells. A
//! low-level view, a `View<Active>` like any other, paints through [`CellPainter`] with
//! [`Cells`], the handle addressing its box's cells from their own origin, and the
//! module exists in the terminal's build alone, so cell-reaching code compiles for the
//! terminal alone. The terminal has no toolkit's view to adopt, so its
//! native view is [`Unadoptable`] and the native door cannot be reached there.

pub use crate::cells::{CellPainter, Cells};

/// The terminal platform: a zero-sized
/// [`Realization`](skene_foundation::Realization).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Terminal;

/// The terminal's native view: none, so the native door cannot be reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unadoptable {}
