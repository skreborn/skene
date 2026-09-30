//! Enhancement: the fallback builds while the signal is false and the child once it
//! is true, each at most once, and the built-ins wait on their platform's own signals.

use std::{cell::Cell, rc::Rc};

use skene_common::{Enhance, media};
use skene_core::{NodeId, Scene, View};
use skene_headless::{Headless, end_turn, reset, text, trigger_hydrated};
use skene_reactive::{Get, Owner, Set, Signal};

type Text = skene_common::Text<Headless>;

/// The nodes the segment `start` begins holds, once the pending turn has ended.
fn content(start: NodeId) -> Vec<NodeId> {
  end_turn();

  Scene::<Headless>::with(|scene| scene.segment_nodes(start))
}

/// The text of the one node the segment `start` holds.
fn shown(start: NodeId) -> String {
  let nodes = content(start);

  assert_eq!(nodes.len(), 1, "one form at a time: {nodes:?}");

  text(nodes[0]).expect("a text")
}

/// While the signal is false the fallback stands; once it is true the child replaces
/// it and stands whatever the signal does after, and neither is built twice.
#[test]
fn the_fallback_stands_until_the_signal_turns() {
  reset();

  let ready = Signal::new(false);
  let fallbacks = Rc::new(Cell::new(0));
  let children = Rc::new(Cell::new(0));
  let owner = Owner::new();
  let start = owner.enter(|| {
    Enhance::<Headless>::new(ready.read_only())
      .set_fallback({
        let fallbacks = Rc::clone(&fallbacks);

        move || {
          fallbacks.set(fallbacks.get() + 1);

          Text::new("baseline")
        }
      })
      .set_child_builder({
        let children = Rc::clone(&children);

        move || {
          children.set(children.get() + 1);

          Text::new("live")
        }
      })
      .build()
  });

  assert_eq!(shown(start), "baseline");
  assert_eq!((fallbacks.get(), children.get()), (1, 0));

  ready.set(true);

  assert_eq!(shown(start), "live", "the child took the fallback's place");
  assert_eq!((fallbacks.get(), children.get()), (1, 1));

  ready.set(false);

  assert_eq!(shown(start), "live", "the child stands once built");
  assert_eq!((fallbacks.get(), children.get()), (1, 1));
}

/// A signal a form reads while it builds never rebuilds it: the fallback and the child
/// each stand, built once, as the signal changes.
#[test]
fn a_signal_read_while_building_rebuilds_nothing() {
  reset();

  let ready = Signal::new(false);
  let read = Signal::new(0);
  let builds = Rc::new(Cell::new(0));
  let owner = Owner::new();
  let start = owner.enter(|| {
    Enhance::<Headless>::new(ready.read_only())
      .set_fallback({
        let builds = Rc::clone(&builds);

        move || {
          builds.set(builds.get() + 1);

          Text::new(format!("baseline {}", read.get()))
        }
      })
      .set_child_builder({
        let builds = Rc::clone(&builds);

        move || {
          builds.set(builds.get() + 1);

          Text::new(format!("live {}", read.get()))
        }
      })
      .build()
  });

  read.set(1);

  assert_eq!(shown(start), "baseline 0");

  ready.set(true);
  end_turn();
  read.set(2);

  assert_eq!(shown(start), "live 1");
  assert_eq!(builds.get(), 2);
}

/// A signal true from the start never builds the fallback, and a segment with no
/// fallback stays empty while its signal is false.
#[test]
fn a_signal_true_from_the_start_never_builds_the_fallback() {
  reset();

  let fallbacks = Rc::new(Cell::new(0));
  let owner = Owner::new();
  let (live, bare) = owner.enter(|| {
    (
      Enhance::<Headless>::new(Signal::new(true).read_only())
        .set_fallback({
          let fallbacks = Rc::clone(&fallbacks);

          move || {
            fallbacks.set(fallbacks.get() + 1);

            Text::new("baseline")
          }
        })
        .set_child_builder(|| Text::new("live"))
        .build(),
      Enhance::<Headless>::new(Signal::new(false).read_only())
        .set_child_builder(|| Text::new("live"))
        .build(),
    )
  });

  assert_eq!(shown(live), "live");
  assert_eq!(fallbacks.get(), 0);
  assert!(
    content(bare).iter().all(|node| text(*node).is_none()),
    "nothing stands in without a fallback"
  );
}

/// The hydration signal is the headless platform's stand-in for the web's: hydrated
/// until a test says otherwise, and an enhancement over it swaps when it turns.
#[test]
fn hydration_is_answered_by_the_platform() {
  reset();
  trigger_hydrated(false);

  let owner = Owner::new();
  let start = owner.enter(|| {
    Enhance::<Headless>::new(media::hydrated::<Headless>())
      .set_fallback(|| Text::new("markup"))
      .set_child_builder(|| Text::new("hydrated"))
      .build()
  });

  assert_eq!(shown(start), "markup");

  trigger_hydrated(true);

  assert_eq!(shown(start), "hydrated");
}
