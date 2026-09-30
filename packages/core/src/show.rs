//! A child built once and shown or hidden without rebuilding.

use std::{cell::Cell, marker::PhantomData};

use skene_reactive::{Effect, Owner, untrack};

use super::{AnyKind, AnyView, IntoView, NoChild, NodeId, Piece, Platform, Segment, View};

/// Shows or hides one child, preserving its state.
///
/// The child is built the first time it shows, then attached or detached between a
/// pair of anchors when the condition changes, and left where it stands while it
/// holds. While hidden its effects pause, catching up once it shows again.
///
/// Holds [`NoChild`] until `set_child` supplies the child; only then is it a view.
#[must_use]
pub struct Show<P: Platform, C = NoChild> {
  shown: Box<dyn Fn() -> bool>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform, C> std::fmt::Debug for Show<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Show").finish_non_exhaustive()
  }
}

impl<P: Platform> Show<P> {
  /// A `Show` with no child yet, shown until told otherwise.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      shown: Box::new(|| true),
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Default for Show<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> Show<P> {
  /// Sets whether the child is shown.
  #[inline(always)]
  pub fn set_shown(mut self, shown: bool) -> Self {
    self.shown = Box::new(move || shown);

    self
  }

  /// Shows the child while `shown` returns `true`, re-evaluated whenever a signal it
  /// reads changes.
  #[inline(always)]
  pub fn derive_shown(mut self, shown: impl Fn() -> bool + 'static) -> Self {
    self.shown = Box::new(shown);

    self
  }

  /// Sets the child.
  #[inline(always)]
  pub fn set_child(self, child: impl IntoView<P>) -> Show<P, AnyView<P>> {
    Show {
      shown: self.shown,
      child: child.into_view(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform, C> AnyKind for Show<P, C> {}

impl<P: Platform> View<P> for Show<P, AnyView<P>> {
  fn build(self) -> NodeId {
    let segment = Segment::<P>::new();
    let start = segment.start();
    let scope = Owner::current().child();
    let shown = self.shown;
    let child = Cell::new(Some(self.child));
    let piece = Cell::new(None::<Piece>);

    Effect::render(move |previous: Option<bool>| {
      let visible = shown();

      if previous == Some(visible) {
        return visible;
      }

      match (visible, piece.get()) {
        (true, Some(held)) => {
          segment.mount(held);
          held.resume();
        }
        (true, None) => {
          if let Some(child) = child.take() {
            let built = segment.build(scope, || untrack(|| child.build()));

            piece.set(Some(built));
            segment.mount(built);
          }
        }
        (false, Some(shown)) => {
          shown.pause();
          segment.unmount(shown);
        }
        (false, None) => {}
      }

      visible
    });

    start
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use skene_reactive::{Effect, Get, Owner, Set, signal};

  use super::Show;
  use crate::{
    AnyView,
    Scene,
    View,
    testing::{TestBox, TestPlatform, children},
  };

  /// A child hidden from the start is built the first time it shows, and while hidden
  /// its effects pause, catching up once, when it shows again.
  #[test]
  fn a_hidden_child_builds_when_first_shown_and_pauses_while_hidden() {
    TestPlatform::reset();

    let owner = Owner::new();
    let (shown, set_shown) = signal(false);
    let (source, set_source) = signal(0);
    let builds = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));

    let _ = owner.enter(|| {
      let builds = Rc::clone(&builds);
      let runs = Rc::clone(&runs);

      Show::<TestPlatform>::new()
        .derive_shown(move || shown.get())
        .set_child(AnyView::new(move || {
          let runs = Rc::clone(&runs);

          builds.set(builds.get() + 1);
          Effect::render(move |_: Option<()>| {
            let _ = source.get();

            runs.set(runs.get() + 1);
          });

          TestBox::node()
        }))
        .build()
    });

    assert_eq!(builds.get(), 0, "a hidden child is not built");

    set_shown.set(true);

    assert_eq!((builds.get(), runs.get()), (1, 1));

    set_shown.set(false);
    set_source.set(1);
    set_source.set(2);

    assert_eq!(runs.get(), 1, "a hidden child's effects pause");

    set_shown.set(true);

    assert_eq!(
      (builds.get(), runs.get()),
      (1, 2),
      "and catch up once, when it shows"
    );

    owner.dispose();
  }

  /// A re-run that leaves the condition as it was leaves the child where it stands.
  #[test]
  fn an_unchanged_condition_leaves_the_child_alone() {
    TestPlatform::reset();

    let owner = Owner::new();
    let (count, set_count) = signal(1);
    let parent = TestBox::node();
    let start = owner.enter(|| {
      Show::<TestPlatform>::new()
        .derive_shown(move || count.get() > 0)
        .set_child(TestBox::new())
        .build()
    });

    Scene::<TestPlatform>::with(|scene| scene.attach(parent, start, None));
    TestPlatform::take_log();
    set_count.set(2);

    assert_eq!(TestPlatform::take_log(), []);

    owner.dispose();
  }

  /// Hiding detaches the child and showing reattaches the same node, never rebuilding.
  #[test]
  fn show_detaches_and_reattaches_the_same_node() {
    TestPlatform::reset();

    let owner = Owner::new();
    let (shown, set_shown) = signal(true);
    let parent = TestBox::node();
    let start = owner.enter(|| {
      Show::<TestPlatform>::new()
        .derive_shown(move || shown.get())
        .set_child(TestBox::new())
        .build()
    });

    Scene::<TestPlatform>::with(|scene| scene.attach(parent, start, None));

    let end = Scene::<TestPlatform>::with(|scene| scene.segment_end(start)).unwrap();
    let child = children(parent)[1];

    assert_eq!(children(parent), [start, child, end]);

    set_shown.set(false);

    assert_eq!(children(parent), [start, end]);
    assert!(Scene::<TestPlatform>::with(|scene| scene
      .record(child)
      .is_some()));

    set_shown.set(true);

    assert_eq!(children(parent), [start, child, end]);

    set_shown.set(false);
    owner.dispose();

    assert!(Scene::<TestPlatform>::with(|scene| scene
      .record(child)
      .is_none()));
  }
}
