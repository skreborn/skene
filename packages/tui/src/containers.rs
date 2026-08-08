//! The typed containers and child wrappers.
//!
//! Each layout gets its own view—[`Row`], [`Column`], [`Grid`], [`Stack`]—so only the
//! properties that layout honors exist to call; the plain [`Container`] lays its
//! children out in normal flow. Per-child properties travel in wrapper views
//! ([`Flexible`], [`Pinned`], [`GridCell`]) accepted only by the matching container;
//! a wrapper creates no node of its own, it annotates its child's.

use std::rc::Rc;

use skene_core::{
  Alignment,
  AnyView,
  Arrangement,
  Dimension,
  FlexChild,
  FlexDirection,
  Flexibility,
  GridChild,
  GridFlow,
  GridPlacement,
  GridTrack,
  IntoChildOf,
  IntoView,
  ItemAlignment,
  Layout,
  Line,
  NodeId,
  Source,
  Spacing,
  StackChild,
  View,
  Wrap,
};

use crate::{Children, Container, container::update_box, props::BoxProp};

/// Generates the shared box surface—decoration, sizing, dynamics, events, children—
/// forwarding to the wrapped [`Container`].
macro_rules! box_surface {
  ($type:ident, $kind:ty) => {
    impl $type {
      /// Adds a conditional style variant, applied while `condition` holds.
      #[inline(always)]
      pub fn set_when(mut self, condition: crate::Condition, style: crate::BoxStyle) -> Self {
        self.base = self.base.set_when(condition, style);

        self
      }

      /// Binds `anchor` to the box, so an overlay positions against it once built.
      #[inline(always)]
      pub fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
        self.base = self.base.bind_anchor(anchor);

        self
      }

      box_surface!(@forward set_width, derive_width, impl Into<Dimension<u32>>, "the width");
      box_surface!(@forward set_height, derive_height, impl Into<Dimension<u32>>, "the height");
      box_surface!(@forward set_min_width, derive_min_width, impl Into<Dimension<u32>>, "the minimum width");
      box_surface!(@forward set_min_height, derive_min_height, impl Into<Dimension<u32>>, "the minimum height");
      box_surface!(@forward set_max_width, derive_max_width, impl Into<Dimension<u32>>, "the maximum width");
      box_surface!(@forward set_max_height, derive_max_height, impl Into<Dimension<u32>>, "the maximum height");
      box_surface!(@plain set_aspect_ratio, derive_aspect_ratio, f32, "the width-to-height ratio");
      box_surface!(@plain set_margin, derive_margin, skene_core::Inset<Dimension<u32>>, "the margin on each edge");
      box_surface!(@plain set_padding, derive_padding, crate::Inset, "the padding on each edge");
      box_surface!(@plain set_offset, derive_offset, skene_core::Inset<Dimension<u32>>, "the in-flow shift from each edge");
      box_surface!(@plain set_color, derive_color, skene_core::Color, "the background color");
      box_surface!(@plain set_border, derive_border, skene_core::Color, "a one-cell border in the given color");
      box_surface!(@plain set_border_glyphs, derive_border_glyphs, crate::BorderGlyphs, "the characters the border draws with");
      box_surface!(@plain set_shadow, derive_shadow, skene_core::Color, "the shadow color, cast one cell down and right");
      box_surface!(@plain set_cursor, derive_cursor, skene_core::MouseCursor, "the mouse cursor shown over the box");
      box_surface!(@plain set_overflow, derive_overflow, skene_core::Overflow, "how content exceeding the bounds is treated");

      box_surface!(@handler handle_click, skene_core::Prevent, "click");
      box_surface!(@handler0 handle_pointer_enter, "pointer-enter");
      box_surface!(@handler0 handle_pointer_leave, "pointer-leave");
      box_surface!(@handler0 handle_focus, "focus");
      box_surface!(@handler0 handle_blur, "blur");
    }

    impl $type {
      /// Sets the key-down handler.
      #[inline(always)]
      pub fn handle_key_down<R: Into<skene_core::Prevent>>(
        mut self,
        action: impl Fn(&skene_core::KeyEvent) -> R + 'static,
      ) -> Self {
        self.base = self.base.handle_key_down(action);

        self
      }

      /// Sets the pointer-down handler.
      #[inline(always)]
      pub fn handle_pointer_down<R: Into<skene_core::Prevent>>(
        mut self,
        action: impl Fn(&crate::PointerEvent) -> R + 'static,
      ) -> Self {
        self.base = self.base.handle_pointer_down(action);

        self
      }

      /// Sets the pointer-up handler.
      #[inline(always)]
      pub fn handle_pointer_up<R: Into<skene_core::Prevent>>(
        mut self,
        action: impl Fn(&crate::PointerEvent) -> R + 'static,
      ) -> Self {
        self.base = self.base.handle_pointer_up(action);

        self
      }

      /// Sets the pointer-move handler.
      #[inline(always)]
      pub fn handle_pointer_move<R: Into<skene_core::Prevent>>(
        mut self,
        action: impl Fn(&crate::PointerEvent) -> R + 'static,
      ) -> Self {
        self.base = self.base.handle_pointer_move(action);

        self
      }

      /// Sets the scroll handler.
      #[inline(always)]
      pub fn handle_scroll(mut self, action: impl Fn(&crate::ScrollEvent) + 'static) -> Self {
        self.base = self.base.handle_scroll(action);

        self
      }
    }

    impl skene_core::HasChildren<crate::TuiOps> for $type {
      type Kind = $kind;

      fn set_children_to(mut self, children: Children) -> Self {
        self.base.children = children;

        self
      }
    }
  };

  (@forward $set:ident, $derive:ident, impl Into<$type:ty>, $doc:literal) => {
    #[doc = concat!("Sets ", $doc, ".")]
    #[inline(always)]
    pub fn $set(mut self, value: impl Into<$type>) -> Self {
      self.base = self.base.$set(value);

      self
    }

    #[doc = concat!("Derives ", $doc, ".")]
    #[inline(always)]
    pub fn $derive<D: Into<$type>>(mut self, value: impl Fn() -> D + 'static) -> Self {
      self.base = self.base.$derive(value);

      self
    }
  };

  (@own $set:ident, $derive:ident, $field:ident, $type:ty, $doc:literal) => {
    #[doc = concat!("Sets ", $doc, ".")]
    #[inline(always)]
    pub fn $set(mut self, value: $type) -> Self {
      self.base.$field = Some(Source::Static(value));

      self
    }

    #[doc = concat!("Derives ", $doc, ".")]
    #[inline(always)]
    pub fn $derive(mut self, value: impl Fn() -> $type + 'static) -> Self {
      self.base.$field = Some(Source::Derived(Rc::new(value)));

      self
    }
  };

  (@plain $set:ident, $derive:ident, $type:ty, $doc:literal) => {
    #[doc = concat!("Sets ", $doc, ".")]
    #[inline(always)]
    pub fn $set(mut self, value: $type) -> Self {
      self.base = self.base.$set(value);

      self
    }

    #[doc = concat!("Derives ", $doc, ".")]
    #[inline(always)]
    pub fn $derive(mut self, value: impl Fn() -> $type + 'static) -> Self {
      self.base = self.base.$derive(value);

      self
    }
  };

  (@handler $name:ident, $verdict:ty, $doc:literal) => {
    #[doc = concat!("Sets the ", $doc, " handler.")]
    #[inline(always)]
    pub fn $name<R: Into<$verdict>>(mut self, action: impl Fn() -> R + 'static) -> Self {
      self.base = self.base.$name(action);

      self
    }
  };

  (@handler0 $name:ident, $doc:literal) => {
    #[doc = concat!("Sets the ", $doc, " handler.")]
    #[inline(always)]
    pub fn $name(mut self, action: impl Fn() + 'static) -> Self {
      self.base = self.base.$name(action);

      self
    }
  };
}

