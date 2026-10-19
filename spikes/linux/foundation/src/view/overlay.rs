//! Content above the app.

use std::{cell::Cell, rc::Rc};

use crate::{
  Effect, Owner, Signal,
  seam::{
    Realization,
    content::{OverlayPlacement, Side, contents},
    node::{NodeKind, NodeSpec},
    record::Record,
    tree,
  },
  untrack,
  value::IntoUnit,
  view::{AnyKind, Child, Cx, Node, NodeHandle, View, Wraps, wraps_itself},
};

/// Content above the app: a popup placed against an anchor, opened and closed by the app.
///
/// It takes no room where it stands. Opening and closing it show and hide it, so focus and
/// scroll inside it survive; its child is built the first time it opens. It never closes
/// itself: a press outside it and its anchor, or the platform's own dismissal, only runs
/// [`OverlayBase::handle_dismiss`]. Opening, the first control inside it that asks for focus
/// (`set_autofocus`) takes it; closing, focus inside it returns to what held it when it
/// opened.
#[must_use]
pub struct Overlay<R: Realization> {
  anchor: Rc<dyn Fn() -> Option<NodeHandle>>,
  side: Rc<dyn Fn() -> Side>,
  widen: Rc<dyn Fn() -> bool>,
  point: Rc<dyn Fn() -> Option<(i32, i32)>>,
  gap: Rc<dyn Fn() -> f32>,
  open: Rc<dyn Fn() -> bool>,
  dismiss: Option<Rc<dyn Fn()>>,
  child: Option<Child<R>>,
}

wraps_itself!(Overlay<R> where R: Realization);

/// What an [`Overlay`] carries: its anchor, its side, its width, its point, its gap, whether
/// it is open, its dismissal and its child.
pub trait OverlayBase<R: Realization>: Wraps<Inner = Overlay<R>> {
  /// A closed popup below its anchor, at least as wide, against the node it stands in where
  /// no anchor is set.
  fn popup() -> Self {
    Self::wrap(Overlay {
      anchor: Rc::new(|| None),
      side: Rc::new(|| Side::Bottom),
      widen: Rc::new(|| true),
      point: Rc::new(|| None),
      gap: Rc::new(|| 2.),
      open: Rc::new(|| false),
      dismiss: None,
      child: None,
    })
  }

  /// The node it is placed against.
  fn set_anchor(self, anchor: &NodeHandle) -> Self {
    let anchor = *anchor;

    self.derive_anchor(move || Some(anchor))
  }

  /// The node it is placed against, kept current: against the node it stands in where `None`.
  fn derive_anchor(mut self, anchor: impl Fn() -> Option<NodeHandle> + 'static) -> Self {
    self.inner_mut().anchor = Rc::new(anchor);

    self
  }

  /// The side of its anchor it opens on; below where unset.
  fn set_placement(self, side: Side) -> Self {
    self.derive_placement(move || side)
  }

  /// The side of its anchor it opens on, kept current.
  fn derive_placement(mut self, side: impl Fn() -> Side + 'static) -> Self {
    self.inner_mut().side = Rc::new(side);

    self
  }

  /// Whether the open overlay is at least as wide as its anchor; true where unset.
  fn set_widen_to_anchor(self, widen: bool) -> Self {
    self.derive_widen_to_anchor(move || widen)
  }

  /// Whether the open overlay is at least as wide as its anchor, kept current.
  fn derive_widen_to_anchor(mut self, widen: impl Fn() -> bool + 'static) -> Self {
    self.inner_mut().widen = Rc::new(widen);

    self
  }

  /// The point of the window it hangs from, its top leading corner there, in place of its
  /// anchor (a context menu at the pointer): window pixels, against its anchor where `None`.
  fn set_point(self, point: Option<(i32, i32)>) -> Self {
    self.derive_point(move || point)
  }

  /// The point of the window it hangs from, kept current.
  fn derive_point(mut self, point: impl Fn() -> Option<(i32, i32)> + 'static) -> Self {
    self.inner_mut().point = Rc::new(point);

    self
  }

  /// The space between it and its anchor, in pixels; 2 where unset.
  fn set_gap(self, gap: impl IntoUnit<f32>) -> Self {
    let gap = gap.into_unit();

    self.derive_gap(move || gap)
  }

  /// The space between it and its anchor, in pixels, kept current.
  fn derive_gap(mut self, gap: impl Fn() -> f32 + 'static) -> Self {
    self.inner_mut().gap = Rc::new(gap);

    self
  }

  /// Whether it is open.
  fn set_open(self, open: bool) -> Self {
    self.derive_open(move || open)
  }

  /// Whether it is open, kept current.
  fn derive_open(mut self, open: impl Fn() -> bool + 'static) -> Self {
    self.inner_mut().open = Rc::new(open);

    self
  }

  /// Opens it while `open` holds, and closes it (writing `false`) when it is dismissed.
  fn bind_open(self, open: Signal<bool>) -> Self {
    self
      .derive_open(move || open.get())
      .handle_dismiss(move || open.set(false))
  }

  /// Runs `dismiss` when the reader dismisses the open overlay: a press outside it and its
  /// anchor, or the platform's own dismissal.
  fn handle_dismiss(mut self, dismiss: impl Fn() + 'static) -> Self {
    self.inner_mut().dismiss = Some(Rc::new(dismiss));

    self
  }

  /// What it shows, last.
  fn set_child(mut self, child: impl View<R>) -> Self {
    self.inner_mut().child = Some(Child::view(child));

    self
  }
}

impl<R: Realization> OverlayBase<R> for Overlay<R> {}

impl<R: Realization> AnyKind for Overlay<R> {}

impl<R: Realization> View<R> for Overlay<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      anchor,
      side,
      widen,
      point,
      gap,
      open,
      dismiss,
      child,
    } = self;
    let id = cx.create(NodeSpec::new(NodeKind::Overlay));

    cx.finish(id, Record::new(), None);
    tree::set_hidden::<R>(id, true);
    tree::bind::<R, _>(id, contents::PLACEMENT, move || OverlayPlacement {
      anchor: anchor().and_then(|anchor| anchor.node()),
      side: side(),
      widen: widen(),
      at: point(),
      #[expect(clippy::cast_possible_truncation, reason = "whole pixels")]
      gap: gap().round() as i32,
    });

    if let Some(dismiss) = dismiss {
      tree::handle_dismiss::<R>(id, move || dismiss());
    }

    let owner = Owner::new();
    let inner = cx.child();
    let pending = Cell::new(child);
    let shown = Cell::new(false);

    Effect::new(move || {
      let open = open();

      untrack(|| {
        if open && let Some(child) = pending.take() {
          let node = owner.run(|| child.build(&mut inner.child()));

          tree::attach::<R>(id, node.id(), 0);
        }

        if shown.replace(open) != open {
          tree::set_hidden::<R>(id, !open);
        }
      });
    });

    Node::new(id)
  }
}
