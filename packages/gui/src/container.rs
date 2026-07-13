// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::{Axis, Color, CrossAxisAlignment, MainAxisAlignment, MouseCursor, Source};

use crate::{Border, Children, HasChildren, Inset, Layout, Role};

/// A box that decorates itself and lays out its children.
///
/// Constructed in a layout mode: [`new`](Container::new) (block normal flow),
/// [`row`](Container::row) / [`column`](Container::column) (flex),
/// [`grid`](Container::grid), or [`stack`](Container::stack) (children overlapping in
/// one cell). Children are set with [`set_children`](HasChildren::set_children) or
/// `derive_children`; decoration, sizing, and layout are set with the chainable
/// `set_`/`derive_` methods. Each platform realizes it its own way.
#[must_use]
pub struct Container {
  pub(crate) children: Children,
  pub(crate) layout: Layout,
  pub(crate) role: Option<Role>,
  pub(crate) width: Option<Source<f32>>,
  pub(crate) height: Option<Source<f32>>,
  pub(crate) min_width: Option<Source<f32>>,
  pub(crate) min_height: Option<Source<f32>>,
  pub(crate) max_width: Option<Source<f32>>,
  pub(crate) max_height: Option<Source<f32>>,
  pub(crate) padding: Option<Source<Inset>>,
  pub(crate) color: Option<Source<Color>>,
  pub(crate) border: Option<Source<Border>>,
  pub(crate) border_radius: Option<Source<f32>>,
  pub(crate) cursor: Option<Source<MouseCursor>>,
  pub(crate) spacing: Option<Source<f32>>,
  pub(crate) main_axis_alignment: Option<Source<MainAxisAlignment>>,
  pub(crate) cross_axis_alignment: Option<Source<CrossAxisAlignment>>,
  pub(crate) click_handler: Option<Rc<dyn Fn()>>,
}

impl Container {
  #[inline(always)]
  fn mode(layout: Layout) -> Self {
    Self {
      children: Children::default(),
      layout,
      role: None,
      width: None,
      height: None,
      min_width: None,
      min_height: None,
      max_width: None,
      max_height: None,
      padding: None,
      color: None,
      border: None,
      border_radius: None,
      cursor: None,
      spacing: None,
      main_axis_alignment: None,
      cross_axis_alignment: None,
      click_handler: None,
    }
  }

  /// A block container: children flow in normal document order, contained within it.
  #[inline(always)]
  pub fn new() -> Self {
    Container::mode(Layout::Block)
  }

  /// A flex container laying children out left to right.
  #[inline(always)]
  pub fn row() -> Self {
    Container::mode(Layout::Flex {
      direction: Axis::Horizontal,
    })
  }

  /// A flex container laying children out top to bottom.
  #[inline(always)]
  pub fn column() -> Self {
    Container::mode(Layout::Flex {
      direction: Axis::Vertical,
    })
  }

  /// A grid container with `columns` equal-width columns.
  #[inline(always)]
  pub fn grid(columns: u16) -> Self {
    Container::mode(Layout::Grid { columns })
  }

  /// A container overlapping its children in one cell, stacked in order (last on top).
  #[inline(always)]
  pub fn stack() -> Self {
    Container::mode(Layout::Stack)
  }

  /// Sets the semantic role.
  #[inline(always)]
  pub fn set_role(mut self, role: Role) -> Self {
    self.role = Some(role);

    self
  }

  /// Sets a static width.
  #[inline(always)]
  pub fn set_width(mut self, width: f32) -> Self {
    self.width = Some(Source::Static(width));

    self
  }

  /// Sets a derived width.
  #[inline(always)]
  pub fn derive_width(mut self, width: impl Fn() -> f32 + 'static) -> Self {
    self.width = Some(Source::Derived(Rc::new(width)));

    self
  }

  /// Sets a static height.
  #[inline(always)]
  pub fn set_height(mut self, height: f32) -> Self {
    self.height = Some(Source::Static(height));

    self
  }

  /// Sets a derived height.
  #[inline(always)]
  pub fn derive_height(mut self, height: impl Fn() -> f32 + 'static) -> Self {
    self.height = Some(Source::Derived(Rc::new(height)));

    self
  }

  /// Sets a static minimum width.
  #[inline(always)]
  pub fn set_min_width(mut self, min_width: f32) -> Self {
    self.min_width = Some(Source::Static(min_width));

    self
  }

  /// Sets a derived minimum width.
  #[inline(always)]
  pub fn derive_min_width(mut self, min_width: impl Fn() -> f32 + 'static) -> Self {
    self.min_width = Some(Source::Derived(Rc::new(min_width)));

    self
  }

  /// Sets a static minimum height.
  #[inline(always)]
  pub fn set_min_height(mut self, min_height: f32) -> Self {
    self.min_height = Some(Source::Static(min_height));

    self
  }

  /// Sets a derived minimum height.
  #[inline(always)]
  pub fn derive_min_height(mut self, min_height: impl Fn() -> f32 + 'static) -> Self {
    self.min_height = Some(Source::Derived(Rc::new(min_height)));

    self
  }

