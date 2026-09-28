//! The pooled, intrusively linked dependency edges between deps and subs.

use crate::id::Id;

/// An index into the link pool.
pub(crate) type LinkIdx = u32;

/// The absent link.
pub(crate) const NONE: LinkIdx = u32::MAX;

/// One dependency edge: `sub` read `dep`, when `dep` was at `version`.
///
/// Every link sits on two lists at once: its dep's subscriber list (`prev_sub`,
/// `next_sub`) and its sub's dependency list (`prev_dep`, `next_dep`), the latter in
/// read order.
#[derive(Clone, Copy)]
pub(crate) struct Link {
  pub(crate) dep: Id,
  pub(crate) sub: Id,
  pub(crate) prev_dep: LinkIdx,
  pub(crate) next_dep: LinkIdx,
  pub(crate) prev_sub: LinkIdx,
  pub(crate) next_sub: LinkIdx,
  pub(crate) version: u32,
}

/// The ends of one node's list of links.
#[derive(Clone, Copy)]
pub(crate) struct LinkList {
  /// The first link.
  pub(crate) head: LinkIdx,
  /// The last link; on the dependency list of a running computation, the last link
  /// the run has read, the links past it being last run's still to be dropped.
  pub(crate) tail: LinkIdx,
}

impl LinkList {
  pub(crate) const EMPTY: Self = Self {
    head: NONE,
    tail: NONE,
  };
}

/// The pool every link is allocated from; freed slots are reused.
pub(crate) struct LinkPool {
  links: Vec<Link>,
  free: Vec<LinkIdx>,
}

impl LinkPool {
  pub(crate) const fn new() -> Self {
    Self {
      links: Vec::new(),
      free: Vec::new(),
    }
  }

  pub(crate) fn allocate(&mut self, link: Link) -> LinkIdx {
    if let Some(index) = self.free.pop() {
      self.links[index as usize] = link;

      return index;
    }

    let index = self.links.len() as LinkIdx;

    self.links.push(link);

    index
  }

  #[inline(always)]
  pub(crate) fn get(&self, index: LinkIdx) -> Link {
    self.links[index as usize]
  }

  #[inline(always)]
  pub(crate) fn get_mut(&mut self, index: LinkIdx) -> &mut Link {
    &mut self.links[index as usize]
  }

  pub(crate) fn free(&mut self, index: LinkIdx) {
    let link = &mut self.links[index as usize];

    link.prev_dep = NONE;
    link.next_dep = NONE;
    link.prev_sub = NONE;
    link.next_sub = NONE;
    self.free.push(index);
  }
}
