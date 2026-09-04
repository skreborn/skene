//! Content above the app: modals and popups.

use std::rc::Rc;

use skene_core::{
  Color,
  Layout,
  Placement,
  next::{
    AnyKind,
    AnyView,
    DeclaredBoxStyle,
    Driver,
    HasChildren,
    IntoView,
    NoChild,
    NodeId,
    NodeKind,
    NodeRecord,
    OverlayProps,
    PlainChild,
    PlainHandler,
    Scene,
    View,
  },
};
use skene_reactive::{Effect, Get, Set, Signal, untrack};

use crate::{Anchor, LayoutBox, macros::decoration, realize::Source};

/// The backdrop a modal paints unless told otherwise: half-transparent black.
#[inline(always)]
fn modal_backdrop() -> Color {
  Color::srgb(0., 0., 0.).with_alpha(0.5)
}

/// Content above the app, opened and closed by the app: a [`modal`](Self::modal) traps
/// focus, blocks interaction beneath it and paints a [backdrop](Self::set_backdrop); a
/// [`popup`](Self::popup) floats without blocking and is placed against an
/// [anchor](Self::set_anchor).
///
/// The overlay never closes itself: `Escape`, a backdrop click or a press outside a
/// popup only run [`handle_dismiss`](Self::handle_dismiss), and the open state stays
/// what [`derive_open`](Self::derive_open) reads; [`bind_open`](Self::bind_open) wires
/// both to one signal. Opening and closing attach nothing and destroy nothing, so state
/// inside the content survives. Its content box is decorated like a container. Holds
/// [`NoChild`] until [`set_child`](Self::set_child) supplies the content.
#[must_use]
pub struct Overlay<D: Driver, C = NoChild> {
  modal: bool,
  placement: Source<Placement>,
  backdrop: Option<Color>,
  anchor: Option<Anchor>,
  open: Source<bool>,
  dismiss: Vec<Rc<PlainHandler>>,
  inner: LayoutBox<D, PlainChild>,
  child: C,
}

impl<D: Driver> Overlay<D> {
  fn new(modal: bool) -> Self {
    Self {
      modal,
      placement: Source::Fixed(Placement::Center),
      backdrop: modal.then(modal_backdrop),
      anchor: None,
      open: Source::Fixed(false),
      dismiss: Vec::new(),
      inner: LayoutBox::new(Layout::Single),
      child: NoChild,
    }
  }

  /// A modal: focus trapped, interaction beneath blocked, a backdrop painted.
  #[inline(always)]
  pub fn modal() -> Self {
    Self::new(true)
  }

  /// A popup: floating, blocking nothing beneath it, without a backdrop.
  #[inline(always)]
  pub fn popup() -> Self {
    Self::new(false)
  }

  /// Sets the content.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<D>) -> Overlay<D, AnyView<D>> {
    Overlay {
      modal: self.modal,
      placement: self.placement,
      backdrop: self.backdrop,
      anchor: self.anchor,
      open: self.open,
      dismiss: self.dismiss,
      inner: self.inner,
      child: child.into_view(),
    }
  }
}

impl<D: Driver, C> Overlay<D, C> {
  #[inline(always)]
  const fn style_mut(&mut self) -> &mut DeclaredBoxStyle<D> {
    self.inner.style_mut()
  }

  decoration! { style_mut }

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
  pub fn set_anchor(mut self, anchor: &Anchor) -> Self {
    self.anchor = Some(anchor.clone());

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

impl<D: Driver, C> AnyKind for Overlay<D, C> {}

impl<D: Driver> View<D> for Overlay<D, AnyView<D>> {
  fn build(self) -> NodeId {
    let Self {
      modal,
      placement,
      backdrop,
      anchor,
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
      .set_child(child)
      .realize(NodeKind::Overlay, |box_props| {
        D::default().create_overlay(&OverlayProps {
          modal,
          placement: initial,
          backdrop,
          box_props,
        })
      });

    Scene::<D>::with(|scene| {
      for handler in dismiss {
        scene.handle_dismiss(node, handler);
      }
    });

    Effect::render(move |_: Option<()>| {
      let placement = placement.as_ref().map(|placement| placement());
      let open = open();
      let anchor = anchor.as_ref().and_then(Anchor::node);

      Scene::<D>::with(|scene| {
        let Some(record) = scene.record(node)
        else {
          return;
        };
        let driver = D::default();

        if let Some(placement) = placement {
          driver.set_overlay_placement(record.sink(), placement);
        }

        let anchor = anchor
          .and_then(|anchor| scene.record(anchor))
          .map(NodeRecord::sink);

        driver.set_overlay_open(record.sink(), open, anchor);
      });
    });

    node
  }
}
