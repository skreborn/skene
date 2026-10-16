//! Members of a group, one current: a radio group's radios, a list's options.

use crate::{
  behavior::{Response, plain},
  seam::report::KeyEvent,
};

/// The axes a collection's arrows move along.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Orientation {
  /// The vertical arrows (a list).
  Vertical,
  /// The horizontal arrows.
  Horizontal,
  /// Both: down and right move forward, up and left backward (a radio group).
  Both,
}

/// A collection's behavior over `count` members, one of them current: the arrows move to the
/// next enabled member along its orientation (wrapping past either end where it wraps), and,
/// where it pages, `Home` and `End` to the first and last enabled member, `PageUp` and
/// `PageDown` by a page.
///
/// It proposes the member to move to; a radio group chooses it as focus lands on it, a list
/// in a popup highlights it.
#[derive(Clone, Copy)]
#[must_use]
pub struct Collection<'a> {
  /// How many members.
  pub count: usize,
  /// The current member.
  pub current: Option<usize>,
  /// Which arrows move.
  pub orientation: Orientation,
  /// Whether moving past an end goes round to the other.
  pub wraps: bool,
  /// The members a page key moves by; none where the collection does not page.
  pub page: Option<usize>,
  /// Whether a member can be moved to.
  pub enabled: &'a dyn Fn(usize) -> bool,
}

impl<'a> Collection<'a> {
  /// A collection of `count` members with `current`, moving along `orientation`, every
  /// member enabled, neither wrapping nor paging.
  pub fn new(count: usize, current: Option<usize>, orientation: Orientation) -> Self {
    Self {
      count,
      current,
      orientation,
      wraps: false,
      page: None,
      enabled: &|_| true,
    }
  }

  /// This collection, wrapping past its ends where `wraps`.
  pub const fn with_wrap(mut self, wraps: bool) -> Self {
    self.wraps = wraps;

    self
  }

  /// This collection, paging by `page` members.
  pub const fn with_page(mut self, page: usize) -> Self {
    self.page = Some(page);

    self
  }

  /// This collection, a member moved to only where `enabled` says.
  pub const fn with_enabled(mut self, enabled: &'a dyn Fn(usize) -> bool) -> Self {
    self.enabled = enabled;

    self
  }

  /// The next enabled member from `from` (exclusive) going forward or backward, wrapping
  /// where the collection wraps; from before the first (or after the last) where `from` is
  /// `None`.
  #[must_use]
  pub fn step(&self, from: Option<usize>, forward: bool) -> Option<usize> {
    if self.count == 0 {
      return None;
    }

    let count = self.count;
    let mut at = from;

    for _ in 0..count {
      let next = match (at, forward) {
        (None, true) => 0,
        (None, false) => count - 1,
        (Some(at), true) if at + 1 < count => at + 1,
        (Some(_), true) if self.wraps => 0,
        (Some(at), false) if at > 0 => at - 1,
        (Some(_), false) if self.wraps => count - 1,
        (Some(_), _) => return None,
      };

      if (self.enabled)(next) {
        return Some(next);
      }

      at = Some(next);
    }

    None
  }

  /// The first enabled member, or the last where `!first`.
  #[must_use]
  pub fn end(&self, first: bool) -> Option<usize> {
    Self {
      wraps: false,
      ..*self
    }
    .step(None, first)
  }

  /// The member a page moves to from the current one, forward or backward: the farthest
  /// enabled member within a page, else the end.
  #[must_use]
  pub fn paged(&self, page: usize, forward: bool) -> Option<usize> {
    let Some(current) = self.current else {
      return self.end(forward);
    };
    let target = if forward {
      (current + page).min(self.count.saturating_sub(1))
    } else {
      current.saturating_sub(page)
    };
    let mut at = target;

    loop {
      if (self.enabled)(at) {
        return (at != current).then_some(at);
      }

      if at == current {
        return None;
      }

      at = if forward { at - 1 } else { at + 1 };
    }
  }

  /// A key pressed while the collection (or its current member) holds focus.
  pub fn key(&self, event: &KeyEvent) -> Response<usize> {
    if !event.down || !plain(event) || event.modifiers.shift {
      return Response::IGNORED;
    }

    let vertical = matches!(self.orientation, Orientation::Vertical | Orientation::Both);
    let horizontal = matches!(
      self.orientation,
      Orientation::Horizontal | Orientation::Both
    );
    let target = match event.key.as_str() {
      "ArrowDown" if vertical => self.step(self.current, true),
      "ArrowUp" if vertical => self.step(self.current, false),
      "ArrowRight" if horizontal => self.step(self.current, true),
      "ArrowLeft" if horizontal => self.step(self.current, false),
      "Home" if self.page.is_some() => self.end(true),
      "End" if self.page.is_some() => self.end(false),
      "PageDown" => match self.page {
        Some(page) => self.paged(page, true),
        None => return Response::IGNORED,
      },
      "PageUp" => match self.page {
        Some(page) => self.paged(page, false),
        None => return Response::IGNORED,
      },
      _ => return Response::IGNORED,
    };

    Response::maybe(target.filter(|target| Some(*target) != self.current))
  }
}
