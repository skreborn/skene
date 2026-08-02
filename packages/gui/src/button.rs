//! A clickable button.

// View fields are read only by a platform's realization module; a target with no platform
// compiles the struct but no realizer, leaving them unread.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use crate::{
  Children,
  HasChildren,
  control::{ControlBase, control_surface},
};

/// A button: natively focusable and clickable, its content the attached children.
///
/// The one focus-taking box: keyboard activation (Enter and Space), the tab order, and
/// the platform's button semantics come with the element. Unstyled by default; styling
/// is the box style, set with `set_style` and conditionally with `set_when`.
#[must_use]
pub struct Button {
  children: Children,
  click_handler: Option<Rc<skene_core::ClickHandler>>,
  control: ControlBase,
}

impl Button {
  /// Creates an empty button.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      children: Children::default(),
      click_handler: None,
      control: ControlBase::new(),
    }
  }

  /// Sets the click handler (activation by pointer, Enter, or Space).
  #[inline(always)]
  pub fn handle_click<R: skene_core::IntoPrevent>(
    mut self,
    action: impl Fn() -> R + 'static,
  ) -> Self {
    self.click_handler = Some(Rc::new(move || {
      skene_core::IntoPrevent::into_prevent(action())
    }));

    self
  }
}

control_surface!(Button);

impl Default for Button {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl HasChildren<crate::GuiOps> for Button {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

#[cfg(target_arch = "wasm32")]
impl skene_core::View for Button {
  fn build(self) -> skene_core::NodeId {
    use skene_core::ButtonProps;

    use crate::driver;

    let (control, handlers) = self.control.into_parts();
    let node = driver::create_button(&ButtonProps { control });

    if let Some(action) = self.click_handler {
      driver::handle_click(node, action);
    }

    handlers.register(node);
    self.children.mount(node);

    node
  }
}
