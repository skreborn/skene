//! A clickable button.

use std::rc::Rc;

use crate::{Children, HasChildren, control::ControlBase};

/// A button: natively focusable and clickable, its content the attached children.
///
/// The one focus-taking box: keyboard activation (Enter and Space), the tab order, and
/// the platform's button semantics come with the element. Unstyled by default; styling
/// is the box style, set with `set_style` and conditionally with `set_when`.
///
/// The unstyled base a component library builds on; [`SimpleButton`](crate::SimpleButton) is
/// the ready-made counterpart.
#[must_use]
pub struct RawButton {
  children: Children,
  click_handler: Option<Rc<skene_core::ClickHandler>>,
  control: ControlBase,
}

impl RawButton {
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
  pub fn handle_click<R: Into<skene_core::Prevent>>(
    mut self,
    action: impl Fn() -> R + 'static,
  ) -> Self {
    self.click_handler = Some(Rc::new(move || action().into()));

    self
  }
}

impl Default for RawButton {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl HasChildren<crate::GuiOps> for RawButton {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl skene_core::View for RawButton {
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

impl crate::HasEvents for RawButton {
  #[inline(always)]
  fn event_core(&mut self) -> &mut crate::EventCore {
    &mut self.control.events
  }
}

impl crate::HasFocus for RawButton {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    &mut self.control.focus
  }
}

impl crate::HasDecoration for RawButton {
  #[inline(always)]
  fn decoration_core(&mut self) -> &mut crate::DecorationCore {
    &mut self.control.style
  }
}
