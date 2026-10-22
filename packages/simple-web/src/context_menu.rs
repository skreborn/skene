//! A ready-made context menu in the Simple design language.

use skene_foundation::{AnyView, NoChild, View, Wraps};
use skene_simple::SimpleContextMenuBase;
use skene_web::Active;

/// A ready-made context menu.
///
/// A card-colored list with a hairline border, its items padded like a picker's rows
/// and lit in the secondary surface under the pointer or focus, a disabled item in the
/// muted foreground, and a separator in the border color.
///
/// [`set_items`](Self::set_items) lists the items; the view served comes last through
/// [`set_child`](Self::set_child), so any view takes a menu by wrapping, or
/// [`serve`](Self::serve) attaches it to a node that already exists. For anything
/// beyond that, compose [`ContextMenu`](skene_web::ContextMenu) directly.
///
/// ```
/// use skene_simple_web::prelude::*;
/// use skene_web::prelude::*;
///
/// let text = SimpleContextMenu::new()
///   .set_items([MenuEntry::copy(), MenuEntry::paste()])
///   .set_child(Text::new("Right-click me"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct SimpleContextMenu<C = NoChild>(skene_simple::SimpleContextMenu<Active, C>);

impl Default for SimpleContextMenu {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl SimpleContextMenuBase<Active> for SimpleContextMenu {}

impl SimpleContextMenu {
  /// Sets the view the menu serves.
  #[inline(always)]
  pub fn set_child(self, child: impl View<Active>) -> SimpleContextMenu<AnyView<Active>> {
    SimpleContextMenu(self.0.set_child(child))
  }
}
