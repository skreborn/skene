//! The shared box every container view wraps, in cells.

use std::rc::Rc;

use skene_core::{
  Arrangement,
  Color,
  Dimension,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTrack,
  ItemAlignment,
  Layout,
  Line,
  MouseCursor,
  NodeId,
  Overflow,
  Source,
  Spacing,
  View,
  Wrap,
};

use crate::{
  Children,
  Inset,
  events::ScrollEvent,
  props::{BoxProp, BoxStyle, Condition},
  realize::{Binders, split_prop},
  state::NodeKind,
  style::box_taffy_style,
  with_state,
};

/// A box that decorates itself and flows its children in document order.
///
/// The shared box surface: the typed containers—[`Row`](crate::Row),
/// [`Column`](crate::Column), [`Grid`](crate::Grid), [`Stack`](crate::Stack)—each wrap
/// one and layer their layout over it, and a container view of your own does the same.
/// Children are set with `set_children` or `derive_children`; decoration and sizing with
/// the chainable `set_`/`derive_` methods, in cells.
#[must_use]
pub struct StyledBox {
  pub(crate) children: Children,
  pub(crate) layout: Layout,
  pub(crate) width: Option<Source<Dimension<u32>>>,
  pub(crate) height: Option<Source<Dimension<u32>>>,
  pub(crate) min_width: Option<Source<Dimension<u32>>>,
  pub(crate) min_height: Option<Source<Dimension<u32>>>,
  pub(crate) max_width: Option<Source<Dimension<u32>>>,
  pub(crate) max_height: Option<Source<Dimension<u32>>>,
  pub(crate) aspect_ratio: Option<Source<f32>>,
  pub(crate) margin: Option<Source<skene_core::Inset<Dimension<u32>>>>,
  pub(crate) padding: Option<Source<Inset>>,
  pub(crate) offset: Option<Source<skene_core::Inset<Dimension<u32>>>>,
  pub(crate) pinned: Option<Source<skene_core::Inset<Dimension<u32>>>>,
  pub(crate) color: Option<Source<Color>>,
  pub(crate) border: Option<Source<Color>>,
  pub(crate) border_glyphs: Option<Source<crate::BorderGlyphs>>,
  pub(crate) spacing: Option<Source<Spacing<u32>>>,
  pub(crate) cursor: Option<Source<MouseCursor>>,
  pub(crate) overflow: Option<Source<Overflow>>,
  pub(crate) shadow: Option<Source<Color>>,
  pub(crate) wrap: Option<Source<Wrap>>,
  pub(crate) flex: Option<Source<Flexibility<u32>>>,
  pub(crate) arrangement: Option<Source<Arrangement>>,
  pub(crate) item_alignment: Option<Source<ItemAlignment>>,
  pub(crate) line_arrangement: Option<Source<Arrangement>>,
  pub(crate) item_horizontal_alignment: Option<Source<ItemAlignment>>,
  pub(crate) self_alignment: Option<Source<ItemAlignment>>,
  pub(crate) self_horizontal_alignment: Option<Source<ItemAlignment>>,
  pub(crate) grid_columns: Option<Source<Vec<GridTrack<u32>>>>,
  pub(crate) grid_rows: Option<Source<Vec<GridTrack<u32>>>>,
  pub(crate) grid_implicit_columns: Option<Source<Vec<GridTrack<u32>>>>,
  pub(crate) grid_implicit_rows: Option<Source<Vec<GridTrack<u32>>>>,
  pub(crate) grid_flow: Option<Source<GridFlow>>,
  pub(crate) grid_row_placement: Option<Source<Line<GridPlacement>>>,
  pub(crate) grid_column_placement: Option<Source<Line<GridPlacement>>>,
  pub(crate) variants: Vec<(Condition, BoxStyle)>,
  pub(crate) click_handler: Option<Rc<skene_core::ClickHandler>>,
  pub(crate) pointer_enter_handler: Option<Rc<dyn Fn()>>,
  pub(crate) pointer_leave_handler: Option<Rc<dyn Fn()>>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
  pub(crate) key_down_handler: Option<Rc<skene_core::KeyHandler>>,
  pub(crate) pointer_down_handler: Option<Rc<crate::events::PointerHandler>>,
  pub(crate) pointer_up_handler: Option<Rc<crate::events::PointerHandler>>,
  pub(crate) pointer_move_handler: Option<Rc<crate::events::PointerHandler>>,
  #[expect(clippy::type_complexity)]
  pub(crate) scroll_handler: Option<Rc<dyn Fn(&ScrollEvent)>>,
  pub(crate) anchor: Option<crate::Anchor>,
}

