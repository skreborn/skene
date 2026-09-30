//! Event dispatch over the scene: one bubbling implementation for every platform.
//!
//! A platform reports an event at a node. A preventable event climbs from that node
//! through the scene's parents as they were when the dispatch began, running every
//! handler on each node deepest first and folding their verdicts into a
//! [`Prevented`], which tells the platform what the platform may still do; the climb
//! stops at the node whose handlers prevented bubbling. A pointer event reaches each
//! node local to that node's own box, measured by the platform. A notification runs the
//! node's own handlers only, and a focus move notifies each node whose subtree it
//! enters or leaves. Every dispatch runs in one batch, and no borrow of the scene is
//! held while a handler runs.

use skene_reactive::batch;
use smallvec::SmallVec;

use super::{Handler, NodeId, Platform, Scene};
use crate::{
  ClickEvent,
  KeyEvent,
  PlainHandler,
  PointerEvent,
  Prevent,
  Prevented,
  ScrollEvent,
  TextSelection,
  WheelEvent,
  handler::{HandlerSet, Pick, Registered},
  task::run_handler,
};

/// Dispatches a click at `node`.
#[inline(always)]
pub fn dispatch_click<P: Platform>(node: NodeId, event: &ClickEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    |set, out| {
      out.extend(set.select(|handler| match handler {
        Handler::ActingClick(run) => Some(run),
        _ => None,
      }));
      out.extend(set.select(|handler| match handler {
        Handler::Click(run) => Some(run),
        _ => None,
      }));

      false
    },
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches the second click of a double click at `node`, after the click itself.
#[inline(always)]
pub fn dispatch_double_click<P: Platform>(node: NodeId, event: &ClickEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::DoubleClick(run) => Some(run),
      _ => None,
    }),
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches a click the platform already acted on at `node`.
///
/// Every click handler up the path runs except the acting ones, and the climb ends at
/// the first node carrying acting ones, as a control consumes the click it acts on; so
/// a control's observers see the click and its behavior does not act a second time.
#[inline(always)]
pub fn dispatch_click_observed<P: Platform>(node: NodeId, event: &ClickEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    |set, out| {
      out.extend(set.select(|handler| match handler {
        Handler::Click(run) => Some(run),
        _ => None,
      }));

      set.has(|handler| match handler {
        Handler::ActingClick(run) => Some(run),
        _ => None,
      })
    },
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches a key press at `node`, the focused node.
#[inline(always)]
pub fn dispatch_key_down<P: Platform>(node: NodeId, event: &KeyEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::KeyDown(run) => Some(run),
      _ => None,
    }),
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches a key release at `node`, the focused node.
#[inline(always)]
pub fn dispatch_key_up<P: Platform>(node: NodeId, event: &KeyEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::KeyUp(run) => Some(run),
      _ => None,
    }),
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches a pointer press at `node`.
#[inline(always)]
pub fn dispatch_pointer_down<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::PointerDown(run) => Some(run),
      _ => None,
    }),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// Dispatches the cancel of a pointer the platform took for itself at `node`, the node
/// its press reached.
#[inline(always)]
pub fn dispatch_pointer_cancel<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::PointerCancel(run) => Some(run),
      _ => None,
    }),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// Dispatches a wheel's turn at `node`, the deepest node under the pointer, before the
/// platform scrolls by it.
#[inline(always)]
pub fn dispatch_wheel<P: Platform>(node: NodeId, event: &WheelEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::Wheel(run) => Some(run),
      _ => None,
    }),
    |at| {
      let local = localize::<P>(at, &event.position());

      WheelEvent {
        local_x: local.local_x,
        local_y: local.local_y,
        ..*event
      }
    },
    |handler, event| handler(event),
  )
}

/// Dispatches a pointer release at `node`.
#[inline(always)]
pub fn dispatch_pointer_up<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::PointerUp(run) => Some(run),
      _ => None,
    }),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// Dispatches a pointer move at `node`.
#[inline(always)]
pub fn dispatch_pointer_move<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::PointerMove(run) => Some(run),
      _ => None,
    }),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// `event` local to `node`'s box, as the platform measures it.
fn localize<P: Platform>(node: NodeId, event: &PointerEvent) -> PointerEvent {
  let sink = Scene::<P>::with(|scene| scene.sink(node));

  sink.map_or(*event, |sink| P::localize_pointer(sink, event))
}

