//! Event dispatch over the scene: one bubbling implementation for every platform.
//!
//! A platform reports an event at a node. A preventable event climbs from that node
//! through the scene's parents, running every handler on each node deepest first and
//! folding their verdicts into a [`Prevented`], which tells the platform what the
//! platform may still do; the climb stops at the node whose handlers prevented
//! bubbling. A pointer event reaches each node local to that node's own box, measured
//! by the platform. A notification runs the node's own handlers only. Every dispatch
//! runs in one batch, and no borrow of the scene is held while a handler runs.

use std::rc::Rc;

use skene_reactive::batch;
use smallvec::SmallVec;

use super::{HandlerSet, NodeId, Platform, Scene};
use crate::{KeyEvent, PointerEvent, Prevent, Prevented, ScrollEvent, TextSelection};

/// Dispatches a click at `node`.
#[inline(always)]
pub fn dispatch_click<P: Platform>(node: NodeId) -> Prevented {
  bubble::<P, _, _>(
    node,
    |set, out| {
      out.extend(set.acting_click().iter().cloned());
      out.extend(set.click().iter().cloned());
    },
    |_| (),
    |handler, ()| handler(),
  )
}

/// Dispatches a click the platform already acted on at `node`: every click handler
/// up the path runs except the acting ones, so a control's observers see the click
/// and its behavior does not act a second time.
#[inline(always)]
pub fn dispatch_click_observed<P: Platform>(node: NodeId) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(HandlerSet::click),
    |_| (),
    |handler, ()| handler(),
  )
}

