//! A ready-made context menu in the Simple design language.

use skene_core::{
  AnyKind,
  AnyView,
  Color,
  Driver,
  IntoView,
  MouseCursor,
  NoChild,
  NodeId,
  State,
  View,
  When,
};
use skene_views::{ContextMenu, CornerRadius, MenuItem};

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
pub struct SimpleContextMenu<D: Driver, C = NoChild> {
  raw: ContextMenu<D, C>,
}

impl<D: Driver> SimpleContextMenu<D> {
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
  pub fn set_child(self, child: impl IntoView<D>) -> SimpleContextMenu<D, AnyView<D>> {
    SimpleContextMenu {
      raw: self.raw.set_child(child),
    }
  }

  /// Attaches the menu to `node`, a view that already exists; returns the list's node.
  #[inline(always)]
  pub fn serve(self, node: NodeId) -> NodeId
  where
    D::Scalar: SimpleUnit,
    D::StyleExt: CornerRadius,
  {
    dress(self.raw).serve(node)
  }
}

impl<D: Driver> Default for SimpleContextMenu<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver, C> AnyKind for SimpleContextMenu<D, C> {}

/// Decorates `menu` in the Simple language.
fn dress<D: Driver, C>(menu: ContextMenu<D, C>) -> ContextMenu<D, C>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  define_defaults::<D>();

  let shape = shape::<D>();

  menu
    .set_padding(shape.list_inset)
    .set_color(SimplePalette::<D::Scalar>::CARD)
    .set_border(hairline(SimplePalette::<D::Scalar>::BORDER))
    .set_corner_radius(RADIUS)
    .set_item_padding(shape.row_inset)
    .set_item_color(
      Color::TRANSPARENT
        .when(
          State::Hovered,
          SimplePalette::<D::Scalar>::SECONDARY_SURFACE,
        )
        .when(
          State::Focused,
          SimplePalette::<D::Scalar>::SECONDARY_SURFACE,
        ),
    )
    .set_item_text_color(SimplePalette::<D::Scalar>::FOREGROUND.when(
      State::Disabled,
      SimplePalette::<D::Scalar>::MUTED_FOREGROUND,
    ))
    .set_shortcut_color(SimplePalette::<D::Scalar>::MUTED_FOREGROUND)
    .set_separator_color(SimplePalette::<D::Scalar>::BORDER)
    .set_shortcut_spacing(shape.shortcut_gap)
    .set_item_cursor(MouseCursor::Click)
    .set_item_corner_radius(MARK_RADIUS)
}

impl<D: Driver> View<D> for SimpleContextMenu<D, AnyView<D>>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    dress(self.raw).build()
  }
}
