//! Keyboard shortcuts over the view it serves.

use skene_foundation::{AnyView, KeymapBase, NoChild, View, Wraps};

use crate::ActivePlatform;

/// Keyboard shortcuts over the one view they serve.
///
/// A shortcut pressed while focus is inside the view runs the action bound to it, and
/// the platform's own response to the key and every shortcut an ancestor binds are
/// passed over. Holds [`NoChild`] until [`set_child`](Self::set_child) supplies the view
/// served; [`serve`](Self::serve) binds the shortcuts on a node that already exists
/// instead.
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Keymap<C = NoChild>(skene_foundation::Keymap<ActivePlatform, C>);

impl Default for Keymap {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl KeymapBase<ActivePlatform> for Keymap {}

impl Keymap {
  /// Sets the view the keymap serves.
  #[inline(always)]
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Keymap<AnyView<ActivePlatform>> {
    Keymap(self.0.set_child(child))
  }
}