/// Moves the scene's pointer over `next`, the deepest node under it.
///
/// Each node the pointer left is notified, innermost first, then each it entered,
/// outermost first, as `pointerleave` and `pointerenter` reach every node whose subtree
/// the pointer crossed.
pub fn notify_pointer_moved<P: Platform>(next: Option<NodeId>) {
  let (left, entered) = Scene::<P>::with(|scene| scene.move_pointer(next));
  let leave: Pick<PlainHandler> = |handler| match handler {
    Handler::PointerLeave(run) => Some(run),
    _ => None,
  };
  let enter: Pick<PlainHandler> = |handler| match handler {
    Handler::PointerEnter(run) => Some(run),
    _ => None,
  };

  batch(|| {
    for node in left {
      notify::<P, _>(node, leave, |handler| handler());
    }

    for node in entered {
      notify::<P, _>(node, enter, |handler| handler());
    }
  });
}

/// Moves the scene's focus to `next`, or off every node for `None`.
///
/// Each node whose subtree held focus and no longer does is blurred, innermost first,
/// then each whose subtree holds it and did not is focused, outermost first, as
/// `:focus-within` changes.
pub fn notify_focus_moved<P: Platform>(next: Option<NodeId>) {
  let (left, reached) = Scene::<P>::with(|scene| scene.move_focus(next));
  let blur: Pick<PlainHandler> = |handler| match handler {
    Handler::Blur(run) => Some(run),
    _ => None,
  };
  let focus: Pick<PlainHandler> = |handler| match handler {
    Handler::Focus(run) => Some(run),
    _ => None,
  };

  batch(|| {
    for node in left {
      notify::<P, _>(node, blur, |handler| handler());
    }

    for node in reached {
      notify::<P, _>(node, focus, |handler| handler());
    }
  });
}

/// Notifies `node` of its new scroll offsets.
#[inline(always)]
pub fn notify_scroll<P: Platform>(node: NodeId, event: &ScrollEvent) {
  notify::<P, _>(
    node,
    |handler| match handler {
      Handler::Scroll(run) => Some(run),
      _ => None,
    },
    |handler| handler(event),
  );
}

/// Notifies `node` of its new textual value.
#[inline(always)]
pub fn notify_change<P: Platform>(node: NodeId, value: &str) {
  notify::<P, _>(
    node,
    |handler| match handler {
      Handler::Change(run) => Some(run),
      _ => None,
    },
    |handler| handler(value),
  );
}

/// Notifies `node` of its new selection.
#[inline(always)]
pub fn notify_selection_change<P: Platform>(node: NodeId, selection: TextSelection) {
  notify::<P, _>(
    node,
    |handler| match handler {
      Handler::SelectionChange(run) => Some(run),
      _ => None,
    },
    |handler| handler(selection),
  );
}

/// Notifies `node` of its new checked state.
#[inline(always)]
pub fn notify_toggle<P: Platform>(node: NodeId, checked: bool) {
  notify::<P, _>(
    node,
    |handler| match handler {
      Handler::Toggle(run) => Some(run),
      _ => None,
    },
    |handler| handler(checked),
  );
}

/// Notifies the overlay `node` that the platform dismissed it.
#[inline(always)]
pub fn notify_dismiss<P: Platform>(node: NodeId) {
  notify::<P, _>(
    node,
    |handler| match handler {
      Handler::Dismiss(run) => Some(run),
      _ => None,
    },
    |handler| handler(),
  );
}

/// Dispatches a context menu request at `node`: `event` is where the menu is asked
/// for, the secondary button on a press and no button from the keyboard.
#[inline(always)]
pub fn dispatch_context_menu<P: Platform>(node: NodeId, event: &PointerEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    all(|handler| match handler {
      Handler::ContextMenu(run) => Some(run),
      _ => None,
    }),
    |at| localize::<P>(at, event),
    |handler, event| handler(event),
  )
}

/// The handlers a dispatch runs on one node, in order.
type Selected<H> = SmallVec<[Registered<H>; 2]>;

/// A selection taking every handler `pick` takes, ending the climb nowhere.
fn all<H: ?Sized>(pick: Pick<H>) -> impl Fn(&HandlerSet, &mut Selected<H>) -> bool {
  move |set, out| {
    out.extend(set.select(pick));

    false
  }
}

