//! Reads and writes that come after a disposal: a debug build remembers where each
//! disposed signal was created and what disposed it, and names both with the late
//! access.

#[cfg(debug_assertions)]
use std::{cell::Cell, collections::VecDeque, fmt, panic::Location};

#[cfg(debug_assertions)]
use rustc_hash::FxHashMap;

use crate::id::{Id, Kind};

/// How many disposed signals a debug build remembers.
#[cfg(debug_assertions)]
const REMEMBERED: usize = 4096;

#[cfg(debug_assertions)]
thread_local! {
  /// The public call a disposal in progress started from, if any.
  static DISPOSING_AT: Cell<Option<&'static Location<'static>>> = const { Cell::new(None) };
}

/// Where a disposed signal was created, and the public call that disposed it; none
/// when it went with an owner's re-run or its handle's drop.
#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct Obituary {
  pub(crate) created_at: Option<&'static Location<'static>>,
  pub(crate) disposed_at: Option<&'static Location<'static>>,
}

#[cfg(debug_assertions)]
impl fmt::Display for Obituary {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.created_at {
      Some(created) => write!(formatter, "created at {created}, ")?,
      None => write!(formatter, "created at an unknown site, ")?,
    }

    match self.disposed_at {
      Some(disposed) => write!(formatter, "disposed by the call at {disposed}"),
      None => write!(formatter, "disposed with its owner's re-run or teardown"),
    }
  }
}

/// The obituaries of the most recently disposed signals, the oldest forgotten first.
#[cfg(debug_assertions)]
#[derive(Default)]
pub(crate) struct Obituaries {
  by_id: FxHashMap<Id, Obituary>,
  order: VecDeque<Id>,
}

#[cfg(debug_assertions)]
impl Obituaries {
  /// Remembers `obituary` for the disposed signal `id`.
  pub(crate) fn record(&mut self, id: Id, obituary: Obituary) {
    if self.order.len() == REMEMBERED
      && let Some(oldest) = self.order.pop_front()
    {
      self.by_id.remove(&oldest);
    }

    self.by_id.insert(id, obituary);
    self.order.push_back(id);
  }

  /// The obituary of the disposed signal `id`, while it is remembered.
  pub(crate) fn get(&self, id: Id) -> Option<Obituary> {
    self.by_id.get(&id).copied()
  }
}

/// A disposal started from a public call, in progress until dropped.
#[cfg(debug_assertions)]
pub(crate) struct Disposing {
  outermost: bool,
}

#[cfg(debug_assertions)]
impl Disposing {
  /// Marks a disposal as started from the call at `site`, unless one already is.
  pub(crate) fn enter(site: &'static Location<'static>) -> Self {
    let outermost = DISPOSING_AT.with(|at| {
      if at.get().is_some() {
        return false;
      }

      at.set(Some(site));

      true
    });

    Self { outermost }
  }

  /// The public call the disposal in progress started from, if any.
  pub(crate) fn site() -> Option<&'static Location<'static>> {
    DISPOSING_AT.with(Cell::get)
  }
}

#[cfg(debug_assertions)]
impl Drop for Disposing {
  fn drop(&mut self) {
    if self.outermost {
      DISPOSING_AT.with(|at| at.set(None));
    }
  }
}

impl Id {
  /// What this id names, for a message.
  const fn noun(self) -> &'static str {
    match self.kind() {
      Kind::Signal | Kind::Sync => "a signal",
      Kind::Computation => "a memo",
      Kind::Owner => "an owner",
    }
  }

  /// Panics for a read of this id after its disposal; a debug build names the read and
  /// what it remembers of the disposal.
  #[cold]
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn panic_disposed(self) -> ! {
    #[cfg(debug_assertions)]
    {
      let read = Location::caller();

      match crate::runtime::try_with_graph(|graph| graph.obituary(self)).flatten() {
        Some(obituary) => panic!(
          "{} read at {read} after its disposal: {obituary}",
          self.noun()
        ),
        None => panic!("{} read at {read} after its disposal", self.noun()),
      }
    }

    #[cfg(not(debug_assertions))]
    panic!("{} read after its disposal", self.noun());
  }

  /// A write to this id after its disposal, which is dropped; a debug build warns,
  /// naming the write and what it remembers of the disposal.
  #[cold]
  #[cfg_attr(debug_assertions, track_caller)]
  #[cfg_attr(
    not(debug_assertions),
    expect(clippy::unused_self, reason = "only a debug build reports the write")
  )]
  pub(crate) fn disposed_write(self) {
    #[cfg(debug_assertions)]
    {
      let write = Location::caller();

      match crate::runtime::try_with_graph(|graph| graph.obituary(self)).flatten() {
        Some(obituary) => eprintln!(
          "skene-reactive: {} written at {write} after its disposal, and the write dropped: \
           {obituary}",
          self.noun()
        ),
        None => eprintln!(
          "skene-reactive: {} written at {write} after its disposal, and the write dropped",
          self.noun()
        ),
      }
    }
  }
}
