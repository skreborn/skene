//! A user's gestures on the terminal, each the input the terminal reports for it,
//! through the `trigger_` seams.
//!
//! A pointer acts at the center cell of what shows of a node's border box within its
//! clip and the screen, and of a run flowing in a paragraph, of the first piece its flow
//! places it in that shows; a key acts where focus is.

use skene_foundation::{Key, KeyEvent, NodeId};
use unicode_segmentation::UnicodeSegmentation;

use super::{
  trigger_key_event,
  trigger_mouse_click,
  trigger_mouse_move,
  trigger_mouse_press,
  trigger_mouse_release,
};
use crate::{frame::cell_rect, with_records};

/// The center cell of what shows of `node`'s border box within its clip and the screen,
/// or of a run flowing in a paragraph, of the first piece it takes that shows
/// ([`Runtime::shown`](skene_layout::runtime::Runtime::shown)).
///
/// # Panics
///
/// Panics where nothing of `node` shows on screen.
fn cell_of(node: NodeId) -> (u16, u16) {
  let center = with_records(|records| records.runtime.shown(node))
    .map(cell_rect)
    .filter(|(_, _, width, height)| *width > 0 && *height > 0)
    .and_then(|(x, y, width, height)| {
      Some((
        u16::try_from(x + width / 2).ok()?,
        u16::try_from(y + height / 2).ok()?,
      ))
    });

  center.unwrap_or_else(|| panic!("{node:?} shows no cell on screen"))
}

/// Moves the pointer over `node`.
///
/// # Panics
///
/// Panics where nothing of `node` shows on screen.
pub fn hover(node: NodeId) {
  let (x, y) = cell_of(node);

  trigger_mouse_move(x, y);
}

/// Clicks `node` with the primary button: the pointer moves over it, presses, moving
/// focus as the press does, and releases.
///
/// # Panics
///
/// Panics where nothing of `node` shows on screen.
pub fn click(node: NodeId) {
  let (x, y) = cell_of(node);

  trigger_mouse_move(x, y);
  trigger_mouse_click(x, y);
}

/// Presses the primary button on `node` and holds it until [`release`], moving focus as
/// the press does.
///
/// # Panics
///
/// Panics where nothing of `node` shows on screen.
pub fn hold(node: NodeId) {
  let (x, y) = cell_of(node);

  trigger_mouse_move(x, y);
  trigger_mouse_press(x, y);
}

/// Releases the primary button [`hold`] pressed, over `node`, which clicks it where
/// the press landed on it.
///
/// # Panics
///
/// Panics where nothing of `node` shows on screen.
pub fn release(node: NodeId) {
  let (x, y) = cell_of(node);

  trigger_mouse_release(x, y);
}

/// Double-clicks `node`: two clicks on one cell, the second counting two.
///
/// # Panics
///
/// Panics where nothing of `node` shows on screen.
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
/// Panics where nothing of `node` shows on screen.
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