/// Merges independently-sourced spacing axes into the base's one spacing source.
fn merge_spacing(
  row: Option<Source<u32>>,
  column: Option<Source<u32>>,
) -> Option<Source<Spacing<u32>>> {
  match (row, column) {
    (None, None) => None,
    (row, column) => {
      let fixed = |source: &Option<Source<u32>>| !source.as_ref().is_some_and(Source::is_derived);

      if fixed(&row) && fixed(&column) {
        Some(Source::Static(Spacing {
          row: row.map_or(0, |source| source.get()),
          column: column.map_or(0, |source| source.get()),
        }))
      }
      else {
        Some(Source::Derived(Rc::new(move || Spacing {
          row: row.as_ref().map_or(0, Source::get),
          column: column.as_ref().map_or(0, Source::get),
        })))
      }
    }
  }
}

/// Splits a two-axis alignment source into the base's per-axis item slots.
fn split_alignment(
  alignment: Option<Source<Alignment>>,
) -> (Option<Source<ItemAlignment>>, Option<Source<ItemAlignment>>) {
  match alignment {
    None => (None, None),
    Some(Source::Static(alignment)) => (
      Some(Source::Static(alignment.horizontal)),
      Some(Source::Static(alignment.vertical)),
    ),
    Some(Source::Derived(derive)) => {
      let horizontal = Rc::clone(&derive);

      (
        Some(Source::Derived(Rc::new(move || horizontal().horizontal))),
        Some(Source::Derived(Rc::new(move || derive().vertical))),
      )
    }
  }
}