impl StyledBox {
  #[inline(always)]
  pub(crate) fn mode(layout: Layout) -> Self {
    Self {
      children: Children::default(),
      layout,
      width: None,
      height: None,
      min_width: None,
      min_height: None,
      max_width: None,
      max_height: None,
      aspect_ratio: None,
      margin: None,
      padding: None,
      offset: None,
      pinned: None,
      color: None,
      border: None,
      border_glyphs: None,
      spacing: None,
      cursor: None,
      overflow: None,
      shadow: None,
      wrap: None,
      flex: None,
      arrangement: None,
      item_alignment: None,
      line_arrangement: None,
      item_horizontal_alignment: None,
      self_alignment: None,
      self_horizontal_alignment: None,
      grid_columns: None,
      grid_rows: None,
      grid_implicit_columns: None,
      grid_implicit_rows: None,
      grid_flow: None,
      grid_row_placement: None,
      grid_column_placement: None,
      variants: Vec::new(),
      click_handler: None,
      pointer_enter_handler: None,
      pointer_leave_handler: None,
      focus_handler: None,
      blur_handler: None,
      key_down_handler: None,
      pointer_down_handler: None,
      pointer_up_handler: None,
      pointer_move_handler: None,
      scroll_handler: None,
      anchor: None,
    }
  }

  /// A block container: children flow in normal document order, contained within it.
  #[inline(always)]
  pub fn new() -> Self {
    StyledBox::mode(Layout::Flow)
  }
}

