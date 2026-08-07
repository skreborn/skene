//! Content above the app: modals and popups, in cells.

use std::{cell::Cell, rc::Rc};

use skene_core::{Color, NodeId, Placement, Source, View};

use crate::{
  Children,
  props::{BoxStyle, Condition},
  state::NodeKind,
  with_state,
};

/// A handle naming a built view as an overlay's anchor.
///
/// Create one, attach it to the anchoring view with its `bind_anchor`, and hand it to
/// the overlay's [`set_anchor`](Overlay::set_anchor); the open overlay positions
/// against that view.
#[derive(Clone, Default)]
#[must_use]
pub struct Anchor(Rc<Cell<Option<NodeId>>>);

impl Anchor {
  /// Creates an unbound anchor.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Binds the anchor to a built view's node.
  pub(crate) fn assign(&self, node: NodeId) {
    self.0.set(Some(node));
  }

  /// The bound node, once the anchoring view built.
  pub(crate) fn node(&self) -> Option<NodeId> {
    self.0.get()
  }
}

/// An overlay: content above the app, opened and closed by a signal.
///
/// A [`modal`](Self::modal) contains the tab order, blocks interaction beneath it, and
/// can carry a [backdrop](Self::set_backdrop) scrim; a [`popup`](Self::popup) floats
/// without blocking and can position against an [anchor](Self::set_anchor). Either way
/// the overlay never closes itself: Escape or a press outside fire
/// [`handle_dismiss`](Self::handle_dismiss), and the open state stays the app's—
/// [`bind_open`](Self::bind_open) wires both directions to one signal.
#[must_use]
pub struct Overlay {
  modal: bool,
  open: Option<Source<bool>>,
  placement: Placement,
  backdrop: Option<Color>,
  style: BoxStyle,
  variants: Vec<(Condition, BoxStyle)>,
  anchor: Option<Anchor>,
  dismiss_handler: Option<Rc<dyn Fn()>>,
  children: Children,
}

impl Overlay {
  fn new(modal: bool) -> Self {
    Self {
      modal,
      open: None,
      placement: Placement::Center,
      backdrop: modal.then(|| Color::srgb(0.0, 0.0, 0.0).with_alpha(0.5)),
      style: BoxStyle::new(),
      variants: Vec::new(),
      anchor: None,
      dismiss_handler: None,
      children: Children::default(),
    }
  }

  /// Creates a modal overlay: the tab order contained, interaction beneath blocked.
  #[inline(always)]
  pub fn modal() -> Self {
    Self::new(true)
  }

  /// Creates a popup overlay: floating, without blocking the app beneath.
  #[inline(always)]
  pub fn popup() -> Self {
    Self::new(false)
  }

  /// Sets a static open state.
  #[inline(always)]
  pub fn set_open(mut self, open: bool) -> Self {
    self.open = Some(Source::Static(open));

    self
  }

  /// Sets a derived open state, re-applied whenever it changes.
  #[inline(always)]
  pub fn derive_open(mut self, open: impl Fn() -> bool + 'static) -> Self {
    self.open = Some(Source::Derived(Rc::new(open)));

    self
  }

  /// Binds the open state to `signal` in both directions: the overlay follows the
  /// signal, and a dismiss request writes it back to `false`.
  #[inline(always)]
  pub fn bind_open(self, signal: skene_reactive::Signal<bool>) -> Self {
    use skene_reactive::{Get, Set};

    self
      .derive_open(move || signal.get())
      .handle_dismiss(move || signal.set(false))
  }

  /// Sets where the overlay sits; anchored, `Bottom` and `Top` open below and above
  /// the anchor.
  #[inline(always)]
  pub fn set_placement(mut self, placement: Placement) -> Self {
    self.placement = placement;

    self
  }

  /// Sets the backdrop scrim mixed over the app while a modal is open, by the color's
  /// alpha; half-transparent black by default, and a fully transparent color disables
  /// it.
  #[inline(always)]
  pub fn set_backdrop(mut self, color: Color) -> Self {
    self.backdrop = Some(color);

    self
  }

  /// Sets the content box's static style.
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

  /// Positions the popup against the view `anchor` is bound to.
  #[inline(always)]
  pub fn set_anchor(mut self, anchor: &Anchor) -> Self {
    self.anchor = Some(anchor.clone());

    self
  }

  /// Sets the dismiss handler, run when the user asks the overlay to close (Escape, or
  /// a press outside it).
  #[inline(always)]
  pub fn handle_dismiss(mut self, action: impl Fn() + 'static) -> Self {
    self.dismiss_handler = Some(Rc::new(action));

    self
  }
}

impl skene_core::HasChildren<crate::TuiOps> for Overlay {
  #[inline(always)]
  fn set_children_to(mut self, children: Children) -> Self {
    self.children = children;

    self
  }
}

impl View for Overlay {
  fn build(self) -> NodeId {
    let node = with_state(|state| {
      let node = state.insert_node(
        NodeKind::Overlay {
          modal: self.modal,
          open: false,
          placement: self.placement,
          backdrop: self.backdrop,
          anchor: None,
          style: self.style,
          variants: self.variants,
        },
        taffy::style::Style {
          display: taffy::style::Display::None,
          position: taffy::style::Position::Absolute,
          ..taffy::style::Style::default()
        },
      );

      state.overlays.borrow_mut().push(node);

      if let Some(action) = self.dismiss_handler {
        state
          .dismiss
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      node
    });

    self.children.mount(node);

    if let Some(open) = self.open {
      let anchor = self.anchor;

      open.bind(move |open| {
        with_state(|state| {
          state.set_overlay_open(node, open, anchor.as_ref().and_then(Anchor::node));
        });
      });
    }

    node
  }
}
