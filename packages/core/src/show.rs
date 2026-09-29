//! A child built once and shown or hidden without rebuilding.

use std::marker::PhantomData;

use skene_reactive::{Effect, on_cleanup};

use super::{AnyKind, AnyView, IntoView, NoChild, NodeId, Platform, Scene, View};

/// Shows or hides one child, preserving its state: the child is built once and
/// attached or detached between a pair of anchors when the condition changes, and left
/// where it stands while it holds.
///
/// Holds [`NoChild`] until `set_child` supplies the child; only then is it a view.
#[must_use]
pub struct Show<P: Platform, C = NoChild> {
  shown: Box<dyn Fn() -> bool>,
  child: C,
  platform: PhantomData<fn() -> P>,
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

impl<P: Platform, C> Show<P, C> {
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
    let (start, _) = Scene::<P>::with(Scene::create_segment);
    let node = self.child.build();
    let shown = self.shown;

    on_cleanup(move || Scene::<P>::with(|scene| scene.destroy(node)));

    Effect::render(move |previous: Option<bool>| {
      let visible = shown();

      if previous != Some(visible) {
        Scene::<P>::with(|scene| {
          if visible {
            scene.insert_in_segment(start, node, None);
          }
          else {
            scene.remove_from_segment(start, node);
          }
        });
      }

      visible
    });

    start
  }
}

#[cfg(test)]
mod tests {
  use skene_reactive::{Get, Owner, Set, signal};

  use super::Show;
  use crate::{
    Scene,
    View,
    testing::{TestBox, TestPlatform, children},
  };

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