/// A flex container laying children out along one axis.
///
/// [`Row`] runs horizontally, [`Column`] vertically; wrap a child in [`Flexible`] to
/// let it take or give up space.
#[must_use]
pub struct Row {
  base: Container,
  spacing: Option<Source<u32>>,
  line_spacing: Option<Source<u32>>,
}

/// A flex container laying children out top to bottom; see [`Row`].
#[must_use]
pub struct Column {
  base: Container,
  spacing: Option<Source<u32>>,
  line_spacing: Option<Source<u32>>,
}

macro_rules! flex_surface {
  ($type:ident, $direction:expr, $reversed:expr) => {
    impl $type {
      /// A container laying children out along this axis.
      #[inline(always)]
      pub fn new() -> Self {
        Self {
          base: Container::mode(Layout::Flex {
            direction: $direction,
          }),
          spacing: None,
          line_spacing: None,
        }
      }

      /// A container laying children out along this axis, reversed.
      #[inline(always)]
      pub fn reversed() -> Self {
        Self {
          base: Container::mode(Layout::Flex {
            direction: $reversed,
          }),
          spacing: None,
          line_spacing: None,
        }
      }

      /// Sets a static spacing between children, along the main axis.
      #[inline(always)]
      pub fn set_spacing(mut self, spacing: u32) -> Self {
        self.spacing = Some(Source::Static(spacing));

        self
      }

      /// Sets a derived spacing between children.
      #[inline(always)]
      pub fn derive_spacing(mut self, spacing: impl Fn() -> u32 + 'static) -> Self {
        self.spacing = Some(Source::Derived(Rc::new(spacing)));

        self
      }

      /// Sets a static spacing between wrapped lines.
      #[inline(always)]
      pub fn set_line_spacing(mut self, line_spacing: u32) -> Self {
        self.line_spacing = Some(Source::Static(line_spacing));

        self
      }

      /// Sets a derived spacing between wrapped lines.
      #[inline(always)]
      pub fn derive_line_spacing(mut self, line_spacing: impl Fn() -> u32 + 'static) -> Self {
        self.line_spacing = Some(Source::Derived(Rc::new(line_spacing)));

        self
      }

      box_surface!(@own set_arrangement, derive_arrangement, arrangement, Arrangement, "the distribution of children along the main axis");
      box_surface!(@own set_item_alignment, derive_item_alignment, item_alignment, ItemAlignment, "the alignment of each child on the cross axis");
      box_surface!(@own set_line_arrangement, derive_line_arrangement, line_arrangement, Arrangement, "the distribution of wrapped lines along the cross axis");
      box_surface!(@own set_wrap, derive_wrap, wrap, Wrap, "whether children wrap onto multiple lines");
    }

    impl Default for $type {
      fn default() -> Self {
        Self::new()
      }
    }

    impl View for $type {
      fn build(self) -> NodeId {
        let mut base = self.base;
        let (row, column) = match $direction {
          FlexDirection::Row | FlexDirection::RowReverse => (self.line_spacing, self.spacing),
          FlexDirection::Column | FlexDirection::ColumnReverse => (self.spacing, self.line_spacing),
        };

        base.spacing = merge_spacing(row, column);

        base.build()
      }
    }
  };
}

flex_surface!(Row, FlexDirection::Row, FlexDirection::RowReverse);
flex_surface!(Column, FlexDirection::Column, FlexDirection::ColumnReverse);
box_surface!(Row, FlexChild);
box_surface!(Column, FlexChild);

/// A grid container placing children into tracks.
#[must_use]
pub struct Grid {
  base: Container,
  spacing: Option<Source<Spacing<u32>>>,
  item_alignment: Option<Source<Alignment>>,
}

