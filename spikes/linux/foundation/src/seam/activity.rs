//! What the platform has done lately, counted: what a testing driver's `settle` waits on.
//!
//! A driver settles when, over a quiet window longer than a frame, neither count moves: no
//! frame was painted and no report was made. The shared half counts reports itself
//! ([`crate::seam::tree::report`] calls [`report`]); a realization calls [`frame`] after each
//! frame it paints (a drawing realization once it presents a frame, a toolkit from its
//! frame clock's after-paint).

use std::cell::Cell;

/// The counts at one moment.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Activity {
  /// Reports made since the thread started.
  pub reports: u64,
  /// Frames painted since the thread started.
  pub frames: u64,
}

thread_local! {
  static ACTIVITY: Cell<Activity> = const { Cell::new(Activity { reports: 0, frames: 0 }) };
}

/// Counts a report.
#[inline(always)]
pub fn report() {
  ACTIVITY.with(|activity| {
    let mut now = activity.get();

    now.reports += 1;
    activity.set(now);
  });
}

/// Counts a painted frame.
#[inline(always)]
pub fn frame() {
  ACTIVITY.with(|activity| {
    let mut now = activity.get();

    now.frames += 1;
    activity.set(now);
  });
}

/// The counts now.
#[must_use]
pub fn now() -> Activity {
  ACTIVITY.with(Cell::get)
}
