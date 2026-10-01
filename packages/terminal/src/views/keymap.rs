//! Keyboard shortcuts over the view it serves.

use skene_foundation::NoChild;

use crate::ActivePlatform;

/// Keyboard shortcuts over the one view they serve.
///
/// A shortcut pressed while focus is inside the view runs the action bound to it, and
/// the platform's own response to the key and every shortcut an ancestor binds are
/// passed over. Holds [`NoChild`] until [`set_child`](Self::set_child) supplies the view
/// served; [`serve`](Self::serve) binds the shortcuts on a node that already exists
/// instead.
#[must_use]
pub struct Keymap<C = NoChild>(skene_foundation::Keymap<ActivePlatform, C>);

skene_foundation::impl_keymap!(Keymap, ActivePlatform);
