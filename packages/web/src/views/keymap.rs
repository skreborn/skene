//! Keyboard shortcuts over the view it serves.

use skene_foundation::{AnyView, KeymapBase, NoChild, View, Wraps};

use crate::Active;

/// Keyboard shortcuts over the one view they serve.
///
/// A shortcut pressed while focus is inside the view runs the action bound to it, and
/// the platform's own response to the key and every shortcut an ancestor binds are
/// passed over. Holds [`NoChild`] until [`set_child`](Self::set_child) supplies the view
/// served; [`serve`](Self::serve) binds the shortcuts on a node that already exists
/// instead.
///
/// ```
/// use skene_web::prelude::*;
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
pub struct Keymap<C = NoChild>(skene_foundation::Keymap<Active, C>);

impl Default for Keymap {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl KeymapBase<Active> for Keymap {}

impl Keymap {
  /// Sets the view the keymap serves.
  #[inline(always)]
  pub fn set_child(self, child: impl View<Active>) -> Keymap<AnyView<Active>> {
    Keymap(self.0.set_child(child))
  }
}
