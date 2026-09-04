//! Event dispatch over the scene: one bubbling implementation for every driver.
//!
//! A driver reports an event at a node. A preventable event climbs from that node
//! through the scene's parents, running every handler on each node deepest first and
//! folding their verdicts into a [`Prevented`], which tells the driver what the
//! platform may still do; the climb stops at the node whose handlers prevented
//! bubbling. A notification runs the node's own handlers only. Every dispatch runs in
//! one batch, and no borrow of the scene is held while a handler runs.

use std::rc::Rc;

use skene_reactive::batch;
use smallvec::SmallVec;

use super::{Driver, HandlerSet, NodeId, Scene};
use crate::{KeyEvent, PointerEvent, Prevent, Prevented, ScrollEvent};

/// Dispatches a click at `node`.
#[inline(always)]
pub fn dispatch_click<D: Driver>(node: NodeId) -> Prevented {
  bubble::<D, _>(node, HandlerSet::click, |handler| handler())
}

/// Dispatches a key press at `node`, the focused node.
#[inline(always)]
pub fn dispatch_key_down<D: Driver>(node: NodeId, event: &KeyEvent) -> Prevented {
  bubble::<D, _>(node, HandlerSet::key_down, |handler| handler(event))
}

/// Dispatches a pointer press at `node`.
#[inline(always)]
pub fn dispatch_pointer_down<D: Driver>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<D, _>(node, HandlerSet::pointer_down, |handler| handler(event))
}

/// Dispatches a pointer release at `node`.
#[inline(always)]
pub fn dispatch_pointer_up<D: Driver>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<D, _>(node, HandlerSet::pointer_up, |handler| handler(event))
}

/// Dispatches a pointer move at `node`.
#[inline(always)]
pub fn dispatch_pointer_move<D: Driver>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<D, _>(node, HandlerSet::pointer_move, |handler| handler(event))
}

/// Notifies `node` that the pointer entered it.
#[inline(always)]
pub fn notify_pointer_enter<D: Driver>(node: NodeId) {
  notify::<D, _>(node, HandlerSet::pointer_enter, |handler| handler());
}

/// Notifies `node` that the pointer left it.
#[inline(always)]
pub fn notify_pointer_leave<D: Driver>(node: NodeId) {
  notify::<D, _>(node, HandlerSet::pointer_leave, |handler| handler());
}

/// Notifies `node` that it gained focus.
#[inline(always)]
pub fn notify_focus<D: Driver>(node: NodeId) {
  notify::<D, _>(node, HandlerSet::focus, |handler| handler());
}

/// Notifies `node` that it lost focus.
#[inline(always)]
pub fn notify_blur<D: Driver>(node: NodeId) {
  notify::<D, _>(node, HandlerSet::blur, |handler| handler());
}

/// Notifies `node` of its new scroll offsets.
#[inline(always)]
pub fn notify_scroll<D: Driver>(node: NodeId, event: &ScrollEvent) {
  notify::<D, _>(node, HandlerSet::scroll, |handler| handler(event));
}

/// Notifies `node` of its new textual value.
#[inline(always)]
pub fn notify_change<D: Driver>(node: NodeId, value: &str) {
  notify::<D, _>(node, HandlerSet::change, |handler| handler(value));
}

/// Notifies `node` of its new checked state.
#[inline(always)]
pub fn notify_toggle<D: Driver>(node: NodeId, checked: bool) {
  notify::<D, _>(node, HandlerSet::toggle, |handler| handler(checked));
}

/// Notifies the overlay `node` that the platform dismissed it.
#[inline(always)]
pub fn notify_dismiss<D: Driver>(node: NodeId) {
  notify::<D, _>(node, HandlerSet::dismiss, |handler| handler());
}

/// The handlers of one kind on one node, cloned out so the scene is released before
/// any runs; with the node's parent.
fn snapshot<D: Driver, H: ?Sized>(
  node: NodeId,
  select: impl Fn(&HandlerSet) -> &[Rc<H>],
) -> (SmallVec<[Rc<H>; 2]>, Option<NodeId>) {
  Scene::<D>::with(|scene| {
    let handlers = scene
      .handlers(node)
      .map(|set| select(set).iter().cloned().collect())
      .unwrap_or_default();

    (handlers, scene.parent(node))
  })
}

fn bubble<D: Driver, H: ?Sized>(
  node: NodeId,
  select: impl Fn(&HandlerSet) -> &[Rc<H>],
  run: impl Fn(&H) -> Prevent,
) -> Prevented {
  batch(|| {
    let mut prevented = Prevented::default();
    let mut current = Some(node);

    while let Some(node) = current {
      let (handlers, parent) = snapshot::<D, H>(node, &select);

      for handler in &handlers {
        prevented.absorb(run(handler));
      }

      if prevented.bubbling {
        break;
      }

      current = parent;
    }

    prevented
  })
}

