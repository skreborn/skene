//! Event dispatch over the scene: one bubbling implementation for every platform.
//!
//! A platform reports an event at a node. A preventable event climbs from that node
//! through the scene's parents as they were when the dispatch began, running every
//! handler on each node deepest first and folding their verdicts into a
//! [`Prevented`], which tells the platform what the platform may still do; the climb
//! stops at the node whose handlers prevented bubbling. A pointer event reaches each
//! node local to that node's own box, measured by the platform. A notification runs the
//! node's own handlers only, and a focus move notifies each node whose subtree it
//! enters or leaves. Every dispatch runs in one batch, and no borrow of the scene is held
//! while a handler runs.

use skene_reactive::batch;
use smallvec::SmallVec;

use super::{Handler, NodeId, Realization, Scene};
use crate::spine::{
  ClickEvent,
  ClickHandler,
  KeyEvent,
  PlainHandler,
  PointerEvent,
  Prevent,
  Prevented,
  ScrollEvent,
  WheelEvent,
  handler::{HandlerSet, Pick, Registered},
  task::run_handler,
};

/// Dispatches a click at `node`.
pub fn dispatch_click<P: Realization>(node: NodeId, event: &ClickEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    HandlerSet::select_click,
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches the second click of a double click at `node`, after the click itself.
pub fn dispatch_double_click<P: Realization>(node: NodeId, event: &ClickEvent) -> Prevented {
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
/// the first node carrying acting ones.
pub fn dispatch_click_observed<P: Realization>(node: NodeId, event: &ClickEvent) -> Prevented {
  bubble::<P, _, _>(
    node,
    HandlerSet::select_observed_click,
    |_| (),
    |handler, ()| handler(event),
  )
}

/// Dispatches a key press at `node`, the focused node.
pub fn dispatch_key_down<P: Realization>(node: NodeId, event: &KeyEvent) -> Prevented {
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
pub fn dispatch_key_up<P: Realization>(node: NodeId, event: &KeyEvent) -> Prevented {
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
pub fn dispatch_pointer_down<P: Realization>(node: NodeId, event: &PointerEvent) -> Prevented {
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
pub fn dispatch_pointer_cancel<P: Realization>(node: NodeId, event: &PointerEvent) -> Prevented {
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
pub fn dispatch_wheel<P: Realization>(node: NodeId, event: &WheelEvent) -> Prevented {
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
pub fn dispatch_pointer_up<P: Realization>(node: NodeId, event: &PointerEvent) -> Prevented {
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
pub fn dispatch_pointer_move<P: Realization>(node: NodeId, event: &PointerEvent) -> Prevented {
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
fn localize<P: Realization>(node: NodeId, event: &PointerEvent) -> PointerEvent {
  P::localize_pointer(node, event)
}

/// Moves the scene's pointer over `next`, the deepest node under it.
///
/// Each node the pointer left is notified, innermost first, then each it entered,
/// outermost first, as `pointerleave` and `pointerenter` reach every node whose subtree
/// the pointer crossed.
pub fn notify_pointer_moved<P: Realization>(next: Option<NodeId>) {
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
/// `:focus-within` changes; then each popup focus left for another node is dismissed
/// ([`Scene::popups_left_by`]).
pub fn notify_focus_moved<P: Realization>(next: Option<NodeId>) {
  let (left, reached, popups) = Scene::<P>::with(|scene| {
    let (left, reached) = scene.move_focus(next);
    let popups = next.map_or_else(Vec::new, |next| scene.popups_left_by(next));

    (left, reached, popups)
  });
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

  for popup in popups {
    notify_dismiss::<P>(popup);
  }
}

/// Notifies `node` of its new scroll offsets.
pub fn notify_scroll<P: Realization>(node: NodeId, event: &ScrollEvent) {
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
pub fn notify_change<P: Realization>(node: NodeId, value: &str) {
  notify::<P, _>(
    node,
    |handler| match handler {
      Handler::Change(run) => Some(run),
      _ => None,
    },
    |handler| handler(value),
  );
}

/// Notifies `node` of its new checked state.
pub fn notify_toggle<P: Realization>(node: NodeId, checked: bool) {
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
pub fn notify_dismiss<P: Realization>(node: NodeId) {
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
pub fn dispatch_context_menu<P: Realization>(node: NodeId, event: &PointerEvent) -> Prevented {
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

/// The nodes a move from one node to another crosses, read from their ancestries, a
/// node and the nodes it stands in, itself first, as [`Scene::ancestry`] gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(in crate::spine) struct Crossing {
  /// The nodes whose subtree the move left, innermost first.
  pub(in crate::spine) left: SmallVec<[NodeId; 16]>,
  /// The nodes whose subtree the move reached, outermost first.
  pub(in crate::spine) reached: SmallVec<[NodeId; 16]>,
}

impl Crossing {
  /// What a move from the node of ancestry `from` to the node of ancestry `to` crosses;
  /// an empty ancestry for no node.
  pub(in crate::spine) fn between(from: &[NodeId], to: &[NodeId]) -> Self {
    let mut reached = Self::leaving(to, from);

    reached.reverse();

    Self {
      left: Self::leaving(from, to),
      reached,
    }
  }

  /// The nodes of the ancestry `from` that are not in the ancestry `to`, innermost
  /// first: those a move from the one node to the other leaves.
  pub(in crate::spine) fn leaving(from: &[NodeId], to: &[NodeId]) -> SmallVec<[NodeId; 16]> {
    from
      .iter()
      .copied()
      .filter(|node| !to.contains(node))
      .collect()
  }
}

impl Prevented {
  /// Climbs `path`, a dispatch's nodes deepest first, folding the verdicts of the
  /// handlers on each: `select` picks a node's handlers, in order, with whether the
  /// selection ends the climb there, and `run` runs each with the payload `prepare` makes
  /// for its node, made only where a handler runs.
  ///
  /// Every handler a node carries runs, and the climb ends past the node whose verdicts
  /// prevent bubbling or whose selection ends it.
  pub(in crate::spine) fn climb<N: Copy, T, E>(
    path: impl IntoIterator<Item = N>,
    mut select: impl FnMut(N) -> (SmallVec<[T; 2]>, bool),
    prepare: impl Fn(N) -> E,
    mut run: impl FnMut(&T, &E) -> Prevent,
  ) -> Self {
    let mut prevented = Self::default();

    for node in path {
      let (handlers, ends) = select(node);

      if !handlers.is_empty() {
        let payload = prepare(node);

        for handler in &handlers {
          prevented.absorb(run(handler, &payload));
        }
      }

      if prevented.bubbling || ends {
        break;
      }
    }

    prevented
  }
}

impl HandlerSet {
  /// The handlers a click runs on one node: its acting ones, then the rest, ending the
  /// climb nowhere.
  fn select_click(&self, out: &mut Selected<ClickHandler>) -> bool {
    out.extend(self.select(|handler| match handler {
      Handler::ActingClick(run) => Some(run),
      _ => None,
    }));
    out.extend(self.select(|handler| match handler {
      Handler::Click(run) => Some(run),
      _ => None,
    }));

    false
  }

  /// The handlers a click the platform already acted on runs on one node: all but the
  /// acting ones, ending the climb at a node carrying acting ones.
  fn select_observed_click(&self, out: &mut Selected<ClickHandler>) -> bool {
    out.extend(self.select(|handler| match handler {
      Handler::Click(run) => Some(run),
      _ => None,
    }));

    self.has(|handler| match handler {
      Handler::ActingClick(run) => Some(run),
      _ => None,
    })
  }
}

/// A selection taking every handler `pick` takes, ending the climb nowhere.
fn all<H: ?Sized>(pick: Pick<H>) -> impl Fn(&HandlerSet, &mut Selected<H>) -> bool {
  move |set, out| {
    out.extend(set.select(pick));

    false
  }
}

/// The handlers `select` picks on one node, cloned out so the scene is released
/// before any runs; with whether `select` ends the climb at the node.
fn snapshot<P: Realization, H: ?Sized>(
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
fn bubble<P: Realization, H: ?Sized, E>(
  node: NodeId,
  select: impl Fn(&HandlerSet, &mut Selected<H>) -> bool,
  prepare: impl Fn(NodeId) -> E,
  run: impl Fn(&H, &E) -> Prevent,
) -> Prevented {
  batch(|| {
    Prevented::climb(
      Scene::<P>::with(|scene| scene.ancestry(node)),
      |at| snapshot::<P, H>(at, &select),
      prepare,
      |handler, payload| run_handler(handler.owner(), || run(handler, payload)),
    )
  })
}

/// Runs the handlers `pick` takes on `node` alone.
fn notify<P: Realization, H: ?Sized>(node: NodeId, pick: Pick<H>, run: impl Fn(&H)) {
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

  use smallvec::SmallVec;

  use super::{Crossing, HandlerSet, Selected};
  use crate::spine::{
    Bubbling,
    ClickEvent,
    ClickHandler,
    Handler,
    NodeId,
    PointerButton,
    PointerEvent,
    Prevent,
    Prevented,
    scene::Held,
  };

  /// A click handler noting `name` in `log`.
  fn record(log: &Rc<RefCell<Vec<&'static str>>>, name: &'static str) -> Rc<ClickHandler> {
    let log = Rc::clone(log);

    Rc::new(move |_: &ClickEvent| {
      log.borrow_mut().push(name);

      Prevent::None
    })
  }

  /// A plain node of a climb: its name, its handlers' names and verdicts in order, and
  /// whether its selection ends the climb.
  type PlainNode = (&'static str, &'static [(&'static str, Prevent)], bool);

  /// Climbs `path`, deepest first, over plain handlers, returning what it folded and the
  /// names of the handlers it ran, in order.
  fn climb(path: &[PlainNode]) -> (Prevented, Vec<&'static str>) {
    let ran = RefCell::new(Vec::new());
    let prevented = Prevented::climb(
      path,
      |(_, handlers, ends)| (handlers.iter().copied().collect(), *ends),
      |_| (),
      |(name, verdict): &(&'static str, Prevent), ()| {
        ran.borrow_mut().push(*name);

        *verdict
      },
    );

    (prevented, ran.into_inner())
  }

  /// The handlers `select` picks out of `set`, run on a click, their names in the order
  /// they ran, with whether the selection ends the climb.
  fn clicked(
    set: &HandlerSet,
    select: fn(&HandlerSet, &mut Selected<ClickHandler>) -> bool,
    log: &Rc<RefCell<Vec<&'static str>>>,
  ) -> (Vec<&'static str>, bool) {
    let mut handlers = Selected::new();
    let ends = select(set, &mut handlers);

    for handler in &handlers {
      let _ = handler(&ClickEvent::new(1));
    }

    (std::mem::take(&mut *log.borrow_mut()), ends)
  }

  /// Handlers run deepest first and every handler on a node runs, a click's acting
  /// handlers before the node's others.
  #[test]
  fn click_runs_deepest_first() {
    assert_eq!(
      climb(&[
        (
          "inner",
          &[
            ("inner first", Prevent::None),
            ("inner second", Prevent::None)
          ],
          false
        ),
        ("middle", &[], false),
        ("outer", &[("outer", Prevent::None)], false),
      ]),
      (
        Prevented::default(),
        vec!["inner first", "inner second", "outer"]
      )
    );

    let log = Rc::new(RefCell::new(Vec::new()));
    let mut set = HandlerSet::default();

    set.push(Handler::Click(record(&log, "first")));
    set.push(Handler::ActingClick(record(&log, "acting")));
    set.push(Handler::Click(record(&log, "second")));

    assert_eq!(
      clicked(&set, HandlerSet::select_click, &log),
      (vec!["acting", "first", "second"], false)
    );
  }

  /// A node that stops bubbling keeps the event from its ancestors, every handler on it
  /// still running, and its verdict folds into the result.
  #[test]
  fn bubbling_stop_ends_the_climb() {
    assert_eq!(
      climb(&[
        ("inner", &[("inner", Prevent::None)], false),
        (
          "middle",
          &[
            ("middle", Prevent::Bubbling),
            ("middle after", Prevent::None)
          ],
          false
        ),
        ("outer", &[("outer", Prevent::None)], false),
      ]),
      (
        Prevented {
          platform: false,
          bubbling: true,
        },
        vec!["inner", "middle", "middle after"]
      )
    );
    assert_eq!(Prevent::from(Bubbling::Stop), Prevent::Bubbling);
  }

  /// The `Prevent::Realization` verdict is the control's own: it suppresses the native
  /// response without stopping the ancestors.
  #[test]
  fn platform_verdict_is_the_controls() {
    assert_eq!(
      climb(&[
        ("control", &[("control", Prevent::Realization)], false),
        ("outer", &[("outer", Prevent::None)], false),
      ]),
      (
        Prevented {
          platform: true,
          bubbling: false,
        },
        vec!["control", "outer"]
      )
    );
  }

  /// An acting handler runs before the node's observers on a click, and an observed
  /// click skips it while the observers beneath and on its node still run, the climb
  /// ending there as the acting handler's verdict ends it.
  #[test]
  fn observed_click_skips_the_acting_handler() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut inner = HandlerSet::default();
    let mut middle = HandlerSet::default();
    let mut outer = HandlerSet::default();
    let acting = {
      let log = Rc::clone(&log);

      Rc::new(move |_: &ClickEvent| {
        log.borrow_mut().push("acting");

        Prevent::Bubbling
      })
    };

    inner.push(Handler::Click(record(&log, "inner")));
    middle.push(Handler::Click(record(&log, "observer")));
    middle.push(Handler::ActingClick(acting));
    outer.push(Handler::Click(record(&log, "outer")));

    let path = [&inner, &middle, &outer];
    let dispatch = |select: fn(&HandlerSet, &mut Selected<ClickHandler>) -> bool| {
      let prevented = Prevented::climb(
        path,
        |set| {
          let mut handlers = Selected::new();
          let ends = select(set, &mut handlers);

          (handlers, ends)
        },
        |_| (),
        |handler, ()| handler(&ClickEvent::new(1)),
      );

      (prevented, std::mem::take(&mut *log.borrow_mut()))
    };

    assert_eq!(
      dispatch(HandlerSet::select_click),
      (
        Prevented {
          platform: false,
          bubbling: true,
        },
        vec!["inner", "acting", "observer"]
      )
    );
    assert_eq!(
      dispatch(HandlerSet::select_observed_click),
      (Prevented::default(), vec!["inner", "observer"])
    );
    assert_eq!(
      clicked(&middle, HandlerSet::select_observed_click, &log),
      (vec!["observer"], true)
    );
  }

  /// A pointer move leaves each node whose subtree the pointer left, innermost first,
  /// then enters each whose subtree it reached, outermost first; a node holding the
  /// pointer throughout hears nothing and stays hovered.
  #[test]
  fn a_pointer_move_notifies_the_subtrees_it_crosses() {
    let [outer, middle, inner, sibling] = std::array::from_fn(NodeId::plain);
    let ancestry = |node: NodeId| -> SmallVec<[NodeId; 16]> {
      match node {
        node if node == inner => [inner, middle, outer].into_iter().collect(),
        node if node == middle => [middle, outer].into_iter().collect(),
        node => [node, outer].into_iter().collect(),
      }
    };
    let mut held = Held::new();
    let mut hover = |next: Option<NodeId>| {
      let previous = held.hover(next);
      let crossing = Crossing::between(
        &previous.map(ancestry).unwrap_or_default(),
        &next.map(ancestry).unwrap_or_default(),
      );

      (crossing.left.to_vec(), crossing.reached.to_vec())
    };

    assert_eq!(hover(Some(inner)), (vec![], vec![outer, middle, inner]));
    assert_eq!(hover(Some(sibling)), (vec![inner, middle], vec![sibling]));
    assert_eq!(hover(Some(sibling)), (vec![], vec![]));

    let generation = held.generation();

    assert!(held.hovers(outer, ancestry));
    assert!(!held.hovers(middle, ancestry));
    assert_eq!(held.hover(None), Some(sibling));
    assert_eq!(held.generation(), generation + 1);
    assert_eq!(
      Crossing::between(&ancestry(sibling), &[]),
      Crossing {
        left: [sibling, outer].into_iter().collect(),
        reached: SmallVec::new(),
      }
    );
  }

  /// A focus move blurs each node whose subtree focus left, innermost first, then
  /// focuses each whose subtree it reached, outermost first; a node holding it
  /// throughout hears nothing.
  #[test]
  fn a_focus_move_notifies_the_subtrees_it_crosses() {
    let [outer, middle, inner, sibling] = std::array::from_fn(NodeId::plain);
    let ancestry = |node: Option<NodeId>| -> SmallVec<[NodeId; 16]> {
      match node {
        None => SmallVec::new(),
        Some(node) if node == inner => [inner, middle, outer].into_iter().collect(),
        Some(node) => [node, outer].into_iter().collect(),
      }
    };
    let mut held = Held::new();
    let mut focus = |next: Option<NodeId>| {
      let previous = held.focus(next);
      let crossing = Crossing::between(&ancestry(previous), &ancestry(next));

      (crossing.left.to_vec(), crossing.reached.to_vec())
    };

    assert_eq!(focus(Some(inner)), (vec![], vec![outer, middle, inner]));
    assert_eq!(focus(Some(sibling)), (vec![inner, middle], vec![sibling]));
    assert_eq!(focus(None), (vec![sibling, outer], vec![]));
    assert_eq!(held.focused(), None);
    assert_eq!(
      Crossing::leaving(&ancestry(Some(inner)), &ancestry(Some(sibling))).to_vec(),
      [inner, middle]
    );
  }

  /// A pointer event reaches each handling node local to that node's own box, made only
  /// for the nodes whose handlers run.
  #[test]
  fn pointer_events_localize_per_node() {
    let event = PointerEvent {
      local_x: 5.,
      local_y: 5.,
      ..PointerEvent::mouse(35., 125.)
    }
    .with_press(PointerButton::Primary);
    let origins: [(&'static str, &'static [&'static str], f32, f32); 3] = [
      ("inner", &["inner"], 30., 120.),
      ("middle", &[], 20., 110.),
      ("outer", &["outer"], 10., 100.),
    ];
    let prepared = RefCell::new(Vec::new());
    let seen = RefCell::new(Vec::new());
    let prevented = Prevented::climb(
      &origins,
      |(_, handlers, _, _)| (handlers.iter().copied().collect(), false),
      |(node, _, x, y)| {
        prepared.borrow_mut().push(*node);

        PointerEvent {
          local_x: event.x - x,
          local_y: event.y - y,
          ..event
        }
      },
      |name: &&'static str, local: &PointerEvent| {
        seen
          .borrow_mut()
          .push((*name, local.local_x, local.local_y));

        Prevent::None
      },
    );

    assert_eq!(prevented, Prevented::default());
    assert_eq!(*seen.borrow(), [("inner", 5., 5.), ("outer", 25., 25.)]);
    assert_eq!(*prepared.borrow(), ["inner", "outer"]);
  }
}