impl Default for StyledBox {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl crate::HasSize for StyledBox {
  #[inline(always)]
  fn size_box(&mut self) -> &mut StyledBox {
    self
  }
}

impl crate::HasDecoration for StyledBox {
  #[inline(always)]
  fn decoration_box(&mut self) -> &mut StyledBox {
    self
  }
}

impl crate::HasEvents for StyledBox {
  #[inline(always)]
  fn events_box(&mut self) -> &mut StyledBox {
    self
  }
}

impl skene_core::HasChildren<crate::TuiOps> for StyledBox {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for StyledBox {
  // One `split_prop` call per property; length is the property count, not complexity.
  #[expect(clippy::too_many_lines)]
  fn build(self) -> NodeId {
    let mut style = BoxStyle::new();
    let mut binders = Binders::new();

    split_prop(
      self.width,
      &mut style.width,
      &mut binders,
      BoxProp::Width,
      update_box,
    );

    split_prop(
      self.height,
      &mut style.height,
      &mut binders,
      BoxProp::Height,
      update_box,
    );

    split_prop(
      self.min_width,
      &mut style.min_width,
      &mut binders,
      BoxProp::MinWidth,
      update_box,
    );

    split_prop(
      self.min_height,
      &mut style.min_height,
      &mut binders,
      BoxProp::MinHeight,
      update_box,
    );

    split_prop(
      self.max_width,
      &mut style.max_width,
      &mut binders,
      BoxProp::MaxWidth,
      update_box,
    );

    split_prop(
      self.max_height,
      &mut style.max_height,
      &mut binders,
      BoxProp::MaxHeight,
      update_box,
    );

    split_prop(
      self.aspect_ratio,
      &mut style.aspect_ratio,
      &mut binders,
      BoxProp::AspectRatio,
      update_box,
    );

    split_prop(
      self.margin,
      &mut style.margin,
      &mut binders,
      BoxProp::Margin,
      update_box,
    );

    split_prop(
      self.padding,
      &mut style.padding,
      &mut binders,
      BoxProp::Padding,
      update_box,
    );

    split_prop(
      self.offset,
      &mut style.offset,
      &mut binders,
      BoxProp::Offset,
      update_box,
    );

    split_prop(
      self.pinned,
      &mut style.pinned,
      &mut binders,
      BoxProp::Pinned,
      update_box,
    );

    split_prop(
      self.color,
      &mut style.color,
      &mut binders,
      BoxProp::Color,
      update_box,
    );

    split_prop(
      self.border,
      &mut style.border,
      &mut binders,
      BoxProp::Border,
      update_box,
    );

    split_prop(
      self.border_glyphs,
      &mut style.border_glyphs,
      &mut binders,
      BoxProp::BorderGlyphs,
      update_box,
    );

    split_prop(
      self.spacing,
      &mut style.spacing,
      &mut binders,
      BoxProp::Spacing,
      update_box,
    );

    split_prop(
      self.cursor,
      &mut style.cursor,
      &mut binders,
      BoxProp::Cursor,
      update_box,
    );

    split_prop(
      self.overflow,
      &mut style.overflow,
      &mut binders,
      BoxProp::Overflow,
      update_box,
    );

    split_prop(
      self.shadow,
      &mut style.shadow,
      &mut binders,
      BoxProp::Shadow,
      update_box,
    );

    split_prop(
      self.wrap,
      &mut style.wrap,
      &mut binders,
      BoxProp::Wrap,
      update_box,
    );

    split_prop(
      self.flex,
      &mut style.flex,
      &mut binders,
      BoxProp::Flex,
      update_box,
    );

    split_prop(
      self.arrangement,
      &mut style.arrangement,
      &mut binders,
      BoxProp::Arrangement,
      update_box,
    );

    split_prop(
      self.item_alignment,
      &mut style.item_alignment,
      &mut binders,
      BoxProp::ItemAlignment,
      update_box,
    );

    split_prop(
      self.line_arrangement,
      &mut style.line_arrangement,
      &mut binders,
      BoxProp::LineArrangement,
      update_box,
    );

    split_prop(
      self.item_horizontal_alignment,
      &mut style.item_horizontal_alignment,
      &mut binders,
      BoxProp::ItemHorizontalAlignment,
      update_box,
    );

    split_prop(
      self.self_alignment,
      &mut style.self_alignment,
      &mut binders,
      BoxProp::SelfAlignment,
      update_box,
    );

    split_prop(
      self.self_horizontal_alignment,
      &mut style.self_horizontal_alignment,
      &mut binders,
      BoxProp::SelfHorizontalAlignment,
      update_box,
    );

    split_prop(
      self.grid_columns,
      &mut style.grid_columns,
      &mut binders,
      BoxProp::GridColumns,
      update_box,
    );

    split_prop(
      self.grid_rows,
      &mut style.grid_rows,
      &mut binders,
      BoxProp::GridRows,
      update_box,
    );

    split_prop(
      self.grid_implicit_columns,
      &mut style.grid_implicit_columns,
      &mut binders,
      BoxProp::GridImplicitColumns,
      update_box,
    );

    split_prop(
      self.grid_implicit_rows,
      &mut style.grid_implicit_rows,
      &mut binders,
      BoxProp::GridImplicitRows,
      update_box,
    );

    split_prop(
      self.grid_flow,
      &mut style.grid_flow,
      &mut binders,
      BoxProp::GridFlow,
      update_box,
    );

    split_prop(
      self.grid_row_placement,
      &mut style.grid_row_placement,
      &mut binders,
      BoxProp::GridRowPlacement,
      update_box,
    );

    split_prop(
      self.grid_column_placement,
      &mut style.grid_column_placement,
      &mut binders,
      BoxProp::GridColumnPlacement,
      update_box,
    );

    let taffy = box_taffy_style(self.layout, &style, false);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Box {
          layout: self.layout,
          style,
          variants: self.variants,
        },
        taffy,
      )
    });

    for binder in binders {
      binder(node);
    }

    with_state(|state| {
      if let Some(action) = self.click_handler {
        state
          .click
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_enter_handler {
        state
          .pointer_enter
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_leave_handler {
        state
          .pointer_leave
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.focus_handler {
        state
          .focus
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.blur_handler {
        state
          .blur
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.key_down_handler {
        state
          .key_down
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_down_handler {
        state
          .pointer_down
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_up_handler {
        state
          .pointer_up
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.pointer_move_handler {
        state
          .pointer_move
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = self.scroll_handler {
        state
          .scroll
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }
    });

    if let Some(anchor) = &self.anchor {
      anchor.assign(node);
    }

    match self.layout {
      // Overlap the children in one grid cell.
      Layout::Stack => self.children.mount_with(node, |child| {
        with_state(|state| state.with_node(child, |record| record.in_stack = true));

        child
      }),
      _ => self.children.mount(node),
    }

    node
  }
}

/// Applies one changed box property.
pub(crate) fn update_box(node: NodeId, prop: BoxProp) {
  with_state(|state| {
    state.with_node(node, |record| {
      if let NodeKind::Box { style, .. } | NodeKind::TextField { style, .. } = &mut record.kind {
        crate::props::apply_box_prop(style, prop);
      }
      else {
        // A kind with no box style of its own—a run, a glyph control, a group—takes
        // wrapper annotations on its record.
        crate::props::apply_box_prop(record.annotations.get_or_insert_default(), prop);
      }
    });

    state.dirty.set(true);
  });
}