impl Grid {
  /// A grid with no explicit tracks; set `columns` (and friends) to shape it.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: Container::mode(Layout::Grid),
      spacing: None,
      item_alignment: None,
    }
  }

  /// A grid with `count` equal-width columns.
  #[inline(always)]
  pub fn uniform(count: u16) -> Self {
    Self::new().set_columns(GridTrack::Flex(1.).repeated(count))
  }

  /// Sets the explicit column tracks.
  #[inline(always)]
  pub fn set_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.base.grid_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Derives the explicit column tracks.
  #[inline(always)]
  pub fn derive_columns(mut self, tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static) -> Self {
    self.base.grid_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets the explicit row tracks.
  #[inline(always)]
  pub fn set_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.base.grid_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Derives the explicit row tracks.
  #[inline(always)]
  pub fn derive_rows(mut self, tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static) -> Self {
    self.base.grid_rows = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets the sizes given to implicitly created columns, cycled.
  #[inline(always)]
  pub fn set_implicit_columns(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.base.grid_implicit_columns = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Derives the sizes given to implicitly created columns, cycled.
  #[inline(always)]
  pub fn derive_implicit_columns(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.base.grid_implicit_columns = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets the sizes given to implicitly created rows, cycled.
  #[inline(always)]
  pub fn set_implicit_rows(mut self, tracks: impl IntoIterator<Item = GridTrack<u32>>) -> Self {
    self.base.grid_implicit_rows = Some(Source::Static(tracks.into_iter().collect()));

    self
  }

  /// Derives the sizes given to implicitly created rows, cycled.
  #[inline(always)]
  pub fn derive_implicit_rows(
    mut self,
    tracks: impl Fn() -> Vec<GridTrack<u32>> + 'static,
  ) -> Self {
    self.base.grid_implicit_rows = Some(Source::Derived(Rc::new(tracks)));

    self
  }

  /// Sets a static spacing between tracks: one value for both axes, or a [`Spacing`]
  /// per axis.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: impl Into<Spacing<u32>>) -> Self {
    self.spacing = Some(Source::Static(spacing.into()));

    self
  }

  /// Sets a derived spacing between tracks.
  #[inline(always)]
  pub fn derive_spacing<S: Into<Spacing<u32>>>(
    mut self,
    spacing: impl Fn() -> S + 'static,
  ) -> Self {
    self.spacing = Some(Source::Derived(Rc::new(move || spacing().into())));

    self
  }

  /// Sets a static alignment of each child within its cell, on both axes.
  #[inline(always)]
  pub fn set_item_alignment(mut self, alignment: Alignment) -> Self {
    self.item_alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived alignment of each child within its cell.
  #[inline(always)]
  pub fn derive_item_alignment(mut self, alignment: impl Fn() -> Alignment + 'static) -> Self {
    self.item_alignment = Some(Source::Derived(Rc::new(alignment)));

    self
  }

  box_surface!(@own set_flow, derive_flow, grid_flow, GridFlow, "the direction auto-placed children fill");
  box_surface!(@own set_column_arrangement, derive_column_arrangement, arrangement, Arrangement, "the horizontal distribution of the column tracks");
  box_surface!(@own set_row_arrangement, derive_row_arrangement, line_arrangement, Arrangement, "the vertical distribution of the row tracks");
}

impl Default for Grid {
  fn default() -> Self {
    Self::new()
  }
}

impl View for Grid {
  fn build(self) -> NodeId {
    let mut base = self.base;
    let (horizontal, vertical) = split_alignment(self.item_alignment);

    base.spacing = self.spacing;
    base.item_horizontal_alignment = horizontal;
    base.item_alignment = vertical;

    base.build()
  }
}

box_surface!(Grid, GridChild);

/// A container overlapping its children in one cell, stacked in order (last on top).
///
/// Wrap a child in [`Pinned`] to place it against the stack's edges instead.
#[must_use]
pub struct Stack {
  base: Container,
  alignment: Option<Source<Alignment>>,
}

impl Stack {
  /// A stack of overlapping children.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: Container::mode(Layout::Stack),
      alignment: None,
    }
  }

  /// Sets a static alignment of the unpinned children within the stack.
  #[inline(always)]
  pub fn set_alignment(mut self, alignment: Alignment) -> Self {
    self.alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived alignment of the unpinned children.
  #[inline(always)]
  pub fn derive_alignment(mut self, alignment: impl Fn() -> Alignment + 'static) -> Self {
    self.alignment = Some(Source::Derived(Rc::new(alignment)));

    self
  }
}

impl Default for Stack {
  fn default() -> Self {
    Self::new()
  }
}

impl View for Stack {
  fn build(self) -> NodeId {
    let mut base = self.base;
    let (horizontal, vertical) = split_alignment(self.alignment);

    base.item_horizontal_alignment = horizontal;
    base.item_alignment = vertical;

    base.build()
  }
}

box_surface!(Stack, StackChild);

/// Reads a source through a shared handle, for merged reactive child properties.
fn reader<T: Clone + 'static>(source: Option<&Source<T>>, fallback: T) -> impl Fn() -> T + 'static {
  let source = source.cloned();

  move || {
    source
      .as_ref()
      .map_or_else(|| fallback.clone(), Source::get)
  }
}

/// Applies one merged child property: once when static, through an effect when any
/// component derives.
fn apply_child_prop(node: NodeId, derived: bool, read: impl Fn() -> BoxProp + 'static) {
  if derived {
    skene_reactive::Effect::new(move |_| update_box(node, read()));
  }
  else {
    update_box(node, read());
  }
}

/// Wraps a child of a [`Row`] or [`Column`], letting it take free space and give up
/// overflow.
///
/// Defaults to a grow factor of `1.`: the child takes an equal share of the free
/// space. The wrapper creates no node; its properties land on the child's own.
#[must_use]
pub struct Flexible {
  child: AnyView,
  grow_factor: Option<Source<f32>>,
  shrink_factor: Option<Source<f32>>,
  initial: Option<Source<Dimension<u32>>>,
  alignment: Option<Source<ItemAlignment>>,
}

impl Flexible {
  /// Wraps `child` with a grow factor of `1.`.
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      grow_factor: None,
      shrink_factor: None,
      initial: None,
      alignment: None,
    }
  }

  /// Sets a static weight for taking free space.
  #[inline(always)]
  pub fn set_grow_factor(mut self, grow_factor: f32) -> Self {
    self.grow_factor = Some(Source::Static(grow_factor));

    self
  }

  /// Sets a derived weight for taking free space.
  #[inline(always)]
  pub fn derive_grow_factor(mut self, grow_factor: impl Fn() -> f32 + 'static) -> Self {
    self.grow_factor = Some(Source::Derived(Rc::new(grow_factor)));

    self
  }

  /// Sets a static weight for giving up space on overflow.
  #[inline(always)]
  pub fn set_shrink_factor(mut self, shrink_factor: f32) -> Self {
    self.shrink_factor = Some(Source::Static(shrink_factor));

    self
  }

  /// Sets a derived weight for giving up space on overflow.
  #[inline(always)]
  pub fn derive_shrink_factor(mut self, shrink_factor: impl Fn() -> f32 + 'static) -> Self {
    self.shrink_factor = Some(Source::Derived(Rc::new(shrink_factor)));

    self
  }

  /// Sets a static main-axis size flexing starts from.
  #[inline(always)]
  pub fn set_initial(mut self, initial: impl Into<Dimension<u32>>) -> Self {
    self.initial = Some(Source::Static(initial.into()));

    self
  }

  /// Sets a derived main-axis size flexing starts from.
  #[inline(always)]
  pub fn derive_initial<D: Into<Dimension<u32>>>(
    mut self,
    initial: impl Fn() -> D + 'static,
  ) -> Self {
    self.initial = Some(Source::Derived(Rc::new(move || initial().into())));

    self
  }

  /// Sets a static cross-axis alignment for this child alone.
  #[inline(always)]
  pub fn set_item_alignment(mut self, alignment: ItemAlignment) -> Self {
    self.alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived cross-axis alignment for this child alone.
  #[inline(always)]
  pub fn derive_item_alignment(mut self, alignment: impl Fn() -> ItemAlignment + 'static) -> Self {
    self.alignment = Some(Source::Derived(Rc::new(alignment)));

    self
  }
}

impl IntoChildOf<FlexChild> for Flexible {
  fn into_child(self) -> AnyView {
    AnyView::new(move || {
      let node = self.child.build();
      let derived = [&self.grow_factor, &self.shrink_factor]
        .iter()
        .any(|source| source.as_ref().is_some_and(Source::is_derived))
        || self.initial.as_ref().is_some_and(Source::is_derived);
      let grow_factor = reader(self.grow_factor.as_ref(), 1.);
      let shrink_factor = reader(self.shrink_factor.as_ref(), 1.);
      let initial = reader(self.initial.as_ref(), Dimension::Auto);

      apply_child_prop(node, derived, move || {
        BoxProp::Flex(Flexibility::new(initial(), grow_factor(), shrink_factor()))
      });

      if let Some(alignment) = self.alignment {
        alignment.bind(move |alignment| {
          update_box(node, BoxProp::SelfAlignment(alignment));
        });
      }

      node
    })
  }
}

/// Wraps a child of a [`Stack`], pinning it out of flow against the stack's edges.
///
/// Unset edges default to auto: the child sits where the stack's alignment puts it on
/// that axis. The wrapper creates no node; its properties land on the child's own.
#[must_use]
pub struct Pinned {
  child: AnyView,
  edges: Option<Source<skene_core::Inset<Dimension<u32>>>>,
}

impl Pinned {
  /// Wraps `child`, pinned but with every edge auto until [`set_edges`](Self::set_edges).
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      edges: None,
    }
  }

  /// Sets the static edges the child is pinned to.
  #[inline(always)]
  pub fn set_edges(mut self, edges: skene_core::Inset<Dimension<u32>>) -> Self {
    self.edges = Some(Source::Static(edges));

    self
  }

  /// Derives the edges the child is pinned to.
  #[inline(always)]
  pub fn derive_edges(
    mut self,
    edges: impl Fn() -> skene_core::Inset<Dimension<u32>> + 'static,
  ) -> Self {
    self.edges = Some(Source::Derived(Rc::new(edges)));

    self
  }
}

