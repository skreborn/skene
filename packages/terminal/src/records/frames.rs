//! When the next frame is due and what it repaints: the damage, the frame clock and the
//! scheduled actions.

use std::time::Instant;

use skene_core::Scene;

use super::{CellRect, Handle, Records};
use crate::Terminal;

/// How long the loop waits for input between frames while the frame clock runs: the
/// frame rate's cap.
pub(crate) const FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

/// The clock motion samples against, in seconds: the wall clock in a launched app, a
/// hand-stepped one under a headless test.
pub(crate) enum Clock {
  /// The wall clock since `Instant`.
  Real(Instant),
  /// A reading stepped by hand.
  Manual(f64),
}

impl Clock {
  /// The current reading, in seconds.
  #[inline(always)]
  pub(crate) fn now(&self) -> f64 {
    match self {
      Self::Real(epoch) => epoch.elapsed().as_secs_f64(),
      Self::Manual(seconds) => *seconds,
    }
  }

  /// Steps the reading forward by `seconds`; a wall clock becomes a hand-stepped one
  /// at its current reading.
  #[inline(always)]
  pub(crate) fn advance(&mut self, seconds: f64) {
    *self = Self::Manual(self.now() + seconds);
  }
}

impl Records {
  /// Marks `handle` for repaint: the rows it covered at the last paint and the rows it
  /// covers now are rewritten at the next frame.
  #[inline(always)]
  pub(crate) fn damage(&mut self, handle: Handle) {
    self.damaged.push(handle);
  }

  /// Takes the nodes and rectangles marked for repaint.
  pub(crate) fn take_damage(&mut self) -> (Vec<Handle>, Vec<CellRect>) {
    (
      std::mem::take(&mut self.damaged),
      std::mem::take(&mut self.damaged_rects),
    )
  }

  /// Whether anything is left to paint: a whole-frame repaint, a pending restyle, a
  /// moved interaction, a layout or structure change, or a damaged node.
  pub(crate) fn needs_frame(&self) -> bool {
    self.paint_dirty
      || self.restyle_all
      || !self.restyle_queue.is_empty()
      || Scene::<Terminal>::read(Scene::interaction_generation) != self.interaction_seen
      || self.layout_dirty
      || self.structure_dirty
      || self.frame_clock_running()
      || !self.damaged.is_empty()
      || !self.damaged_rects.is_empty()
  }

  /// Whether the next frame is due at the frame rate: a tween or an animation runs, or
  /// a caret drag is held past its entry's edge.
  #[inline(always)]
  pub(crate) fn frame_clock_running(&self) -> bool {
    self.motion_active || self.autoscrolls()
  }

  /// How long until the earliest scheduled action is due, if any is; a delay past
  /// what a duration holds is the longest one.
  pub(crate) fn next_timer_due(&self) -> Option<std::time::Duration> {
    let now = self.now();

    self.timers.next_due().map(|at| {
      std::time::Duration::try_from_secs_f64((at - now).max(0.)).unwrap_or(std::time::Duration::MAX)
    })
  }

  /// The clock reading motion samples against, in seconds.
  #[inline(always)]
  pub(crate) fn now(&self) -> f64 {
    self.clock.now()
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use skene_core::Platform as _;

  use crate::{Terminal, with_records};

  /// A delay past what a duration holds waits the longest a duration can.
  #[test]
  fn a_huge_delay_waits_the_longest() {
    crate::install();

    let _ = Terminal::schedule(Duration::MAX, Box::new(|| {}));

    assert_eq!(
      with_records(|records| records.next_timer_due()),
      Some(Duration::MAX)
    );

    crate::uninstall();
  }
}
