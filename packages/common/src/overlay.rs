//! Content above the app: modals and popups.

use std::rc::Rc;

use skene_core::{
  Animation,
  AnyKind,
  AnyView,
  BoxKeyframe,
  Color,
  DeclaredBoxStyle,
  Handler,
  HasChildren,
  IntoView,
  Layout,
  NoChild,
  NodeId,
  NodeKind,
  OverlayProps,
  Placement,
  PlainChild,
  PlainHandler,
  Platform,
  Point,
  Role,
  Scene,
  StyleExtension,
  Transition,
  View,
};
use skene_reactive::{Effect, Get, Set, Signal, untrack};

use crate::{LayoutBox, NodeHandle, decoration, realize::Source};

/// The backdrop a modal paints unless told otherwise: half-transparent black.
#[inline(always)]
fn modal_backdrop() -> Color {
  Color::srgb(0., 0., 0.).with_alpha(0.5)
}

/// Content above the app, opened and closed by the app.
///
/// A [`modal`](Self::modal) traps focus, blocks interaction beneath it and paints a
/// [backdrop](Self::set_backdrop); a [`popup`](Self::popup) floats without blocking and
/// is placed against an [anchor](Self::set_anchor).
///
/// The overlay never closes itself: `Escape`, a backdrop click or a press outside a
/// popup only run [`handle_dismiss`](Self::handle_dismiss), and the open state stays
/// what [`derive_open`](Self::derive_open) reads; [`bind_open`](Self::bind_open) wires
/// both to one signal. Opening and closing attach nothing and destroy nothing, so state
/// inside the content survives. Its content box is decorated like a container. Holds
/// [`NoChild`] until [`set_child`](Self::set_child) supplies the content.
#[must_use]
pub struct Overlay<P: Platform, C = NoChild> {
  modal: bool,
  hint: bool,
  placement: Source<Placement>,
  backdrop: Option<Color>,
  anchor: Option<NodeHandle<P>>,
  position: Option<Rc<dyn Fn() -> Option<Point<f32>>>>,
  open: Source<bool>,
  dismiss: Vec<Rc<PlainHandler>>,
  inner: LayoutBox<P, PlainChild>,
  child: C,
}

impl<P: Platform, C> std::fmt::Debug for Overlay<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Overlay").finish_non_exhaustive()
  }
}

impl<P: Platform> Overlay<P> {
  fn new(modal: bool, hint: bool) -> Self {
    Self {
      modal,
      hint,
      placement: Source::Fixed(Placement::Center),
      backdrop: modal.then(modal_backdrop),
      anchor: None,
      position: None,
      open: Source::Fixed(false),
      dismiss: Vec::new(),
      inner: LayoutBox::new(Layout::Single),
      child: NoChild,
    }
  }

  /// A modal: focus trapped, interaction beneath blocked, a backdrop painted.
  #[inline(always)]
  pub fn modal() -> Self {
    Self::new(true, false)
  }

  /// A popup: floating, blocking nothing beneath it, without a backdrop.
  #[inline(always)]
  pub fn popup() -> Self {
    Self::new(false, false)
  }

  /// A hint: a popup describing its anchor, which never takes focus and sits beneath
  /// every other popup.
  #[inline(always)]
  pub fn hint() -> Self {
    Self::new(false, true)
  }

  /// Sets the content.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<P>) -> Overlay<P, AnyView<P>> {
    Overlay {
      modal: self.modal,
      hint: self.hint,
      placement: self.placement,
      backdrop: self.backdrop,
      anchor: self.anchor,
      position: self.position,
      open: self.open,
      dismiss: self.dismiss,
      inner: self.inner,
      child: child.into_view(),
    }
  }
}

impl<P: Platform> Overlay<P> {
  /// The declared style of the content box, for a facade's medium-specific setters to
  /// write into.
  #[inline(always)]
  pub const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<P> {
    self.inner.style_mut()
  }

  decoration! { style_mut }

  crate::forward! { inner;
    /// Adds a property transition, interpolating that property whenever its value
    /// changes.
    fn set_transition(transition: Transition<<P::StyleExt as StyleExtension>::Property>);
    /// Adds a keyframe animation.
    fn set_animation(animation: Animation<BoxKeyframe<P>>);
    /// Names the box for view transitions: it is paired with the box of the same name
    /// across a navigation, where the platform animates between them.
    fn set_transition_name(name: impl Into<String>);
    /// Sets the overlay's accessible name, where no heading inside names it.
    fn set_label(label: impl Into<String>);
    /// Sets the node whose text names the overlay: its heading.
    fn set_labelled_by(node: NodeId);
    /// Sets the semantic role a popup takes: [`Role::Dialog`] for one holding a form; a
    /// modal is a dialog already.
    fn set_role(role: Role);
  }

