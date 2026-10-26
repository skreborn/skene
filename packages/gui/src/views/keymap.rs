//! Keyboard shortcuts over the view it serves.

use skene_foundation::{KeymapBase, View, Wraps};

use crate::Active;

/// Keyboard shortcuts over the one view they serve.
///
/// A shortcut pressed while focus is inside the view runs the action bound to it, and
/// the platform's own response to the key and every shortcut an ancestor binds are
/// passed over. Without the view [`set_child`](Self::set_child) supplies, it renders
/// nothing; [`serve`](Self::serve) binds the shortcuts on a node that already exists
/// instead.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let saved = Signal::new(0);
/// let editor = Keymap::new()
///   .handle_shortcut(Shortcut::new(Modifiers::CTRL, "s"), move || {
///     saved.update(|count| *count += 1);
///   })
///   .set_child(Text::new("Press Ctrl-S"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Keymap(skene_foundation::Keymap<Active>);

impl Default for Keymap {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl KeymapBase<Active> for Keymap {}
