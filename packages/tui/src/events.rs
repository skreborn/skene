//! The terminal's pointer event, in cells.

use skene_core::{Modifiers, PointerButton};

/// A pointer event: where in the terminal, which button, and the modifier state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct PointerEvent {
  /// The pointer's column, in cells from the terminal's left edge.
  pub column: u16,
  /// The pointer's row, in cells from the terminal's top edge.
  pub row: u16,
  /// The pointer's column, relative to the handling view's left edge.
  pub local_column: u16,
  /// The pointer's row, relative to the handling view's top edge.
  pub local_row: u16,
  /// The button that changed state, or `None` for a move.
  pub button: Option<PointerButton>,
  /// The modifier state.
  pub modifiers: Modifiers,
}
