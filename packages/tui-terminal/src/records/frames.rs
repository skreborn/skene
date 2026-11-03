//! When the next frame is due and what it repaints: the damage, the frame clock's pace
//! and the scheduled actions.

use skene_foundation::{NodeId, Scene};

use super::{CellRect, Records};
use crate::Terminal;

/// How long the loop waits for input between frames while the frame clock runs: the
/// frame rate's cap.
pub(crate) const FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

impl Records {
  /// Takes the nodes and rectangles marked for repaint.
  pub(crate) fn take_damage(&mut self) -> (Vec<NodeId>, Vec<CellRect>) {
    (
      self.runtime.take_damage(),
      std::mem::take(&mut self.damaged_rects),
    )
  }

  /// Whether anything is left to paint: a whole-frame repaint, a pending restyle, a
  /// moved interaction, a layout or structure change, or a damaged node.
  pub(crate) fn needs_frame(&self) -> bool {
    self.runtime.paint_dirty
      || self
        .runtime
        .restyle
        .is_pending(Scene::<Terminal>::read(Scene::interaction_generation))
      || self.runtime.layout_dirty
      || self.frame_clock_running()
      || self.runtime.has_damage()
      || !self.damaged_rects.is_empty()
  }

  /// Whether the next frame is due at the frame rate: a tween or an animation runs, or
  /// a caret drag is held past its entry's edge.
  #[inline(always)]
  pub(crate) fn frame_clock_running(&self) -> bool {
    self.runtime.motion_active || self.autoscrolls()
  }

  /// How long until the earliest scheduled action is due, if any is; a delay past
  /// what a duration holds is the longest one.
  pub(crate) fn next_timer_due(&self) -> Option<std::time::Duration> {
    let now = self.runtime.now();

    self.timers.next_due().map(|at| {
      std::time::Duration::try_from_secs_f64((at - now).max(0.)).unwrap_or(std::time::Duration::MAX)
    })
  }
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use skene_foundation::Realization as _;

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
