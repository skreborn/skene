//! The shared box every container view wraps.

use std::rc::Rc;

use skene_core::{
  Animation,
  Arrangement,
  BoxStyle,
  Color,
  Condition,
  Dimension,
  Flexibility,
  GridFlow,
  GridPlacement,
  GridTrack,
  ItemAlignment,
  Line,
  MouseCursor,
  Overflow,
  ScrollEvent,
  Source,
  Spacing,
  Transition,
  Wrap,
};

use crate::{Border, Children, HasChildren, Inset, Layout, Role};

/// The realized box every view in the vocabulary is built from: the whole property
/// set, split across the public surfaces that write into it—[`Container`](crate::Container)
/// for decoration and sizing, the layout views and [`LayoutBox`](crate::LayoutBox) for
/// arrangement.
#[must_use]
pub(crate) struct BoxNode {
  pub(crate) children: Children,
  pub(crate) layout: Layout,
  pub(crate) role: Option<Role>,
  pub(crate) focus_order: Option<Source<skene_core::FocusOrder>>,
  pub(crate) width: Option<Source<Dimension<f32>>>,
  pub(crate) height: Option<Source<Dimension<f32>>>,
  pub(crate) min_width: Option<Source<Dimension<f32>>>,
  pub(crate) min_height: Option<Source<Dimension<f32>>>,
  pub(crate) max_width: Option<Source<Dimension<f32>>>,
  pub(crate) max_height: Option<Source<Dimension<f32>>>,
  pub(crate) aspect_ratio: Option<Source<f32>>,
  pub(crate) margin: Option<Source<skene_core::Inset<Dimension<f32>>>>,
  pub(crate) padding: Option<Source<Inset>>,
  pub(crate) offset: Option<Source<skene_core::Inset<Dimension<f32>>>>,
  pub(crate) pinned: Option<Source<skene_core::Inset<Dimension<f32>>>>,
  pub(crate) color: Option<Source<Color>>,
  pub(crate) border: Option<Source<Border>>,
  pub(crate) border_radius: Option<Source<f32>>,
  pub(crate) cursor: Option<Source<MouseCursor>>,
  pub(crate) spacing: Option<Source<Spacing<f32>>>,
  pub(crate) wrap: Option<Source<Wrap>>,
  pub(crate) flex: Option<Source<Flexibility<f32>>>,
  pub(crate) arrangement: Option<Source<Arrangement>>,
  pub(crate) item_alignment: Option<Source<ItemAlignment>>,
  pub(crate) line_arrangement: Option<Source<Arrangement>>,
  pub(crate) item_horizontal_alignment: Option<Source<ItemAlignment>>,
  pub(crate) self_alignment: Option<Source<ItemAlignment>>,
  pub(crate) self_horizontal_alignment: Option<Source<ItemAlignment>>,
  pub(crate) grid_columns: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_rows: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_implicit_columns: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_implicit_rows: Option<Source<Vec<GridTrack<f32>>>>,
  pub(crate) grid_flow: Option<Source<GridFlow>>,
  pub(crate) grid_row_placement: Option<Source<Line<GridPlacement>>>,
  pub(crate) grid_column_placement: Option<Source<Line<GridPlacement>>>,
  pub(crate) overflow: Option<Source<Overflow>>,
  pub(crate) accessibility: Option<Source<Vec<skene_core::AccessProp>>>,
  pub(crate) anchor: Option<crate::Anchor>,
  pub(crate) variants: Vec<(Condition, BoxStyle)>,
  pub(crate) transitions: Vec<Transition>,
  pub(crate) animations: Vec<Animation<BoxStyle>>,
  pub(crate) click_handler: Option<Rc<skene_core::ClickHandler>>,
  pub(crate) pointer_enter_handler: Option<Rc<dyn Fn()>>,
  pub(crate) pointer_leave_handler: Option<Rc<dyn Fn()>>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
  pub(crate) key_down_handler: Option<Rc<skene_core::KeyHandler>>,
  pub(crate) pointer_down_handler: Option<Rc<skene_core::PointerHandler>>,
  pub(crate) pointer_up_handler: Option<Rc<skene_core::PointerHandler>>,
  pub(crate) pointer_move_handler: Option<Rc<skene_core::PointerHandler>>,
  #[expect(clippy::type_complexity)]
  pub(crate) scroll_handler: Option<Rc<dyn Fn(&ScrollEvent)>>,
}

