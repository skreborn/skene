//! A clickable button, in cells.

use std::rc::Rc;

use skene_core::{Layout, NodeId, View};

use crate::{Children, control::ControlBase, state::NodeKind, style::box_taffy_style, with_state};

/// A button: focusable and clickable, its content the attached children.
///
/// The one focus-taking box: `Tab` reaches it, `Enter` and `Space` activate it, and a
/// mouse click focuses and activates it. Unstyled by default; styling is the box style,
/// set with [`set_style`](Self::set_style) and conditionally with
/// [`set_when`](Self::set_when).
#[must_use]
pub struct Button {
  base: ControlBase,
  children: Children,
  click_handler: Option<Rc<dyn Fn()>>,
}

impl Button {
  /// Creates an empty button.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      base: ControlBase::new(),
      children: Children::default(),
      click_handler: None,
    }
  }

  /// Sets the click handler (activation by mouse click, `Enter`, or `Space`).
  #[inline(always)]
  pub fn handle_click(mut self, action: impl Fn() + 'static) -> Self {
    self.click_handler = Some(Rc::new(action));

    self
  }
}

impl Default for Button {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

crate::control::control_surface!(Button, crate::props::BoxStyle);

impl skene_core::HasChildren<crate::TuiOps> for Button {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for Button {
  fn build(mut self) -> NodeId {
    let style = self.base.style;
    let variants = std::mem::take(&mut self.base.variants);
    let node = with_state(|state| {
      let node = state.insert_node(
        NodeKind::Box {
          layout: Layout::Block,
          style,
          variants,
        },
        box_taffy_style(Layout::Block, &style, false),
      );

      if let Some(action) = self.click_handler {
        state
          .click
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      node
    });

    self.base.register(node);
    self.children.mount(node);

    node
  }
}
