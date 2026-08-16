//! A clickable button, in cells.

use skene_core::{Layout, NodeId, Prevent, View};

use crate::{
  Children,
  control::{Behavior, ControlBase},
  state::NodeKind,
  style::box_taffy_style,
  with_state,
};

/// A button: focusable and clickable, its content the attached children.
///
/// The one focus-taking box: `Tab` reaches it, `Enter` and `Space` activate it, and a
/// mouse click focuses and activates it. Unstyled by default; styling is the box style,
/// spelled per property through [`HasDecoration`](crate::HasDecoration), as a
/// [`Container`](crate::Container)'s is.
///
/// The unstyled base a component library builds on; [`SimpleButton`](crate::SimpleButton) is
/// the ready-made counterpart.
#[must_use]
pub struct RawButton {
  base: ControlBase<crate::props::BoxStyle>,
  children: Children,
}

impl RawButton {
  /// Creates an empty button.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      children: Children::default(),
    }
  }
}

impl Default for RawButton {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl skene_core::HasChildren<crate::TuiOps> for RawButton {
  type Kind = skene_core::PlainChild;

  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for RawButton {
  fn build(mut self) -> NodeId {
    let style = self.base.style.style.clone();
    let variants = std::mem::take(&mut self.base.style.variants);
    let node = with_state(|state| {
      state.insert_node(
        NodeKind::Box {
          layout: Layout::Flow,
          style: style.clone(),
          variants,
        },
        box_taffy_style(Layout::Flow, &style, false),
      )
    });

    self
      .base
      .register(node, Behavior::new().click(|| Prevent::Bubbling));
    crate::theme::build_unselectable(|| self.children.mount(node));

    node
  }
}

impl crate::HasEvents for RawButton {
  #[inline(always)]
  fn event_core(&mut self) -> &mut crate::EventCore {
    &mut self.base.events
  }
}

impl crate::HasFocus for RawButton {
  #[inline(always)]
  fn focus_core(&mut self) -> &mut crate::FocusCore {
    &mut self.base.focus
  }
}

impl crate::HasDecoration for RawButton {
  #[inline(always)]
  fn decoration_core(&mut self) -> &mut crate::DecorationCore {
    &mut self.base.style
  }
}