/// The handlers `select` picks on one node, cloned out so the scene is released
/// before any runs; with whether `select` ends the climb at the node.
fn snapshot<P: Platform, H: ?Sized>(
  node: NodeId,
  select: impl Fn(&HandlerSet, &mut Selected<H>) -> bool,
) -> (Selected<H>, bool) {
  Scene::<P>::with(|scene| {
    let mut handlers = Selected::new();
    let ends = scene
      .handlers(node)
      .is_some_and(|set| select(set, &mut handlers));

    (handlers, ends)
  })
}

/// Climbs from `node` along the path it has before any handler runs, running the
/// handlers `select` picks on each node with the payload `prepare` makes for that
/// node, made only where a handler runs.
fn bubble<P: Platform, H: ?Sized, E>(
  node: NodeId,
  select: impl Fn(&HandlerSet, &mut Selected<H>) -> bool,
  prepare: impl Fn(NodeId) -> E,
  run: impl Fn(&H, &E) -> Prevent,
) -> Prevented {
  batch(|| {
    let mut prevented = Prevented::default();

    for node in Scene::<P>::with(|scene| scene.ancestry(node)) {
      let (handlers, ends) = snapshot::<P, H>(node, &select);

      if !handlers.is_empty() {
        let payload = prepare(node);

        for handler in &handlers {
          prevented.absorb(run_handler(handler.owner(), || run(handler, &payload)));
        }
      }

      if prevented.bubbling || ends {
        break;
      }
    }

    prevented
  })
}