  /// Sets a static maximum width.
  #[inline(always)]
  pub fn set_max_width(mut self, max_width: f32) -> Self {
    self.max_width = Some(Source::Static(max_width));

    self
  }

  /// Sets a derived maximum width.
  #[inline(always)]
  pub fn derive_max_width(mut self, max_width: impl Fn() -> f32 + 'static) -> Self {
    self.max_width = Some(Source::Derived(Rc::new(max_width)));

    self
  }

  /// Sets a static maximum height.
  #[inline(always)]
  pub fn set_max_height(mut self, max_height: f32) -> Self {
    self.max_height = Some(Source::Static(max_height));

    self
  }

  /// Sets a derived maximum height.
  #[inline(always)]
  pub fn derive_max_height(mut self, max_height: impl Fn() -> f32 + 'static) -> Self {
    self.max_height = Some(Source::Derived(Rc::new(max_height)));

    self
  }

  /// Sets a static padding.
  #[inline(always)]
  pub fn set_padding(mut self, padding: Inset) -> Self {
    self.padding = Some(Source::Static(padding));

    self
  }

  /// Sets a derived padding.
  #[inline(always)]
  pub fn derive_padding(mut self, padding: impl Fn() -> Inset + 'static) -> Self {
    self.padding = Some(Source::Derived(Rc::new(padding)));

    self
  }

  /// Sets a static background color.
  #[inline(always)]
  pub fn set_color(mut self, color: Color) -> Self {
    self.color = Some(Source::Static(color));

    self
  }

  /// Sets a derived background color.
  #[inline(always)]
  pub fn derive_color(mut self, color: impl Fn() -> Color + 'static) -> Self {
    self.color = Some(Source::Derived(Rc::new(color)));

    self
  }

  /// Sets a static border.
  #[inline(always)]
  pub fn set_border(mut self, border: Border) -> Self {
    self.border = Some(Source::Static(border));

    self
  }

  /// Sets a derived border.
  #[inline(always)]
  pub fn derive_border(mut self, border: impl Fn() -> Border + 'static) -> Self {
    self.border = Some(Source::Derived(Rc::new(border)));

    self
  }

  /// Sets a static border radius.
  #[inline(always)]
  pub fn set_border_radius(mut self, border_radius: f32) -> Self {
    self.border_radius = Some(Source::Static(border_radius));

    self
  }

  /// Sets a derived border radius.
  #[inline(always)]
  pub fn derive_border_radius(mut self, border_radius: impl Fn() -> f32 + 'static) -> Self {
    self.border_radius = Some(Source::Derived(Rc::new(border_radius)));

    self
  }

  /// Sets a static mouse cursor.
  #[inline(always)]
  pub fn set_cursor(mut self, cursor: MouseCursor) -> Self {
    self.cursor = Some(Source::Static(cursor));

    self
  }

  /// Sets a derived mouse cursor.
  #[inline(always)]
  pub fn derive_cursor(mut self, cursor: impl Fn() -> MouseCursor + 'static) -> Self {
    self.cursor = Some(Source::Derived(Rc::new(cursor)));

    self
  }

  /// Sets a static gap between children.
  #[inline(always)]
  pub fn set_spacing(mut self, spacing: f32) -> Self {
    self.spacing = Some(Source::Static(spacing));

    self
  }

  /// Sets a derived gap between children.
  #[inline(always)]
  pub fn derive_spacing(mut self, spacing: impl Fn() -> f32 + 'static) -> Self {
    self.spacing = Some(Source::Derived(Rc::new(spacing)));

    self
  }

  /// Sets a static main axis alignment.
  #[inline(always)]
  pub fn set_main_axis_alignment(mut self, alignment: MainAxisAlignment) -> Self {
    self.main_axis_alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived main axis alignment.
  #[inline(always)]
  pub fn derive_main_axis_alignment(
    mut self,
    alignment: impl Fn() -> MainAxisAlignment + 'static,
  ) -> Self {
    self.main_axis_alignment = Some(Source::Derived(Rc::new(alignment)));

    self
  }

  /// Sets a static cross axis alignment.
  #[inline(always)]
  pub fn set_cross_axis_alignment(mut self, alignment: CrossAxisAlignment) -> Self {
    self.cross_axis_alignment = Some(Source::Static(alignment));

    self
  }

  /// Sets a derived cross axis alignment.
  #[inline(always)]
  pub fn derive_cross_axis_alignment(
    mut self,
    alignment: impl Fn() -> CrossAxisAlignment + 'static,
  ) -> Self {
    self.cross_axis_alignment = Some(Source::Derived(Rc::new(alignment)));

    self
  }

  /// Sets the click handler.
  #[inline(always)]
  pub fn handle_click(mut self, action: impl Fn() + 'static) -> Self {
    self.click_handler = Some(Rc::new(action));

    self
  }
}

impl Default for Container {
  #[inline(always)]
  fn default() -> Self {
    Container::new()
  }
}

impl HasChildren for Container {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}
