//! Propagation: marking what a change reaches and queueing the effects it makes due.

use std::cmp::Reverse;

use smallvec::SmallVec;

use super::{
  Graph,
  records::flags::{DIRTY, MEMO, PAUSED, PENDING, QUEUED, RENDER},
};
use crate::{
  id::{Id, Kind},
  link::{Link, LinkIdx, NONE},
};

impl Graph {
  /// A source changed: direct subscribers become dirty, memos downstream of them
  /// pending, and every affected effect joins its queue.
  pub(crate) fn propagate(&mut self, source: Id) {
    let head = match source.kind() {
      Kind::Signal => {
        let Some(signal) = self.signal_mut(source)
        else {
          return;
        };

        signal.version = signal.version.wrapping_add(1);
        signal.subs.head
      }
      Kind::Computation | Kind::Owner => return,
    };
    let mut stack: SmallVec<[(LinkIdx, bool); 8]> = SmallVec::new();

    stack.push((head, true));

    while let Some((mut index, mut dirty)) = stack.pop() {
      while index != NONE {
        let Link { sub, next_sub, .. } = self.links.get(index);

        index = next_sub;

        let Some(computation) = self.computation_mut(sub)
        else {
          continue;
        };

        let flags = computation.flags;

        if flags & MEMO != 0 {
          if flags & (DIRTY | PENDING) == 0 {
            computation.flags |= if dirty { DIRTY } else { PENDING };

            let downstream = computation.subs.head;

            if downstream != NONE {
              stack.push((index, dirty));
              index = downstream;
              dirty = false;
            }
          }
          else if dirty {
            computation.flags |= DIRTY;
          }

          continue;
        }

        computation.flags |= if dirty { DIRTY } else { PENDING };

        if flags & (QUEUED | PAUSED) == 0 {
          computation.flags |= QUEUED;

          let render = flags & RENDER != 0;
          let depth = computation.owner.depth;
          let order = computation.order;

          self.push_queue(sub, render, depth, order);
        }
      }
    }
  }

  fn push_queue(&mut self, id: Id, render: bool, depth: u32, order: u64) {
    if render {
      self.render_queue.push(Reverse((depth, order, id)));
    }
    else {
      self.queue.push_back(id);
    }
  }

  /// Queues an invalidated effect that is not queued yet; `false` when nothing is due.
  pub(crate) fn enqueue(&mut self, id: Id) -> bool {
    let Some(computation) = self.computation_mut(id)
    else {
      return false;
    };

    let flags = computation.flags;

    if flags & (DIRTY | PENDING) == 0 || flags & (QUEUED | MEMO) != 0 {
      return false;
    }

    computation.flags |= QUEUED;

    let render = flags & RENDER != 0;
    let depth = computation.owner.depth;
    let order = computation.order;

    self.push_queue(id, render, depth, order);

    true
  }

  /// The next effect due: render effects first, by owner depth then creation, then
  /// ordinary effects in propagation order.
  pub(crate) fn pop_queued(&mut self) -> Option<Id> {
    if let Some(Reverse((_, _, id))) = self.render_queue.pop() {
      return Some(id);
    }

    self.queue.pop_front()
  }

  /// Whether a flush has work: an effect queued, or a signal's unobserved hook due.
  pub(crate) fn has_queued(&self) -> bool {
    !self.render_queue.is_empty() || !self.queue.is_empty() || !self.unobserved.is_empty()
  }
}
