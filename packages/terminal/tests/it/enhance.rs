//! Enhancement in cells: the fallback stands while the signal is false and the child
//! once it is true, each built at most once, a signal a form reads while building
//! rebuilds nothing, and a signal true from the start never builds the fallback.

use std::{cell::Cell, rc::Rc};

use skene_terminal::{platform::terminal::mount, prelude::*, testing};

use crate::common::{rows, with_platform};

/// While the signal is false the fallback stands; once it is true the child replaces
/// it and stands whatever the signal does after, and neither is built twice.
#[test]
fn the_fallback_stands_until_the_signal_turns() {
  with_platform(|| {
    let ready = Signal::new(false);
    let fallbacks = Rc::new(Cell::new(0));
    let children = Rc::new(Cell::new(0));

    let _ = mount({
      let fallbacks = Rc::clone(&fallbacks);
      let children = Rc::clone(&children);

      move || {
        Enhance::new(ready.read_only())
          .set_fallback_builder(move || {
            fallbacks.set(fallbacks.get() + 1);

            Text::new("waiting")
          })
          .set_child_builder(move || {
            children.set(children.get() + 1);

            Text::new("ready")
          })
          .into_any()
          .build()
      }
    });

    assert_eq!(testing::render_to_text(10, 1), "waiting\n");
    assert_eq!((fallbacks.get(), children.get()), (1, 0));

    ready.set(true);

    assert_eq!(
      testing::render_to_text(10, 1),
      "ready\n",
      "the child took the fallback's place"
    );
    assert_eq!((fallbacks.get(), children.get()), (1, 1));

    ready.set(false);

    assert_eq!(
      testing::render_to_text(10, 1),
      "ready\n",
      "the child stands once built"
    );
    assert_eq!((fallbacks.get(), children.get()), (1, 1));
  });
}

/// A signal a form reads while it builds never rebuilds it: the fallback and the child
/// each stand, built once, as the signal changes.
#[test]
fn a_signal_read_while_building_rebuilds_nothing() {
  with_platform(|| {
    let ready = Signal::new(false);
    let read = Signal::new(0);
    let builds = Rc::new(Cell::new(0));

    let _ = mount({
      let fallback_builds = Rc::clone(&builds);
      let child_builds = Rc::clone(&builds);

      move || {
        Enhance::new(ready.read_only())
          .set_fallback_builder(move || {
            fallback_builds.set(fallback_builds.get() + 1);

            Text::new(format!("waiting {}", read.get()))
          })
          .set_child_builder(move || {
            child_builds.set(child_builds.get() + 1);

            Text::new(format!("ready {}", read.get()))
          })
          .into_any()
          .build()
      }
    });

    read.set(1);

    assert_eq!(testing::render_to_text(10, 1), "waiting 0\n");

    ready.set(true);

    let _ = testing::render_to_text(10, 1);

    read.set(2);

    assert_eq!(testing::render_to_text(10, 1), "ready 1\n");
    assert_eq!(builds.get(), 2);
  });
}

/// A signal true from the start never builds the fallback, and an enhancement with no
/// fallback shows nothing while its signal is false.
#[test]
fn a_signal_true_from_the_start_never_builds_the_fallback() {
  with_platform(|| {
    let fallbacks = Rc::new(Cell::new(0));

    let _ = mount({
      let fallbacks = Rc::clone(&fallbacks);

      move || {
        Column::new()
          .set_children((
            Text::new("above"),
            Enhance::new(Signal::new(true).read_only())
              .set_fallback_builder(move || {
                fallbacks.set(fallbacks.get() + 1);

                Text::new("waiting")
              })
              .set_child_builder(|| Text::new("ready")),
            Enhance::new(Signal::new(false).read_only()).set_child_builder(|| Text::new("bare")),
            Text::new("below"),
          ))
          .into_any()
          .build()
      }
    });

    assert_eq!(
      rows(&testing::render_to_text(10, 4)),
      "above\nready\nbelow\n\n",
      "nothing stands in without a fallback"
    );
    assert_eq!(fallbacks.get(), 0);
  });
}

/// An enhancement without its child keeps its fallback while the signal is false and
/// empties once it turns.
#[test]
fn an_enhancement_without_its_child_empties_once_live() {
  with_platform(|| {
    let live = Signal::new(false);

    let _ = mount(move || {
      Column::new()
        .set_children((
          Enhance::new(live.read_only()).set_fallback_builder(|| Text::new("fallback")),
          Text::new("after"),
        ))
        .into_any()
        .build()
    });

    assert_eq!(rows(&testing::render_to_text(10, 2)), "fallback\nafter\n");

    live.set(true);

    assert_eq!(rows(&testing::render_to_text(10, 2)), "after\n\n");
  });
}