impl IntoChildOf<StackChild> for Pinned {
  fn into_child(self) -> AnyView {
    AnyView::new(move || {
      let node = self.child.build();
      let edges = self.edges.unwrap_or(Source::Static(skene_core::Inset {
        top: Dimension::Auto,
        right: Dimension::Auto,
        bottom: Dimension::Auto,
        left: Dimension::Auto,
      }));

      edges.bind(move |edges| update_box(node, BoxProp::Pinned(edges)));

      node
    })
  }
}

/// Wraps a child of a [`Grid`], placing it in the grid's tracks.
///
/// The wrapper creates no node; its properties land on the child's own.
#[must_use]
pub struct GridCell {
  child: AnyView,
  row: Option<Source<Line<GridPlacement>>>,
  column: Option<Source<Line<GridPlacement>>>,
  alignment: Option<Source<Alignment>>,
}

impl GridCell {
  /// Wraps `child`, auto-placed until a row or column placement is set.
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      row: None,
      column: None,
      alignment: None,
    }
  }

  /// Sets the static row placement.
  #[inline(always)]
  pub fn set_row(mut self, row: Line<GridPlacement>) -> Self {
    self.row = Some(Source::Static(row));

    self
  }

  /// Derives the row placement.
  #[inline(always)]
  pub fn derive_row(mut self, row: impl Fn() -> Line<GridPlacement> + 'static) -> Self {
    self.row = Some(Source::Derived(Rc::new(row)));

    self
  }

  /// Sets the static column placement.
  #[inline(always)]
  pub fn set_column(mut self, column: Line<GridPlacement>) -> Self {
    self.column = Some(Source::Static(column));

    self
  }

  /// Derives the column placement.
  #[inline(always)]
  pub fn derive_column(mut self, column: impl Fn() -> Line<GridPlacement> + 'static) -> Self {
    self.column = Some(Source::Derived(Rc::new(column)));

    self
  }

  /// Sets a static alignment within the cell for this child alone, on both axes.
  #[inline(always)]
  pub fn set_alignment(mut self, alignment: Alignment) -> Self {
    self.alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived alignment within the cell for this child alone.
  #[inline(always)]
  pub fn derive_alignment(mut self, alignment: impl Fn() -> Alignment + 'static) -> Self {
    self.alignment = Some(Source::Derived(Rc::new(alignment)));

    self
  }
}

impl IntoChildOf<GridChild> for GridCell {
  fn into_child(self) -> AnyView {
    AnyView::new(move || {
      let node = self.child.build();

      if let Some(row) = self.row {
        row.bind(move |row| update_box(node, BoxProp::GridRowPlacement(row)));
      }

      if let Some(column) = self.column {
        column.bind(move |column| {
          update_box(node, BoxProp::GridColumnPlacement(column));
        });
      }

      if let Some(alignment) = self.alignment {
        alignment.bind(move |alignment| {
          update_box(node, BoxProp::SelfHorizontalAlignment(alignment.horizontal));
          update_box(node, BoxProp::SelfAlignment(alignment.vertical));
        });
      }

      node
    })
  }
}
