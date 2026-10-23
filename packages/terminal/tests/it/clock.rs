//! The terminal's clock: an action scheduled on it runs once a frame reads it due.

use std::{cell::RefCell, rc::Rc, time::Duration};

use skene_terminal::{Realization, Terminal, testing};

use crate::common::with_platform;

/// A frame runs the actions due by its reading earliest due first, whatever order they
/// were scheduled in, a cancelled one not at all, and one not yet due not before it is.
#[test]
fn due_actions_run_earliest_first() {
  with_platform(|| {
    let ran = Rc::new(RefCell::new(Vec::new()));
    let action = |label: &'static str| -> Box<dyn FnOnce()> {
      let ran = Rc::clone(&ran);

      Box::new(move || ran.borrow_mut().push(label))
    };
    let _ = Terminal::schedule(Duration::from_secs(2), action("late"));
    let _ = Terminal::schedule(Duration::from_secs(1), action("early"));
    let cancelled = Terminal::schedule(Duration::from_secs(1), action("cancelled"));
    let _ = Terminal::schedule(Duration::from_secs(5), action("later"));

    Terminal::cancel(cancelled);
    testing::trigger_frame(Duration::from_secs(3));

    assert_eq!(*ran.borrow(), ["early", "late"]);
    assert_eq!(testing::pending_timers(), 1, "the one not yet due waits");

    testing::trigger_frame(Duration::from_secs(2));

    assert_eq!(*ran.borrow(), ["early", "late", "later"]);
    assert_eq!(testing::pending_timers(), 0);
  });
}
