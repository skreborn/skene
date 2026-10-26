//! A ready-made context menu in the Simple design language.

use skene_foundation::MouseCursor;
use skene_gui::{
  Color,
  ContextMenu,
  ContextMenuBase,
  ContextMenuWithCornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  Inset,
  Spacing,
  State,
  When,
};

use crate::{SimplePalette, shape};

/// A ready-made context menu, a preset of [`ContextMenu`].
///
/// A card-colored list with a hairline border, its items padded like a picker's rows
/// and lit in the secondary surface under the pointer or focus, a disabled item in the
/// muted foreground, and a separator in the border color. [`new`](Self::new) returns
/// the menu, so every member of a context menu applies: its `set_items` lists the
/// items, and the view served comes last through its `set_child`, so any view takes a
/// menu by wrapping. [`styled`](Self::styled) dresses a menu built otherwise. Its
/// colors are [`SimplePalette`]'s tokens, inked in the palette's default unless a
/// [`Theme`](skene_gui::Theme) of the palette above redefines them.
///
/// ```
/// use skene_gui::prelude::*;
/// use skene_simple_gui::prelude::*;
///
/// let text = SimpleContextMenu::new()
///   .set_items([MenuEntry::copy(), MenuEntry::paste()])
///   .set_child(Text::new("Right-click me"));
/// ```
#[derive(Debug)]
pub enum SimpleContextMenu {}

impl SimpleContextMenu {
  /// A menu with no items yet.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new() -> ContextMenu {
    Self::styled(ContextMenu::new())
  }

  /// `menu`'s list on the card surface inside a hairline border, rounded, and its items
  /// per property.
  pub fn styled(menu: ContextMenu) -> ContextMenu {
    let shape = shape();

    menu
      .set_padding(shape.list_inset)
      .set_background(SimplePalette::CARD)
      .set_border_width(Inset::all(1))
      .set_border_color(SimplePalette::BORDER)
      .set_corner_radius(shape.control_radius)
      .set_item_padding(shape.row_inset)
      .set_item_color(
        Color::TRANSPARENT
          .when(State::Hovered, SimplePalette::SECONDARY_SURFACE)
          .when(State::Focused, SimplePalette::SECONDARY_SURFACE),
      )
      .set_item_text_color(
        SimplePalette::FOREGROUND.when(State::Disabled, SimplePalette::MUTED_FOREGROUND),
      )
      .set_shortcut_color(SimplePalette::MUTED_FOREGROUND)
      .set_separator_color(SimplePalette::BORDER)
      .set_shortcut_spacing(Spacing::all(shape.shortcut_spacing))
      .set_item_cursor(MouseCursor::Click)
      .set_item_corner_radius(shape.mark_radius)
  }
}
