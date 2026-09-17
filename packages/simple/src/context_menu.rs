//! A ready-made context menu in the Simple design language.

use skene_common::{ContextMenu, CornerRadius, MenuItem};
use skene_core::{
  AnyKind,
  AnyView,
  Color,
  IntoView,
  MouseCursor,
  NoChild,
  NodeId,
  Platform,
  State,
  View,
  When,
};

use super::{
  SimplePalette,
  shape::{MARK_RADIUS, RADIUS, SimpleUnit, define_defaults, hairline, shape},
};

/// A ready-made context menu: a card-colored list with a hairline border, its items
/// padded like a picker's rows and lit in the secondary surface under the pointer or
/// focus, a disabled item in the muted foreground, and a separator in the border
/// color.
///
/// [`set_items`](Self::set_items) lists the items; the view served comes last through
/// [`set_child`](Self::set_child), so any view takes a menu by wrapping, or
/// [`serve`](Self::serve) attaches it to a node that already exists. For anything
/// beyond that, compose [`ContextMenu`] directly.
#[must_use]
pub struct SimpleContextMenu<P: Platform, C = NoChild> {
  raw: ContextMenu<P, C>,
}

impl<P: Platform> SimpleContextMenu<P> {
  /// A menu with no items yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      raw: ContextMenu::new(),
    }
  }

  /// Sets the items, in order.
  #[inline(always)]
  pub fn set_items(mut self, items: impl IntoIterator<Item = MenuItem>) -> Self {
    self.raw = self.raw.set_items(items);

    self
  }

  /// Sets the view the menu serves.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<P>) -> SimpleContextMenu<P, AnyView<P>> {
    SimpleContextMenu {
      raw: self.raw.set_child(child),
    }
  }

  /// Attaches the menu to `node`, a view that already exists; returns the list's node.
  #[inline(always)]
  pub fn serve(self, node: NodeId) -> NodeId
  where
    P::Scalar: SimpleUnit,
    P::StyleExt: CornerRadius,
  {
    dress(self.raw).serve(node)
  }
}

impl<P: Platform> Default for SimpleContextMenu<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform, C> AnyKind for SimpleContextMenu<P, C> {}

/// Decorates `menu` in the Simple language.
fn dress<P: Platform, C>(menu: ContextMenu<P, C>) -> ContextMenu<P, C>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  define_defaults::<P>();

  let shape = shape::<P>();

  menu
    .set_padding(shape.list_inset)
    .set_color(SimplePalette::<P::Scalar>::CARD)
    .set_border(hairline(SimplePalette::<P::Scalar>::BORDER))
    .set_corner_radius(RADIUS)
    .set_item_padding(shape.row_inset)
    .set_item_color(
      Color::TRANSPARENT
        .when(
          State::Hovered,
          SimplePalette::<P::Scalar>::SECONDARY_SURFACE,
        )
        .when(
          State::Focused,
          SimplePalette::<P::Scalar>::SECONDARY_SURFACE,
        ),
    )
    .set_item_text_color(SimplePalette::<P::Scalar>::FOREGROUND.when(
      State::Disabled,
      SimplePalette::<P::Scalar>::MUTED_FOREGROUND,
    ))
    .set_shortcut_color(SimplePalette::<P::Scalar>::MUTED_FOREGROUND)
    .set_separator_color(SimplePalette::<P::Scalar>::BORDER)
    .set_shortcut_spacing(shape.shortcut_gap)
    .set_item_cursor(MouseCursor::Click)
    .set_item_corner_radius(MARK_RADIUS)
}

impl<P: Platform> View<P> for SimpleContextMenu<P, AnyView<P>>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    dress(self.raw).build()
  }
}
