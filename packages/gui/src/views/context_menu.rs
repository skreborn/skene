//! Commands shown beside the pointer on a secondary press over the view it serves.

use skene_foundation::{ContextMenuBase, DecorationBase, View, Wraps};

use crate::Active;

/// A context menu of [`MenuEntry`](skene_foundation::MenuEntry)s over the view it serves.
///
/// A secondary press over the view opens it beside the pointer, and `Shift-F10` or the
/// `ContextMenu` key beside the view while focus is inside it; the platform's own menu
/// stays everywhere the view does not reach.
///
/// The menu is a popup [`Overlay`](crate::Overlay), the
/// [`Part::List`](skene_foundation::Part::List), holding a [`Menu`](skene_foundation::Menu) of
/// [`MenuOption`](skene_foundation::MenuOption) controls, the
/// [`Part::Item`](skene_foundation::Part::Item)s: the arrows, `Home`, `End`, typing, `Enter` and
/// `Escape` come from those behaviors, an item runs on release as a button does and closes the
/// menu, and a press outside dismisses it. The deepest menu along the path from the pressed node
/// wins, so a menu on a root serves everything no closer menu does. A menu whose items are all
/// disabled or absent does not open; the platform's menu is prevented either way.
///
/// The list is decorated like a container and every item per property through the
/// `set_item_*` setters. Without the view [`set_child`](Self::set_child) supplies, it
/// renders nothing; [`serve`](Self::serve) attaches the menu to a node that already
/// exists instead.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let editor = ContextMenu::new()
///   .set_items([
///     MenuEntry::copy(),
///     MenuEntry::separator(),
///     MenuEntry::new("Inspect"),
///   ])
///   .set_child(Text::new("Right-click me"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct ContextMenu(skene_foundation::ContextMenu<Active>);

impl Default for ContextMenu {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl ContextMenuBase<Active> for ContextMenu {}

impl DecorationBase<Active> for ContextMenu {}
