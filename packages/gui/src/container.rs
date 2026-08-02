// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::{
  Animation,
  Axis,
  BoxStyle,
  Color,
  Condition,
  CrossAxisAlignment,
  KeyEvent,
  MainAxisAlignment,
  MouseCursor,
  Overflow,
  PointerEvent,
  Prevent,
  ScrollEvent,
  Source,
  Transition,
};

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
  pub(crate) overflow: Option<Source<Overflow>>,
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
      overflow: None,
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

  /// Adds a conditional style variant, applied while `condition` holds.
  ///
  /// Declarative: it compiles to a stylesheet rule, so it works in static output with no
  /// script. Later variants win over earlier ones.
  #[inline(always)]
  pub fn set_when(mut self, condition: Condition, style: BoxStyle) -> Self {
    self.variants.push((condition, style));

    self
  }

  /// Adds a property transition, interpolating that property whenever its value changes.
  #[inline(always)]
  pub fn set_transition(mut self, transition: Transition) -> Self {
    self.transitions.push(transition);

    self
  }

  /// Adds a keyframe animation.
  #[inline(always)]
  pub fn set_animation(mut self, animation: Animation<BoxStyle>) -> Self {
    self.animations.push(animation);

    self
  }

  /// Sets the click handler.
  #[inline(always)]
  pub fn handle_click<R: Into<Prevent>>(mut self, action: impl Fn() -> R + 'static) -> Self {
    self.click_handler = Some(Rc::new(move || action().into()));

    self
  }

  /// Sets the pointer-enter handler.
  #[inline(always)]
  pub fn handle_pointer_enter(mut self, action: impl Fn() + 'static) -> Self {
    self.pointer_enter_handler = Some(Rc::new(action));

    self
  }

  /// Sets the pointer-leave handler.
  #[inline(always)]
  pub fn handle_pointer_leave(mut self, action: impl Fn() + 'static) -> Self {
    self.pointer_leave_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler, run when the box or a descendant gains focus.
  ///
  /// A box never takes focus itself—only controls do; this observes focus within it.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler, run when the box or a descendant loses focus.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.blur_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler, run for keys pressed on the box or within it.
  #[inline(always)]
  pub fn handle_key_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&KeyEvent) -> R + 'static,
  ) -> Self {
    self.key_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-down handler, run for presses on the box or within it.
  #[inline(always)]
  pub fn handle_pointer_down<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_down_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-up handler, run for releases on the box or within it.
  #[inline(always)]
  pub fn handle_pointer_up<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_up_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the pointer-move handler, run for moves over the box or within it.
  #[inline(always)]
  pub fn handle_pointer_move<R: Into<Prevent>>(
    mut self,
    action: impl Fn(&PointerEvent) -> R + 'static,
  ) -> Self {
    self.pointer_move_handler = Some(Rc::new(move |event| action(event).into()));

    self
  }

  /// Sets the scroll handler, run with the new offsets when the box scrolls.
  ///
  /// The box scrolls only with an overflow that allows it; see
  /// [`set_overflow`](Self::set_overflow).
  #[inline(always)]
  pub fn handle_scroll(mut self, action: impl Fn(&ScrollEvent) + 'static) -> Self {
    self.scroll_handler = Some(Rc::new(action));

    self
  }

  /// Sets a static overflow behavior.
  #[inline(always)]
  pub fn set_overflow(mut self, overflow: Overflow) -> Self {
    self.overflow = Some(Source::Static(overflow));

    self
  }

  /// Sets a derived overflow behavior.
  #[inline(always)]
  pub fn derive_overflow(mut self, overflow: impl Fn() -> Overflow + 'static) -> Self {
    self.overflow = Some(Source::Derived(Rc::new(overflow)));

    self
  }

  /// Binds `anchor` to the box, so an overlay's
  /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
  #[inline(always)]
  pub fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
    self.anchor = Some(anchor.clone());

    self
  }
}

impl Default for Container {
  #[inline(always)]
  fn default() -> Self {
    Container::new()
  }
}

impl HasChildren<crate::GuiOps> for Container {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

#[cfg(target_arch = "wasm32")]
impl skene_core::View for Container {
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
      self.padding,
      &mut style.padding,
      &mut binders,
      BoxProp::Padding,
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
      self.main_axis_alignment,
      &mut style.main_axis_alignment,
      &mut binders,
      BoxProp::MainAxisAlignment,
      driver::update_box,
    );

    split_prop(
      self.cross_axis_alignment,
      &mut style.cross_axis_alignment,
      &mut binders,
      BoxProp::CrossAxisAlignment,
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
