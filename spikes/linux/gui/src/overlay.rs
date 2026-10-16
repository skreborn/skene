//! Content above the app.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{
  Effect, Owner, Signal,
  seam::{
    content::{OverlayPlacement, Side, contents},
    node::{NodeKind, NodeSpec},
    record::Record,
    tree,
  },
  untrack,
};

use crate::{
  Active,
  view::{Child, Cx, Node, NodeHandle, View, child_of, finish},
};

/// Content above the app: a popup placed against an anchor, opened and closed by the app.
///
/// It takes no room where it stands. Opening and closing it show and hide it, so focus and
/// scroll inside it survive; its child is built the first time it opens. It never closes
/// itself: a press outside it and its anchor, or the platform's own dismissal, only runs
/// [`Overlay::handle_dismiss`].
///
/// ```
/// use skene_gui::prelude::*;
///
/// let open = Signal::new(false);
/// let anchor = NodeHandle::new();
///
/// Column::new()
///   .add_child(
///     Button::new("More")
///       .set_handle(&anchor)
///       .handle_activate(move || open.set(true)),
///   )
///   .add_child(
///     Overlay::popup()
///       .set_anchor(&anchor)
///       .bind_open(open)
///       .set_child(Text::new("Details")),
///   );
/// ```
#[must_use]
pub struct Overlay {
  anchor: Option<NodeHandle>,
  side: Side,
  widen: bool,
  open: Rc<dyn Fn() -> bool>,
  dismiss: Option<Rc<dyn Fn()>>,
  child: Option<Child>,
}

impl Overlay {
  /// A closed popup below its anchor, at least as wide, against the node it stands in where
  /// no anchor is set.
  pub fn popup() -> Self {
    Self {
      anchor: None,
      side: Side::Bottom,
      widen: true,
      open: Rc::new(|| false),
      dismiss: None,
      child: None,
    }
  }

  /// The node it is placed against.
  pub const fn set_anchor(mut self, anchor: &NodeHandle) -> Self {
    self.anchor = Some(*anchor);

    self
  }

  /// The side of its anchor it opens on; below where unset.
  pub const fn set_placement(mut self, side: Side) -> Self {
    self.side = side;

    self
  }

  /// Whether the open overlay is at least as wide as its anchor; true where unset.
  pub const fn set_widen_to_anchor(mut self, widen: bool) -> Self {
    self.widen = widen;

    self
  }

  /// Whether it is open.
  pub fn set_open(self, open: bool) -> Self {
    self.derive_open(move || open)
  }

  /// Whether it is open, kept current.
  pub fn derive_open(mut self, open: impl Fn() -> bool + 'static) -> Self {
    self.open = Rc::new(open);

    self
  }

  /// Opens it while `open` holds, and closes it (writing `false`) when it is dismissed.
  pub fn bind_open(self, open: Signal<bool>) -> Self {
    self
      .derive_open(move || open.get())
      .handle_dismiss(move || open.set(false))
  }

  /// Runs `dismiss` when the reader dismisses the open overlay: a press outside it and its
  /// anchor, or the platform's own dismissal.
  pub fn handle_dismiss(mut self, dismiss: impl Fn() + 'static) -> Self {
    self.dismiss = Some(Rc::new(dismiss));

    self
  }

  /// What it shows, last.
  pub fn set_child(mut self, child: impl View) -> Self {
    self.child = Some(child_of::<Self>(child));

    self
  }
}

impl View for Overlay {
  fn build(self, cx: &mut Cx) -> Node {
    let Self {
      anchor,
      side,
      widen,
      open,
      dismiss,
      child,
    } = self;
    let id = cx.create(NodeSpec::new(NodeKind::Overlay));

    finish(id, Record::new(), cx, None);
    tree::set_hidden::<Active>(id, true);
    tree::bind::<Active, _>(id, contents::PLACEMENT, move || OverlayPlacement {
      anchor: anchor.and_then(|anchor| anchor.node()),
      side,
      widen,
    });

    if let Some(dismiss) = dismiss {
      tree::handle_dismiss::<Active>(id, move || dismiss());
    }

    let owner = Owner::new();
    let inner = cx.child();
    let pending = Cell::new(child);
    let shown = Cell::new(false);

    Effect::new(move || {
      let open = open();

      untrack(|| {
        if open && let Some(child) = pending.take() {
          let node = owner.run(|| child(&mut inner.child()));

          tree::attach::<Active>(id, node.0, 0);
        }

        if shown.replace(open) != open {
          tree::set_hidden::<Active>(id, !open);
        }
      });
    });

    Node(id)
  }
}