fn notify<D: Driver, H: ?Sized>(
  node: NodeId,
  select: impl Fn(&HandlerSet) -> &[Rc<H>],
  run: impl Fn(&H),
) {
  batch(|| {
    let (handlers, _) = snapshot::<D, H>(node, select);

    for handler in &handlers {
      run(handler);
    }
  });
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::{dispatch_click, notify_focus};
  use crate::{
    Bubbling,
    Prevent,
    Prevented,
    next::{
      EventKind,
      NodeId,
      Scene,
      testing::{Call, TestBox, TestDriver, sink},
    },
  };

  fn chain() -> (NodeId, NodeId, NodeId) {
    let outer = TestBox::node();
    let middle = TestBox::node();
    let inner = TestBox::node();

    Scene::<TestDriver>::with(|scene| {
      scene.attach(outer, middle, None);
      scene.attach(middle, inner, None);
    });
    TestDriver::take_log();

    (outer, middle, inner)
  }

  fn record(log: &Rc<RefCell<Vec<&'static str>>>, name: &'static str) -> Rc<dyn Fn() -> Prevent> {
    let log = Rc::clone(log);

    Rc::new(move || {
      log.borrow_mut().push(name);

      Prevent::None
    })
  }

  /// Handlers run deepest first, every handler on a node runs, and registration is
  /// reported to the driver.
  #[test]
  fn click_runs_deepest_first() {
    TestDriver::reset();

    let (outer, middle, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestDriver>::with(|scene| {
      scene.handle_click(outer, record(&log, "outer"));
      scene.handle_click(inner, record(&log, "inner first"));
      scene.handle_click(inner, record(&log, "inner second"));
    });

    assert_eq!(
      TestDriver::take_log(),
      [
        Call::Listen(sink(outer), EventKind::Click),
        Call::Listen(sink(inner), EventKind::Click),
        Call::Listen(sink(inner), EventKind::Click),
      ]
    );
    assert_eq!(dispatch_click::<TestDriver>(inner), Prevented::default());
    assert_eq!(*log.borrow(), ["inner first", "inner second", "outer"]);
    assert_eq!(dispatch_click::<TestDriver>(middle), Prevented::default());
    assert_eq!(
      *log.borrow(),
      ["inner first", "inner second", "outer", "outer"]
    );
  }

  /// A box that stops bubbling keeps the click from its ancestors, and its verdict
  /// folds into the result.
  #[test]
  fn bubbling_stop_ends_the_climb() {
    TestDriver::reset();

    let (outer, middle, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestDriver>::with(|scene| {
      scene.handle_click(outer, record(&log, "outer"));
      scene.handle_click(inner, record(&log, "inner"));

      let stop = Rc::clone(&log);

      scene.handle_click(
        middle,
        Rc::new(move || {
          stop.borrow_mut().push("middle");

          Bubbling::Stop.into()
        }),
      );
    });

    assert_eq!(
      dispatch_click::<TestDriver>(inner),
      Prevented {
        platform: false,
        bubbling: true,
      }
    );
    assert_eq!(*log.borrow(), ["inner", "middle"]);
  }

  /// The platform verdict is the control's own: it suppresses the platform without
  /// stopping the ancestors, and a handler may reach the scene while it runs.
  #[test]
  fn platform_verdict_is_the_controls() {
    TestDriver::reset();

    let (outer, _, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestDriver>::with(|scene| {
      scene.handle_click(outer, record(&log, "outer"));

      let control = Rc::clone(&log);

      scene.handle_click(
        inner,
        Rc::new(move || {
          let parent = Scene::<TestDriver>::with(|scene| scene.parent(inner));

          control.borrow_mut().push(if parent.is_some() {
            "control"
          }
          else {
            "orphan"
          });

          Prevent::Platform
        }),
      );
    });

    assert_eq!(
      dispatch_click::<TestDriver>(inner),
      Prevented {
        platform: true,
        bubbling: false,
      }
    );
    assert_eq!(*log.borrow(), ["control", "outer"]);
  }

  /// A notification runs the node's own handlers only.
  #[test]
  fn notifications_do_not_bubble() {
    TestDriver::reset();

    let (outer, _, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestDriver>::with(|scene| {
      let outer_log = Rc::clone(&log);
      let inner_log = Rc::clone(&log);

      scene.handle_focus(outer, Rc::new(move || outer_log.borrow_mut().push("outer")));
      scene.handle_focus(inner, Rc::new(move || inner_log.borrow_mut().push("inner")));
    });

    notify_focus::<TestDriver>(inner);

    assert_eq!(*log.borrow(), ["inner"]);
  }
}
