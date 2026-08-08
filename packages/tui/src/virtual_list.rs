//! A vertically scrolling list rendering only its visible slice, in cells.

use std::rc::Rc;

use skene_core::{HasChildren, IntoView, Layout, NodeId, Overflow, View, virtual_window};
use skene_reactive::{Get, Memo, Set, Signal};

use crate::{
  events::ScrollEvent,
  props::{BoxStyle, Condition},
  state::NodeKind,
  style::box_taffy_style,
  with_state,
};

/// A scrollable column of `count()` fixed-height items, only the visible slice (plus an
/// overscan margin) built at any moment; the rest is empty space.
///
/// Items are keyed by index: scrolling reuses the views whose indices stay in the
/// window, and an item expresses its own reactivity with `derive_` properties as usual.
#[must_use]
pub struct VirtualList {
  item_height: u32,
  count: Rc<dyn Fn() -> usize>,
  item: Rc<dyn Fn(usize) -> NodeId>,
  style: BoxStyle,
  variants: Vec<(Condition, BoxStyle)>,
  overscan: usize,
}

impl VirtualList {
  /// Creates a list of `count()` items, each `item_height` cells tall, built by `item`.
  pub fn new<V: IntoView>(
    item_height: u32,
    count: impl Fn() -> usize + 'static,
    item: impl Fn(usize) -> V + 'static,
  ) -> Self {
    Self {
      item_height,
      count: Rc::new(count),
      item: Rc::new(move |index| {
        crate::Container::new()
          .set_height(item_height)
          .set_child(item(index))
          .build()
      }),
      style: BoxStyle::new(),
      variants: Vec::new(),
      overscan: 3,
    }
  }

  /// Sets the scroll box's static style; its overflow defaults to `Auto`.
  #[inline(always)]
  pub fn set_style(mut self, style: BoxStyle) -> Self {
    self.style = style;

    self
  }

  /// Adds a conditional style variant, applied while `condition` holds.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.variants.push((condition, style));

    self
  }

  /// Sets how many extra items build beyond each edge of the visible slice.
  #[inline(always)]
  pub fn set_overscan(mut self, overscan: usize) -> Self {
    self.overscan = overscan;

    self
  }
}

impl View for VirtualList {
  fn build(self) -> NodeId {
    let mut style = self.style;

    style.overflow = Some(style.overflow.unwrap_or(Overflow::Auto));

    let offset = Signal::new(0u32);
    let viewport = Signal::new(0u32);
    let taffy = box_taffy_style(Layout::Flow, &style, false);
    let outer = with_state(|state| {
      let outer = state.insert_node(
        NodeKind::Box {
          layout: Layout::Flow,
          style,
          variants: self.variants,
        },
        taffy,
      );

      state
        .scroll
        .borrow_mut()
        .entry(outer)
        .or_default()
        .push(Rc::new(move |event: &ScrollEvent| offset.set(event.row)));

      state.watch_viewport(outer, Rc::new(move |_, height| viewport.set(height)));

      outer
    });

    let item_height = self.item_height;
    let count = self.count;
    let overscan = self.overscan;
    let window = Memo::new(move |_| {
      let count = count();

      #[expect(clippy::cast_precision_loss)]
      (
        virtual_window(
          offset.get() as f32,
          viewport.get() as f32,
          item_height as f32,
          count,
          overscan,
        ),
        count,
      )
    });

    let extent = move |items: usize| {
      u32::try_from(items.saturating_mul(item_height as usize)).unwrap_or(u32::MAX)
    };
    let leading = crate::Container::new()
      .derive_height(move || extent(window.get().0.start))
      .build();
    let items = crate::create_group();
    let trailing = crate::Container::new()
      .derive_height(move || {
        let (window, count) = window.get();

        extent(count - window.end)
      })
      .build();

    with_state(|state| {
      state.attach_child(outer, leading, None);
      state.attach_child(outer, items, None);
      state.attach_child(outer, trailing, None);
    });

    let item = self.item;

    skene_core::reconcile::<crate::TuiOps, usize, _, usize>(
      items,
      move || {
        let (window, _) = window.get();

        window.start..window.end
      },
      |index| *index,
      move |index| item(index),
    );

    outer
  }
}
