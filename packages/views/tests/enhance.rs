//! Enhancement: the fallback builds while the signal is false and the child once it
//! is true, each at most once, and the built-ins wait on their driver's own signals.

use std::{cell::Cell, rc::Rc};

use skene_core::{NodeId, Scene, View};
use skene_headless::{Headless, reset, set_hydrated, text};
use skene_reactive::{Owner, Set, Signal};
use skene_views::{Enhance, media};

type Text = skene_views::Text<Headless>;

/// The nodes the segment `start` begins holds.
fn content(start: NodeId) -> Vec<NodeId> {
  Scene::<Headless>::with(|scene| scene.segment_nodes(start))
}

/// The text of the one node the segment `start` holds.
fn shown(start: NodeId) -> String {
  let nodes = content(start);

  assert_eq!(nodes.len(), 1, "one form at a time: {nodes:?}");

  text(nodes[0]).expect("a text")
}

/// While the signal is false the fallback stands; once it is true the child replaces
/// it, and neither is built twice.
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

  assert!(
    content(start).iter().all(|node| text(*node).is_none()),
    "a form is built at most once, so the fallback does not return"
  );
  assert_eq!((fallbacks.get(), children.get()), (1, 1));
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

/// The hydration signal is the headless driver's stand-in for the web's: hydrated
/// until a test says otherwise, and an enhancement over it swaps when it turns.
#[test]
fn hydration_is_answered_by_the_driver() {
  reset();
  set_hydrated(false);

  let owner = Owner::new();
  let start = owner.enter(|| {
    Enhance::<Headless>::new(media::hydrated::<Headless>())
      .set_fallback(|| Text::new("markup"))
      .set_child_builder(|| Text::new("hydrated"))
      .build()
  });

  assert_eq!(shown(start), "markup");

  set_hydrated(true);

  assert_eq!(shown(start), "hydrated");
}
