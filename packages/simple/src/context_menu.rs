//! A ready-made context menu in the Simple design language.

use skene_foundation::{
  AnyKind,
  Color,
  ContextMenu,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  MenuEntry,
  MouseCursor,
  NodeId,
  Realization,
  Spacing,
  State,
  View,
  When,
  Wraps,
};

use super::{
  SimplePalette,
  shape::{SimpleUnit, define_defaults, hairline, shape},
};

/// A ready-made context menu.
///
/// A card-colored list with a hairline border, its items padded like a picker's rows
/// and lit in the secondary surface under the pointer or focus, a disabled item in the
/// muted foreground, and a separator in the border color.
///
/// [`set_items`](Self::set_items) lists the items; the view served comes last through
/// [`set_child`](Self::set_child), so any view takes a menu by wrapping, or
/// [`serve`](Self::serve) attaches it to a node that already exists. For anything
/// beyond that, compose [`ContextMenu`] directly.
#[doc(hidden)]
#[derive(Debug)]
#[must_use]
pub struct SimpleContextMenu<P: Realization> {
  raw: ContextMenu<P>,
}

impl<P: Realization> SimpleContextMenu<P> {
  /// A menu with no items yet.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      raw: ContextMenu::new(),
    }
  }

  /// Sets the items, in order.
  pub fn set_items(mut self, items: impl IntoIterator<Item = MenuEntry>) -> Self {
    self.raw = self.raw.set_items(items);

    self
  }

  /// Sets the view the menu serves.
  pub fn set_child(mut self, child: impl View<P>) -> Self {
    self.raw = self.raw.set_child(child);

    self
  }

  /// Attaches the menu to `node`, a view that already exists; returns the list's node.
  /// A view [`set_child`](Self::set_child) supplied is not built.
  #[inline(always)]
  pub fn serve(self, node: NodeId) -> NodeId
  where
    P::Scalar: SimpleUnit,
    P::StyleExt: CornerRadius,
  {
    dress(self.raw).serve(node)
  }
}

impl<P: Realization> Default for SimpleContextMenu<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> AnyKind for SimpleContextMenu<P> {}

/// Decorates `menu` in the Simple language.
fn dress<P: Realization>(menu: ContextMenu<P>) -> ContextMenu<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  define_defaults::<P>();

  let shape = shape::<P>();

  menu
    .set_padding(shape.list_inset)
    .set_background(SimplePalette::<P::Scalar>::CARD)
    .set_border_width(hairline())
    .set_border_color(SimplePalette::<P::Scalar>::BORDER)
    .set_corner_radius(shape.control_radius)
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
    .set_shortcut_spacing(Spacing::all(shape.shortcut_spacing))
    .set_item_cursor(MouseCursor::Click)
    .set_item_corner_radius(shape.mark_radius)
}

impl<P: Realization> View<P> for SimpleContextMenu<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    dress(self.raw).build()
  }
}

/// What a platform's `SimpleContextMenu` carries.
pub trait SimpleContextMenuBase<P: Realization>: Wraps<Inner = SimpleContextMenu<P>> {
  /// A menu with no items yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(SimpleContextMenu::new())
  }

  /// Sets the items, in order.
  #[must_use]
  fn set_items(self, items: impl IntoIterator<Item = MenuEntry>) -> Self {
    Self::wrap(self.into_inner().set_items(items))
  }

  /// Sets the view the menu serves.
  #[must_use]
  fn set_child(self, child: impl View<P>) -> Self {
    Self::wrap(self.into_inner().set_child(child))
  }

  /// Attaches the menu to `node`, a view that already exists; returns the list's node.
  /// A view [`set_child`](Self::set_child) supplied is not built.
  fn serve(self, node: NodeId) -> NodeId
  where
    P::Scalar: SimpleUnit,
    P::StyleExt: CornerRadius,
  {
    self.into_inner().serve(node)
  }
}
