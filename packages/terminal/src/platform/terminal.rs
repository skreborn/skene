//! The terminal platform: [`Terminal`] and its escape trait.
//!
//! For a library author writing a view directly against the terminal's cells. A
//! low-level view states [`CellPainter`] as its bound and paints through [`Cells`], the
//! handle addressing its box's cells from their own origin, so cell-reaching code
//! compiles for the terminal alone.

pub use crate::cells::{CellPainter, Cells};

/// The terminal platform: a zero-sized
/// [`Realization`](skene_foundation::Realization).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct Terminal;