/// Runs the handlers `pick` takes on `node` alone.
fn notify<P: Platform, H: ?Sized>(node: NodeId, pick: Pick<H>, run: impl Fn(&H)) {
  batch(|| {
    let (handlers, _) = snapshot::<P, H>(node, all(pick));

    for handler in &handlers {
      run_handler(handler.owner(), || run(handler));
    }
  });
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use skene_reactive::{Owner, provide_context, use_context};

  use super::{
    dispatch_click,
    dispatch_click_observed,
    dispatch_pointer_down,
    notify_dismiss,
    notify_focus_moved,
    notify_pointer_moved,
  };
  use crate::{
    Boundary,
    Bubbling,
    ClickEvent,
    ClickHandler,
    EventKind,
    Handler,
    NodeId,
    PointerButton,
    PointerEvent,
    PointerHandler,
    Prevent,
    Prevented,
    Scene,
    count_pending,
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

  fn record(log: &Rc<RefCell<Vec<&'static str>>>, name: &'static str) -> Rc<ClickHandler> {
    let log = Rc::clone(log);

    Rc::new(move |_: &ClickEvent| {
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
      scene.handle(outer, Handler::Click(record(&log, "outer")));
      scene.handle(inner, Handler::Click(record(&log, "inner first")));
      scene.handle(inner, Handler::Click(record(&log, "inner second")));
    });

    assert_eq!(
      TestPlatform::take_log(),
      [
        Call::Listen(sink(outer), EventKind::Click),
        Call::Listen(sink(inner), EventKind::Click),
        Call::Listen(sink(inner), EventKind::Click),
      ]
    );
    assert_eq!(
      dispatch_click::<TestPlatform>(inner, &ClickEvent::new(1)),
      Prevented::default()
    );
    assert_eq!(*log.borrow(), ["inner first", "inner second", "outer"]);
    assert_eq!(
      dispatch_click::<TestPlatform>(middle, &ClickEvent::new(1)),
      Prevented::default()
    );
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
      scene.handle(outer, Handler::Click(record(&log, "outer")));
      scene.handle(inner, Handler::Click(record(&log, "inner")));

      let stop = Rc::clone(&log);

      scene.handle(
        middle,
        Handler::Click(Rc::new(move |_: &ClickEvent| {
          stop.borrow_mut().push("middle");

          Bubbling::Stop.into()
        })),
      );
    });

    assert_eq!(
      dispatch_click::<TestPlatform>(inner, &ClickEvent::new(1)),
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
      scene.handle(outer, Handler::Click(record(&log, "outer")));

      let control = Rc::clone(&log);

      scene.handle(
        inner,
        Handler::Click(Rc::new(move |_: &ClickEvent| {
          let parent = Scene::<TestPlatform>::with(|scene| scene.parent(inner));

          control.borrow_mut().push(if parent.is_some() {
            "control"
          }
          else {
            "orphan"
          });

          Prevent::Platform
        })),
      );
    });

    assert_eq!(
      dispatch_click::<TestPlatform>(inner, &ClickEvent::new(1)),
      Prevented {
        platform: true,
        bubbling: false,
      }
    );
    assert_eq!(*log.borrow(), ["control", "outer"]);
  }

  /// An acting handler runs before the node's observers on a click, and an observed
  /// click skips it while the observers beneath and on its node still run, the climb
  /// ending there as the acting handler's verdict ends it.
  #[test]
  fn observed_click_skips_the_acting_handler() {
    TestPlatform::reset();

    let (outer, middle, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));
    let acting = {
      let log = Rc::clone(&log);

      Rc::new(move |_: &ClickEvent| {
        log.borrow_mut().push("acting");

        Prevent::Bubbling
      })
    };

    Scene::<TestPlatform>::with(|scene| {
      scene.handle(inner, Handler::Click(record(&log, "inner")));
      scene.handle(middle, Handler::Click(record(&log, "observer")));
      scene.handle(middle, Handler::ActingClick(acting));
      scene.handle(outer, Handler::Click(record(&log, "outer")));
    });

    let _ = dispatch_click::<TestPlatform>(inner, &ClickEvent::new(1));

    assert_eq!(*log.borrow(), ["inner", "acting", "observer"]);

    log.borrow_mut().clear();

    let _ = dispatch_click_observed::<TestPlatform>(inner, &ClickEvent::new(1));

    assert_eq!(*log.borrow(), ["inner", "observer"]);
  }

  /// The climb follows the parents a node had when the dispatch began, whatever a
  /// handler changes on the way.
  #[test]
  fn the_path_is_fixed_before_any_handler_runs() {
    TestPlatform::reset();

    let (outer, middle, inner) = chain();
    let log = Rc::new(RefCell::new(Vec::new()));

    Scene::<TestPlatform>::with(|scene| {
      scene.handle(
        inner,
        Handler::Click(Rc::new(move |_: &ClickEvent| {
          Scene::<TestPlatform>::with(|scene| scene.detach(outer, middle));

          Prevent::None
        })),
      );
      scene.handle(outer, Handler::Click(record(&log, "outer")));
    });

    let _ = dispatch_click::<TestPlatform>(inner, &ClickEvent::new(1));

    assert_eq!(*log.borrow(), ["outer"]);
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

      scene.handle(
        outer,
        Handler::Dismiss(Rc::new(move || outer_log.borrow_mut().push("outer"))),
      );
      scene.handle(
        inner,
        Handler::Dismiss(Rc::new(move || inner_log.borrow_mut().push("inner"))),
      );
    });

    notify_dismiss::<TestPlatform>(inner);

    assert_eq!(*log.borrow(), ["inner"]);
  }

  /// A pointer move leaves each node whose subtree the pointer left, innermost first,
  /// then enters each whose subtree it reached, outermost first; a node holding the
  /// pointer throughout hears nothing.
  #[test]
  fn a_pointer_move_notifies_the_subtrees_it_crosses() {
    TestPlatform::reset();

    let (outer, middle, inner) = chain();
    let sibling = TestBox::node();
    let log = Rc::new(RefCell::new(Vec::new()));
    let note = |name: &'static str| -> Rc<dyn Fn()> {
      let log = Rc::clone(&log);

      Rc::new(move || log.borrow_mut().push(name))
    };

    Scene::<TestPlatform>::with(|scene| {
      scene.attach(outer, sibling, None);

      for (node, enter, leave) in [
        (outer, "outer enter", "outer leave"),
        (middle, "middle enter", "middle leave"),
        (inner, "inner enter", "inner leave"),
        (sibling, "sibling enter", "sibling leave"),
      ] {
        scene.handle(node, Handler::PointerEnter(note(enter)));
        scene.handle(node, Handler::PointerLeave(note(leave)));
      }
    });

    notify_pointer_moved::<TestPlatform>(Some(inner));

    assert_eq!(
      std::mem::take(&mut *log.borrow_mut()),
      ["outer enter", "middle enter", "inner enter"]
    );

    notify_pointer_moved::<TestPlatform>(Some(sibling));

    assert_eq!(
      std::mem::take(&mut *log.borrow_mut()),
      ["inner leave", "middle leave", "sibling enter"]
    );
    assert!(Scene::<TestPlatform>::with(|scene| scene.is_hovered(outer)));

    notify_pointer_moved::<TestPlatform>(None);

    assert_eq!(*log.borrow(), ["sibling leave", "outer leave"]);
  }

  /// A focus move blurs each node whose subtree focus left, innermost first, then
  /// focuses each whose subtree it reached, outermost first; a node holding it
  /// throughout hears nothing.
  #[test]
  fn a_focus_move_notifies_the_subtrees_it_crosses() {
    TestPlatform::reset();

    let (outer, middle, inner) = chain();
    let sibling = TestBox::node();
    let log = Rc::new(RefCell::new(Vec::new()));
    let note = |name: &'static str| -> Rc<dyn Fn()> {
      let log = Rc::clone(&log);

      Rc::new(move || log.borrow_mut().push(name))
    };

    Scene::<TestPlatform>::with(|scene| {
      scene.attach(outer, sibling, None);

      for (node, focus, blur) in [
        (outer, "outer focus", "outer blur"),
        (middle, "middle focus", "middle blur"),
        (inner, "inner focus", "inner blur"),
        (sibling, "sibling focus", "sibling blur"),
      ] {
        scene.handle(node, Handler::Focus(note(focus)));
        scene.handle(node, Handler::Blur(note(blur)));
      }
    });

    notify_focus_moved::<TestPlatform>(Some(inner));

    assert_eq!(
      std::mem::take(&mut *log.borrow_mut()),
      ["outer focus", "middle focus", "inner focus"]
    );

    notify_focus_moved::<TestPlatform>(Some(sibling));

    assert_eq!(
      std::mem::take(&mut *log.borrow_mut()),
      ["inner blur", "middle blur", "sibling focus"]
    );

    notify_focus_moved::<TestPlatform>(None);

    assert_eq!(*log.borrow(), ["sibling blur", "outer blur"]);
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
      scene.handle(outer, Handler::PointerDown(observe("outer")));
      scene.handle(inner, Handler::PointerDown(observe("inner")));
    });
    TestPlatform::set_origin(sink(outer), 10., 100.);
    TestPlatform::set_origin(sink(inner), 30., 120.);

    let event = PointerEvent {
      local_x: 5.,
      local_y: 5.,
      ..PointerEvent::mouse(35., 125.)
    }
    .with_press(PointerButton::Primary);
    let _ = dispatch_pointer_down::<TestPlatform>(inner, &event);

    assert_eq!(*seen.borrow(), [("inner", 5., 5.), ("outer", 25., 25.)]);
  }

  /// A handler runs as the owner that registered it, so it sees that owner's context
  /// and what it creates ends with that owner.
  #[test]
  fn a_handler_runs_as_the_owner_that_registered_it() {
    TestPlatform::reset();

    let (_, _, inner) = chain();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner::new();

    owner.enter(|| {
      provide_context(7u32);

      let seen = Rc::clone(&seen);

      Scene::<TestPlatform>::with(|scene| {
        scene.handle(
          inner,
          Handler::Click(Rc::new(move |_: &ClickEvent| {
            seen
              .borrow_mut()
              .push((use_context::<u32>(), Owner::current() == owner));

            Prevent::None
          })),
        );
      });
    });

    assert_eq!(
      dispatch_click::<TestPlatform>(inner, &ClickEvent::new(1)),
      Prevented::default()
    );
    assert_eq!(*seen.borrow(), [(Some(7), true)]);
  }

  /// Work a handler starts is an action, counted toward no boundary.
  #[test]
  fn a_task_a_handler_starts_counts_toward_no_boundary() {
    TestPlatform::reset();

    let (_, _, inner) = chain();
    let boundary = Boundary::new();
    let owner = Owner::new();
    let tasks = Rc::new(RefCell::new(Vec::new()));

    owner.enter(|| {
      provide_context(boundary);

      let tasks = Rc::clone(&tasks);

      Scene::<TestPlatform>::with(|scene| {
        scene.handle(
          inner,
          Handler::Click(Rc::new(move |_: &ClickEvent| {
            tasks.borrow_mut().push(count_pending(Box::new(())));

            Prevent::None
          })),
        );
      });
    });

    assert_eq!(
      dispatch_click::<TestPlatform>(inner, &ClickEvent::new(1)),
      Prevented::default()
    );
    assert_eq!(tasks.borrow().len(), 1);
    assert!(boundary.is_settled());
  }
}