  /// Sets where the overlay sits; anchored, `Bottom` and `Top` open below and above
  /// the anchor.
  #[inline(always)]
  pub fn set_placement(mut self, placement: Placement) -> Self {
    self.placement = Source::Fixed(placement);

    self
  }

  /// Derives where the overlay sits from the signals `placement` reads.
  #[inline(always)]
  pub fn derive_placement(mut self, placement: impl Fn() -> Placement + 'static) -> Self {
    self.placement = Source::derive(placement);

    self
  }

  /// Sets the backdrop painted over the app while open; `None` paints nothing.
  #[inline(always)]
  pub const fn set_backdrop(mut self, backdrop: Option<Color>) -> Self {
    self.backdrop = backdrop;

    self
  }

  /// Places the open overlay against the view `anchor` is bound to.
  #[inline(always)]
  pub const fn set_anchor(mut self, anchor: NodeHandle<P>) -> Self {
    self.anchor = Some(anchor);

    self
  }

  /// Pins the open overlay's top-start corner to `position`, in the units a pointer
  /// event reports, shifted to stay within the viewport; pinned, the overlay ignores
  /// its placement and anchor.
  #[inline(always)]
  pub fn set_position(mut self, position: Point<f32>) -> Self {
    self.position = Some(Rc::new(move || Some(position)));

    self
  }

  /// Derives where the open overlay is pinned from the signals `position` reads;
  /// `None` leaves it to its placement and anchor.
  #[inline(always)]
  pub fn derive_position(mut self, position: impl Fn() -> Option<Point<f32>> + 'static) -> Self {
    self.position = Some(Rc::new(position));

    self
  }

  /// Sets whether the overlay is open.
  #[inline(always)]
  pub fn set_open(mut self, open: bool) -> Self {
    self.open = Source::Fixed(open);

    self
  }

  /// Derives whether the overlay is open from the signals `open` reads.
  #[inline(always)]
  pub fn derive_open(mut self, open: impl Fn() -> bool + 'static) -> Self {
    self.open = Source::derive(open);

    self
  }

  /// Runs `action` when the user asks the overlay to close: `Escape`, a backdrop
  /// click, or a press outside a popup.
  #[inline(always)]
  pub fn handle_dismiss(mut self, action: impl Fn() + 'static) -> Self {
    self.dismiss.push(Rc::new(action));

    self
  }

  /// Binds the open state to `signal` in both directions: the overlay follows it, and
  /// a dismiss request writes `false` into it.
  #[inline(always)]
  pub fn bind_open(self, signal: Signal<bool>) -> Self {
    self
      .derive_open(move || signal.get())
      .handle_dismiss(move || signal.set(false))
  }
}

impl<P: Platform, C> AnyKind for Overlay<P, C> {}

impl<P: Platform> View<P> for Overlay<P, AnyView<P>> {
  fn build(self) -> NodeId {
    let Self {
      modal,
      hint,
      placement,
      backdrop,
      anchor,
      position,
      open,
      dismiss,
      inner,
      child,
    } = self;
    let (initial, placement) = match placement {
      Source::Fixed(placement) => (placement, None),
      Source::Derived(compute) => (untrack(|| compute()), Some(compute)),
    };
    let open: Rc<dyn Fn() -> bool> = match open {
      Source::Fixed(open) => Rc::new(move || open),
      Source::Derived(compute) => compute,
    };
    let node = inner
      .add_child(child)
      .realize(NodeKind::Overlay, |node, box_props| {
        P::create_overlay(
          node,
          &OverlayProps {
            modal,
            hint,
            placement: initial,
            backdrop,
            box_props,
          },
        );
      });

    Scene::<P>::with(|scene| {
      scene.note_overlay(node, modal);

      for handler in dismiss {
        scene.handle(node, Handler::Dismiss(handler));
      }
    });

    Effect::render(move |_: Option<()>| {
      let placement = placement.as_ref().map(|placement| placement());
      let position = position.as_ref().map(|position| position());
      let open = open();
      let anchor = anchor.and_then(|anchor| untrack(|| anchor.node()));

      if !Scene::<P>::with(|scene| scene.contains(node)) {
        return;
      }

      if let Some(placement) = placement {
        P::set_overlay_placement(node, placement);
      }

      if let Some(position) = position {
        P::set_overlay_position(node, position);
      }

      Scene::<P>::with(|scene| scene.set_overlay_open(node, open, anchor));
    });

    node
  }
}