/// Dispatches a key press at `node`, the focused node.
#[inline(always)]
pub fn dispatch_key_down<P: Platform>(node: NodeId, event: &KeyEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(HandlerSet::key_down),
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches a key release at `node`, the focused node.
#[inline(always)]
pub fn dispatch_key_up<P: Platform>(node: NodeId, event: &KeyEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(HandlerSet::key_up),
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches a pointer press at `node`.
#[inline(always)]
pub fn dispatch_pointer_down<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(HandlerSet::pointer_down),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// Dispatches a pointer release at `node`.
#[inline(always)]
pub fn dispatch_pointer_up<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(HandlerSet::pointer_up),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// Dispatches a pointer move at `node`.
#[inline(always)]
pub fn dispatch_pointer_move<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(HandlerSet::pointer_move),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// `event` local to `node`'s box, as the platform measures it.
fn localize<P: Platform>(node: NodeId, event: &PointerEvent) -> PointerEvent {
  let sink = Scene::<P>::with(|scene| scene.sink(node));

  sink.map_or(*event, |sink| P::default().localize_pointer(&sink, event))
}

/// Notifies `node` that the pointer entered it.
#[inline(always)]
pub fn notify_pointer_enter<P: Platform>(node: NodeId) {
  notify::<P, _>(node, all(HandlerSet::pointer_enter), |handler| handler());
}

/// Notifies `node` that the pointer left it.
#[inline(always)]
pub fn notify_pointer_leave<P: Platform>(node: NodeId) {
  notify::<P, _>(node, all(HandlerSet::pointer_leave), |handler| handler());
}

/// Notifies `node` that it gained focus.
#[inline(always)]
pub fn notify_focus<P: Platform>(node: NodeId) {
  notify::<P, _>(node, all(HandlerSet::focus), |handler| handler());
}

/// Notifies `node` that it lost focus.
#[inline(always)]
pub fn notify_blur<P: Platform>(node: NodeId) {
  notify::<P, _>(node, all(HandlerSet::blur), |handler| handler());
}

/// Notifies `node` of its new scroll offsets.
#[inline(always)]
pub fn notify_scroll<P: Platform>(node: NodeId, event: &ScrollEvent) {
  notify::<P, _>(node, all(HandlerSet::scroll), |handler| handler(event));
}

/// Notifies `node` of its new textual value.
#[inline(always)]
pub fn notify_change<P: Platform>(node: NodeId, value: &str) {
  notify::<P, _>(node, all(HandlerSet::change), |handler| handler(value));
}

/// Notifies `node` of its new selection.
#[inline(always)]
pub fn notify_selection_change<P: Platform>(node: NodeId, selection: TextSelection) {
  notify::<P, _>(node, all(HandlerSet::selection_change), |handler| {
    handler(selection);
  });
}

/// Notifies `node` of its new checked state.
#[inline(always)]
pub fn notify_toggle<P: Platform>(node: NodeId, checked: bool) {
  notify::<P, _>(node, all(HandlerSet::toggle), |handler| handler(checked));
}

/// Notifies the overlay `node` that the platform dismissed it.
#[inline(always)]
pub fn notify_dismiss<P: Platform>(node: NodeId) {
  notify::<P, _>(node, all(HandlerSet::dismiss), |handler| handler());
}

/// Dispatches a context menu request at `node`: `event` is where the menu is asked
/// for, the secondary button on a press and no button from the keyboard.
#[inline(always)]
pub fn dispatch_context_menu<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(HandlerSet::context_menu),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// The handlers a dispatch runs on one node, in order.
type Selected<H> = SmallVec<[Rc<H>; 2]>;

/// A selection taking every handler of one list.
fn all<H: ?Sized>(list: fn(&HandlerSet) -> &[Rc<H>]) -> impl Fn(&HandlerSet, &mut Selected<H>) {
  move |set, out| out.extend(list(set).iter().cloned())
}

/// The handlers `select` picks on one node, cloned out so the scene is released
/// before any runs; with the node's parent.
fn snapshot<P: Platform, H: ?Sized>(
  node: NodeId,
  select: impl Fn(&HandlerSet, &mut Selected<H>),
) -> (Selected<H>, Option<NodeId>) {
  Scene::<P>::with(|scene| {
    let mut handlers = Selected::new();

    if let Some(set) = scene.handlers(node) {
      select(set, &mut handlers);
    }

    (handlers, scene.parent(node))
  })
}

/// Climbs from `node`, running the handlers `select` picks on each node with the
/// payload `prepare` makes for that node, made only where a handler runs.
fn bubble<P: Platform, H: ?Sized, E>(
  node: NodeId,
  select: impl Fn(&HandlerSet, &mut Selected<H>),
  prepare: impl Fn(NodeId) -> E,
  run: impl Fn(&H, &E) -> Prevent,
) -> Prevented {
  batch(|| {
    let mut prevented = Prevented::default();
    let mut current = Some(node);

    while let Some(node) = current {
      let (handlers, parent) = snapshot::<P, H>(node, &select);

      if !handlers.is_empty() {
        let payload = prepare(node);

        for handler in &handlers {
          prevented.absorb(run(handler, &payload));
        }
      }

      if prevented.bubbling {
        break;
      }

      current = parent;
    }

    prevented
  })
}

fn notify<P: Platform, H: ?Sized>(
  node: NodeId,
  select: impl Fn(&HandlerSet, &mut Selected<H>),
  run: impl Fn(&H),
) {
  batch(|| {
    let (handlers, _) = snapshot::<P, H>(node, select);

    for handler in &handlers {
      run(handler);
    }
  });
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::{dispatch_click, dispatch_click_observed, dispatch_pointer_down, notify_focus};
  use crate::{
    Bubbling,
    EventKind,
    Modifiers,
    NodeId,
    PointerButton,
    PointerEvent,
    PointerHandler,
    Prevent,
    Prevented,
    Scene,
    testing::{Call, TestBox, TestPlatform, sink},
  };

  fn chain() -> (NodeId, NodeId, NodeId) {
    let outer = TestBox::node();
    let middle = TestBox::node();
    let inner = TestBox::node();

    Scene::<TestPlatform>::with(|scene| {
      scene.attach(outer, middle, None);
      scene.attach(middle, inner, None);
    });
    TestPlatform::take_log();

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
  /// reported to the platform.
  #[test]
  fn click_runs_deepest_first() {
    TestPlatform::reset();

    let (outer, middle, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestPlatform>::with(|scene| {
      scene.handle_click(outer, record(&log, "outer"));
      scene.handle_click(inner, record(&log, "inner first"));
      scene.handle_click(inner, record(&log, "inner second"));
    });

    assert_eq!(
      TestPlatform::take_log(),
      [
        Call::Listen(sink(outer), EventKind::Click),
        Call::Listen(sink(inner), EventKind::Click),
        Call::Listen(sink(inner), EventKind::Click),
      ]
    );
    assert_eq!(dispatch_click::<TestPlatform>(inner), Prevented::default());
    assert_eq!(*log.borrow(), ["inner first", "inner second", "outer"]);
    assert_eq!(dispatch_click::<TestPlatform>(middle), Prevented::default());
    assert_eq!(
      *log.borrow(),
      ["inner first", "inner second", "outer", "outer"]
    );
  }

  /// A box that stops bubbling keeps the click from its ancestors, and its verdict
  /// folds into the result.
  #[test]
  fn bubbling_stop_ends_the_climb() {
    TestPlatform::reset();

    let (outer, middle, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestPlatform>::with(|scene| {
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
      dispatch_click::<TestPlatform>(inner),
      Prevented {
        platform: false,
        bubbling: true,
      }
    );
    assert_eq!(*log.borrow(), ["inner", "middle"]);
  }

  /// The `Prevent::Platform` verdict is the control's own: it suppresses the native response
  /// without stopping the ancestors, and a handler may reach the scene while it runs.
  #[test]
  fn platform_verdict_is_the_controls() {
    TestPlatform::reset();

    let (outer, _, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestPlatform>::with(|scene| {
      scene.handle_click(outer, record(&log, "outer"));

      let control = Rc::clone(&log);

      scene.handle_click(
        inner,
        Rc::new(move || {
          let parent = Scene::<TestPlatform>::with(|scene| scene.parent(inner));

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
      dispatch_click::<TestPlatform>(inner),
      Prevented {
        platform: true,
        bubbling: false,
      }
    );
    assert_eq!(*log.borrow(), ["control", "outer"]);
  }

  /// An acting handler runs before the node's observers on a click, and an observed
  /// click skips it while the observers and the ancestors still run.
  #[test]
  fn observed_click_skips_the_acting_handler() {
    TestPlatform::reset();

    let (outer, _, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestPlatform>::with(|scene| {
      scene.handle_click(inner, record(&log, "observer"));
      scene.handle_click_acting(inner, record(&log, "acting"));
      scene.handle_click(outer, record(&log, "outer"));
    });

    assert_eq!(dispatch_click::<TestPlatform>(inner), Prevented::default());
    assert_eq!(*log.borrow(), ["acting", "observer", "outer"]);

    log.borrow_mut().clear();

    assert_eq!(
      dispatch_click_observed::<TestPlatform>(inner),
      Prevented::default()
    );
    assert_eq!(*log.borrow(), ["observer", "outer"]);
  }

  /// A notification runs the node's own handlers only.
  #[test]
  fn notifications_do_not_bubble() {
    TestPlatform::reset();

    let (outer, _, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestPlatform>::with(|scene| {
      let outer_log = Rc::clone(&log);
      let inner_log = Rc::clone(&log);

      scene.handle_focus(outer, Rc::new(move || outer_log.borrow_mut().push("outer")));
      scene.handle_focus(inner, Rc::new(move || inner_log.borrow_mut().push("inner")));
    });

    notify_focus::<TestPlatform>(inner);

    assert_eq!(*log.borrow(), ["inner"]);
  }

  /// A pointer event reaches each handling node local to that node's own box.
  #[test]
  fn pointer_events_localize_per_node() {
    TestPlatform::reset();

    let (outer, _, inner) = chain();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let observe = |name: &'static str| -> Rc<PointerHandler> {
      let seen = Rc::clone(&seen);

      Rc::new(move |event: &PointerEvent| {
        seen.borrow_mut().push((name, event.local_x, event.local_y));

        Prevent::None
      })
    };

    Scene::<TestPlatform>::with(|scene| {
      scene.handle_pointer_down(outer, observe("outer"));
      scene.handle_pointer_down(inner, observe("inner"));
    });
    TestPlatform::set_origin(sink(outer), 10., 100.);
    TestPlatform::set_origin(sink(inner), 30., 120.);

    let event = PointerEvent {
      x: 35.,
      y: 125.,
      local_x: 5.,
      local_y: 5.,
      button: Some(PointerButton::Primary),
      modifiers: Modifiers::default(),
    };
    let _ = dispatch_pointer_down::<TestPlatform>(inner, &event);

    assert_eq!(*seen.borrow(), [("inner", 5., 5.), ("outer", 25., 25.)]);
  }
}
