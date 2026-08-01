//! A clickable button, in cells.

use std::rc::Rc;

use skene_core::{KeyEvent, Layout, NodeId, View};

use crate::{
  Children,
  FocusHandle,
  props::{BoxStyle, Condition},
  state::NodeKind,
  style::box_taffy_style,
  with_state,
};

/// A button: focusable and clickable, its content the attached children.
///
/// The one focus-taking box: `Tab` reaches it, `Enter` and `Space` activate it, and a
/// mouse click focuses and activates it. Unstyled by default; styling is the box style,
/// set with [`set_style`](Self::set_style) and conditionally with
/// [`set_when`](Self::set_when).
#[must_use]
pub struct Button {
  children: Children,
  style: BoxStyle,
  variants: Vec<(Condition, BoxStyle)>,
  click_handler: Option<Rc<dyn Fn()>>,
  focus_handler: Option<Rc<dyn Fn()>>,
  blur_handler: Option<Rc<dyn Fn()>>,
  #[expect(clippy::type_complexity)]
  key_down_handler: Option<Rc<dyn Fn(&KeyEvent)>>,
  autofocus: bool,
  focus_handle: Option<FocusHandle>,
}

impl Button {
  /// Creates an empty button.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      children: Children::default(),
      style: BoxStyle::new(),
      variants: Vec::new(),
      click_handler: None,
      focus_handler: None,
      blur_handler: None,
      key_down_handler: None,
      autofocus: false,
      focus_handle: None,
    }
  }

  /// Sets the static style.
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

  /// Sets the click handler (activation by mouse click, `Enter`, or `Space`).
  #[inline(always)]
  pub fn handle_click(mut self, action: impl Fn() + 'static) -> Self {
    self.click_handler = Some(Rc::new(action));

    self
  }

  /// Sets the focus handler.
  #[inline(always)]
  pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
    self.focus_handler = Some(Rc::new(action));

    self
  }

  /// Sets the blur handler.
  #[inline(always)]
  pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
    self.blur_handler = Some(Rc::new(action));

    self
  }

  /// Sets the key-press handler, run for keys pressed while the button is focused.
  #[inline(always)]
  pub fn handle_key_down(mut self, action: impl Fn(&KeyEvent) + 'static) -> Self {
    self.key_down_handler = Some(Rc::new(action));

    self
  }

  /// Makes the button take focus when it mounts.
  #[inline(always)]
  pub fn set_autofocus(mut self) -> Self {
    self.autofocus = true;

    self
  }

  /// Binds `handle` to the button, so [`FocusHandle::focus`](FocusHandle::focus)
  /// targets it once built.
  #[inline(always)]
  pub fn bind_focus(mut self, handle: &FocusHandle) -> Self {
    self.focus_handle = Some(handle.clone());

    self
  }
}

impl Default for Button {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl skene_core::HasChildren<crate::TuiOps> for Button {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for Button {
  fn build(self) -> NodeId {
    let node = with_state(|state| {
      let node = state.insert_node(
        NodeKind::Box {
          layout: Layout::Block,
          style: self.style,
          variants: self.variants,
        },
        box_taffy_style(Layout::Block, &self.style, false),
      );

      state.controls.borrow_mut().insert(node);

      if let Some(action) = self.click_handler {
        state
          .click
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

      if self.autofocus {
        state.focused.set(Some(node));
        state.dirty.set(true);
      }

      node
    });

    if let Some(handle) = &self.focus_handle {
      handle.assign(node);
    }

    self.children.mount(node);

    node
  }
}
