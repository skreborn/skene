use std::cell::RefCell;

use skene_reactive::Runtime;

use crate::spine::{Platform, Scene, task};

thread_local! {
  /// The flush the turn's writes scheduled, run when the turn ends.
  static TURN: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

/// Holds every flush on this thread until its turn ends.
///
/// The flush a turn's writes schedule waits for [`end_turn`], and `on_hold` runs as a
/// turn's first write is held, where a platform ends the turn on its own clock.
pub fn hold_flushes(on_hold: impl Fn() + 'static) {
  Runtime::set_scheduler(move |flush| {
    TURN.with(|turn| *turn.borrow_mut() = Some(flush));
    on_hold();
  });
}

/// Ends the turn: runs the flush the writes since the last turn's end scheduled, so
/// their effects have taken hold.
pub fn end_turn() {
  let flush = TURN.with(|turn| turn.borrow_mut().take());

  if let Some(flush) = flush {
    flush();
  }
}

/// Resets what running the platform `P` left on this thread.
///
/// The turn is dropped unflushed, and the scene, the reactive runtime, the root scope
/// and the tasks start over, so one thread can run many independent mounts.
pub fn reset_thread<P: Platform>() {
  TURN.with(|turn| *turn.borrow_mut() = None);
  Scene::<P>::with(|scene| *scene = Scene::new());
  Runtime::reset();
  task::clear();
}