impl BoxNode {
  #[inline(always)]
  pub(crate) fn mode(layout: Layout) -> Self {
    Self {
      children: Children::default(),
      layout,
      role: None,
      focus_order: None,
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
      border_radius: None,
      cursor: None,
      spacing: None,
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
      overflow: None,
      accessibility: None,
      anchor: None,
      variants: Vec::new(),
      transitions: Vec::new(),
      animations: Vec::new(),
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
    }
  }

  /// A block container: children flow in normal document order, contained within it.
  #[inline(always)]
  pub fn new() -> Self {
    BoxNode::mode(Layout::Flow)
  }
}

impl Default for BoxNode {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl HasChildren<crate::GuiOps> for BoxNode {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl skene_core::View for BoxNode {
  // One `split_prop` call per property; length is the property count, not complexity.
  #[expect(clippy::too_many_lines)]
  fn build(self) -> skene_core::NodeId {
    use skene_core::{BoxProp, BoxProps};

    use crate::{
      driver,
      realize::{Binders, split_prop},
    };

    let mut style = BoxStyle::default();
    let mut binders = Binders::new();

    split_prop(
      self.width,
      &mut style.width,
      &mut binders,
      BoxProp::Width,
      driver::update_box,
    );

    split_prop(
      self.height,
      &mut style.height,
      &mut binders,
      BoxProp::Height,
      driver::update_box,
    );

    split_prop(
      self.min_width,
      &mut style.min_width,
      &mut binders,
      BoxProp::MinWidth,
      driver::update_box,
    );

    split_prop(
      self.min_height,
      &mut style.min_height,
      &mut binders,
      BoxProp::MinHeight,
      driver::update_box,
    );

    split_prop(
      self.max_width,
      &mut style.max_width,
      &mut binders,
      BoxProp::MaxWidth,
      driver::update_box,
    );

    split_prop(
      self.max_height,
      &mut style.max_height,
      &mut binders,
      BoxProp::MaxHeight,
      driver::update_box,
    );

    split_prop(
      self.aspect_ratio,
      &mut style.aspect_ratio,
      &mut binders,
      BoxProp::AspectRatio,
      driver::update_box,
    );

    split_prop(
      self.margin,
      &mut style.margin,
      &mut binders,
      BoxProp::Margin,
      driver::update_box,
    );

    split_prop(
      self.padding,
      &mut style.padding,
      &mut binders,
      BoxProp::Padding,
      driver::update_box,
    );

    split_prop(
      self.offset,
      &mut style.offset,
      &mut binders,
      BoxProp::Offset,
      driver::update_box,
    );

    split_prop(
      self.pinned,
      &mut style.pinned,
      &mut binders,
      BoxProp::Pinned,
      driver::update_box,
    );

    split_prop(
      self.color,
      &mut style.color,
      &mut binders,
      BoxProp::Color,
      driver::update_box,
    );

    split_prop(
      self.border,
      &mut style.border,
      &mut binders,
      BoxProp::Border,
      driver::update_box,
    );

    split_prop(
      self.border_radius,
      &mut style.border_radius,
      &mut binders,
      BoxProp::BorderRadius,
      driver::update_box,
    );

    split_prop(
      self.cursor,
      &mut style.cursor,
      &mut binders,
      BoxProp::Cursor,
      driver::update_box,
    );

    split_prop(
      self.spacing,
      &mut style.spacing,
      &mut binders,
      BoxProp::Spacing,
      driver::update_box,
    );

    split_prop(
      self.wrap,
      &mut style.wrap,
      &mut binders,
      BoxProp::Wrap,
      driver::update_box,
    );

    split_prop(
      self.flex,
      &mut style.flex,
      &mut binders,
      BoxProp::Flex,
      driver::update_box,
    );

    split_prop(
      self.arrangement,
      &mut style.arrangement,
      &mut binders,
      BoxProp::Arrangement,
      driver::update_box,
    );

    split_prop(
      self.item_alignment,
      &mut style.item_alignment,
      &mut binders,
      BoxProp::ItemAlignment,
      driver::update_box,
    );

    split_prop(
      self.line_arrangement,
      &mut style.line_arrangement,
      &mut binders,
      BoxProp::LineArrangement,
      driver::update_box,
    );

    split_prop(
      self.item_horizontal_alignment,
      &mut style.item_horizontal_alignment,
      &mut binders,
      BoxProp::ItemHorizontalAlignment,
      driver::update_box,
    );

    split_prop(
      self.self_alignment,
      &mut style.self_alignment,
      &mut binders,
      BoxProp::SelfAlignment,
      driver::update_box,
    );

    split_prop(
      self.self_horizontal_alignment,
      &mut style.self_horizontal_alignment,
      &mut binders,
      BoxProp::SelfHorizontalAlignment,
      driver::update_box,
    );

    split_prop(
      self.grid_columns,
      &mut style.grid_columns,
      &mut binders,
      BoxProp::GridColumns,
      driver::update_box,
    );

    split_prop(
      self.grid_rows,
      &mut style.grid_rows,
      &mut binders,
      BoxProp::GridRows,
      driver::update_box,
    );

    split_prop(
      self.grid_implicit_columns,
      &mut style.grid_implicit_columns,
      &mut binders,
      BoxProp::GridImplicitColumns,
      driver::update_box,
    );

    split_prop(
      self.grid_implicit_rows,
      &mut style.grid_implicit_rows,
      &mut binders,
      BoxProp::GridImplicitRows,
      driver::update_box,
    );

    split_prop(
      self.grid_flow,
      &mut style.grid_flow,
      &mut binders,
      BoxProp::GridFlow,
      driver::update_box,
    );

    split_prop(
      self.grid_row_placement,
      &mut style.grid_row_placement,
      &mut binders,
      BoxProp::GridRowPlacement,
      driver::update_box,
    );

    split_prop(
      self.grid_column_placement,
      &mut style.grid_column_placement,
      &mut binders,
      BoxProp::GridColumnPlacement,
      driver::update_box,
    );

    split_prop(
      self.overflow,
      &mut style.overflow,
      &mut binders,
      BoxProp::Overflow,
      driver::update_box,
    );

    let node = driver::create_box(&BoxProps {
      layout: self.layout,
      role: self.role,
      style,
      variants: self.variants,
      transitions: self.transitions,
      animations: self.animations,
    });

    for binder in binders {
      binder(node);
    }

    if let Some(action) = self.click_handler {
      driver::handle_click(node, action);
    }

    if let Some(action) = self.pointer_enter_handler {
      driver::handle_pointer_enter(node, action);
    }

    if let Some(action) = self.pointer_leave_handler {
      driver::handle_pointer_leave(node, action);
    }

    if let Some(action) = self.focus_handler {
      driver::handle_focus(node, action);
    }

    if let Some(action) = self.blur_handler {
      driver::handle_blur(node, action);
    }

    if let Some(action) = self.key_down_handler {
      driver::handle_key_down(node, action);
    }

    if let Some(action) = self.pointer_down_handler {
      driver::handle_pointer_down(node, action);
    }

    if let Some(action) = self.pointer_up_handler {
      driver::handle_pointer_up(node, action);
    }

    if let Some(action) = self.pointer_move_handler {
      driver::handle_pointer_move(node, action);
    }

    if let Some(action) = self.scroll_handler {
      driver::handle_scroll(node, action);
    }

    if let Some(focus_order) = self.focus_order {
      focus_order.bind(move |order| driver::update_focus_order(node, order));
    }

    if let Some(state) = self.accessibility {
      state.bind(move |state| {
        for prop in &state {
          driver::update_access(node, prop);
        }
      });
    }

    if let Some(anchor) = &self.anchor {
      anchor.assign(node);
    }

    match self.layout {
      // Overlap the children in one grid cell.
      Layout::Stack => self.children.mount_with(node, |child| {
        driver::place_in_stack(child);

        child
      }),
      _ => self.children.mount(node),
    }

    node
  }
}
