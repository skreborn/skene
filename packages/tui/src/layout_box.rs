//! The public layout node: children arranged by a chosen mode, in cells.

use std::{marker::PhantomData, rc::Rc};

use skene_core::{
  Arrangement,
  GridFlow,
  GridTrack,
  HasChildren,
  ItemAlignment,
  Layout,
  NodeId,
  Source,
  Spacing,
  View,
  Wrap,
};

use crate::{Children, TuiOps, box_node::BoxNode, props::BoxStyle};

/// A box that arranges its children, carrying no decoration of its own.
///
/// The substrate the built-in layouts are made of, and the one a layout view of your
/// own is made of: pick a [`Layout`] mode, set the properties that mode honors, and
/// wrap it in your view, exposing whichever of them your view means to offer. `K` is
/// the child kind it accepts, so per-child wrappers—[`Flexible`](crate::Flexible),
/// [`Pinned`](crate::Pinned), [`GridCell`](crate::GridCell)—reach only a matching
/// layout. Decoration and sizing come from a wrapping [`Container`](crate::Container).
#[must_use]
pub struct LayoutBox<K = skene_core::PlainChild> {
  pub(crate) node: BoxNode,
  kind: PhantomData<K>,
}

/// Generates a layout property's three setters: fixed, derived, and conditional.
macro_rules! layout_prop {
  ($set:ident, $derive:ident, $when:ident, $field:ident, $style:ident, $type:ty, $doc:literal) => {
    #[doc = concat!("Sets ", $doc, ".")]
    #[inline(always)]
    pub fn $set(mut self, value: $type) -> Self {
      self.node.$field = Some(Source::Static(value));

      self
    }

    #[doc = concat!("Derives ", $doc, ".")]
    #[inline(always)]
    pub fn $derive(mut self, value: impl Fn() -> $type + 'static) -> Self {
      self.node.$field = Some(Source::Derived(Rc::new(value)));

      self
    }

    #[doc = concat!("Sets ", $doc, ", applied while `condition` holds.")]
    #[inline(always)]
    pub fn $when(mut self, condition: crate::Condition, value: $type) -> Self {
      self
        .node
        .variants
        .push((condition, BoxStyle::new().$style(value)));

      self
    }
  };
}

impl<K> LayoutBox<K> {
  /// Creates a layout box arranging its children in `layout`.
  #[inline(always)]
  pub fn new(layout: Layout) -> Self {
    Self {
      node: BoxNode::mode(layout),
      kind: PhantomData,
    }
  }

  layout_prop!(
    set_padding,
    derive_padding,
    set_padding_when,
    padding,
    set_padding,
    crate::Inset,
    "the inset between the box and the children it arranges, in cells"
  );
  layout_prop!(
    set_spacing,
    derive_spacing,
    set_spacing_when,
    spacing,
    set_spacing,
    Spacing<u32>,
    "the spacing between children, per axis"
  );
  layout_prop!(
    set_arrangement,
    derive_arrangement,
    set_arrangement_when,
    arrangement,
    set_arrangement,
    Arrangement,
    "the distribution of children along the main axis"
  );
  layout_prop!(
    set_item_alignment,
    derive_item_alignment,
    set_item_alignment_when,
    item_alignment,
    set_item_alignment,
    ItemAlignment,
    "the alignment of each child on the cross axis"
  );
  layout_prop!(
    set_line_arrangement,
    derive_line_arrangement,
    set_line_arrangement_when,
    line_arrangement,
    set_line_arrangement,
    Arrangement,
    "the distribution of wrapped lines along the cross axis"
  );
  layout_prop!(
    set_wrap,
    derive_wrap,
    set_wrap_when,
    wrap,
    set_wrap,
    Wrap,
    "whether children wrap onto multiple lines"
  );
  layout_prop!(
    set_flow,
    derive_flow,
    set_flow_when,
    grid_flow,
    set_grid_flow,
    GridFlow,
    "the direction auto-placed children fill"
  );

  /// Sets the explicit column tracks.
  #[inline(always)]
  pub fn set_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.node.grid_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Sets the explicit row tracks.
  #[inline(always)]
  pub fn set_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.node.grid_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }
}

impl<K: 'static> HasChildren<TuiOps> for LayoutBox<K> {
  type Kind = K;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.node.children = children;

    self
  }
}

impl<K: 'static> View for LayoutBox<K> {
  #[inline(always)]
  fn build(self) -> NodeId {
    self.node.build()
  }
}
