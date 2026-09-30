//! A user's gestures on the terminal, each the input the terminal reports for it,
//! through the `trigger_` seams.
//!
//! A pointer acts at the center cell of the rectangle a node covered at the last paint;
//! a key acts where focus is.

use skene_core::{Key, KeyEvent, NodeId};
use unicode_segmentation::UnicodeSegmentation;

use super::{trigger_key_event, trigger_mouse_click, trigger_mouse_move};
use crate::with_store;

/// The center cell of the rectangle `node` covered at the last paint.
///
/// # Panics
///
/// Panics where `node` covered no cell: a user reaches only what is on screen.
fn cell_of(node: NodeId) -> (u16, u16) {
  let rect = with_store(|store| store.record(node).and_then(|record| record.painted));
  let center = rect
    .filter(|(_, _, width, height)| *width > 0 && *height > 0)
    .and_then(|(x, y, width, height)| {
      Some((
        u16::try_from(x + width / 2).ok()?,
        u16::try_from(y + height / 2).ok()?,
      ))
    });

  center.unwrap_or_else(|| panic!("{node:?} covers no cell on screen"))
}

/// Moves the pointer over `node`.
///
/// # Panics
///
/// Panics where `node` covers no cell on screen.
pub fn hover(node: NodeId) {
  let (x, y) = cell_of(node);

  trigger_mouse_move(x, y);
}

/// Clicks `node` with the primary button: the pointer moves over it, presses, moving
/// focus as the press does, and releases.
///
/// # Panics
///
/// Panics where `node` covers no cell on screen.
pub fn click(node: NodeId) {
  let (x, y) = cell_of(node);

  trigger_mouse_move(x, y);
  trigger_mouse_click(x, y);
}

/// Double-clicks `node`: two clicks on one cell, the second counting two.
///
/// # Panics
///
/// Panics where `node` covers no cell on screen.
pub fn double_click(node: NodeId) {
  click(node);
  click(node);
}

/// Presses and releases the key `event` describes where focus is.
pub fn press_key(event: &KeyEvent) {
  trigger_key_event(event);
}

/// Clicks the text entry `node` and types `text` into it, one grapheme cluster at a time,
/// each a key press, a newline pressing `Enter`.
///
/// # Panics
///
/// Panics where `node` covers no cell on screen.
pub fn type_text(node: NodeId, text: &str) {
  click(node);

  for cluster in text.graphemes(true) {
    let event = match cluster {
      "\n" | "\r\n" => KeyEvent::new("Enter"),
      _ => KeyEvent {
        key: Key::Character(cluster.to_owned()),
        ..KeyEvent::new(cluster)
      },
    };

    press_key(&event);
  }
}
