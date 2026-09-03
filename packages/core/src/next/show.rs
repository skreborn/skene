//! A child built once and shown or hidden without rebuilding.

use std::marker::PhantomData;

use skene_reactive::{Effect, on_cleanup};

use super::{AnyKind, AnyView, Driver, IntoView, NoChild, NodeId, Scene, View};

/// Shows or hides one child, preserving its state: the child is built once and
/// attached or detached between a pair of anchors as the condition changes.
///
/// Holds [`NoChild`] until `set_child` supplies the child; only then is it a view.
#[must_use]
pub struct Show<D: Driver, C = NoChild> {
  shown: Box<dyn Fn() -> bool>,
  child: C,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> Show<D> {
  /// A `Show` with no child yet, shown until told otherwise.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      shown: Box::new(|| true),
      child: NoChild,
      driver: PhantomData,
    }
  }
}

impl<D: Driver> Default for Show<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver, C> Show<D, C> {
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
  pub fn set_child(self, child: impl IntoView<D>) -> Show<D, AnyView<D>> {
    Show {
      shown: self.shown,
      child: child.into_view(),
      driver: PhantomData,
    }
  }
}

impl<D: Driver, C> AnyKind for Show<D, C> {}

impl<D: Driver> View<D> for Show<D, AnyView<D>> {
  fn build(self) -> NodeId {
    let (start, _) = Scene::<D>::with(Scene::create_segment);
    let node = self.child.build();
    let shown = self.shown;

    on_cleanup(move || Scene::<D>::with(|scene| scene.destroy(node)));

    Effect::new(move |_: Option<()>| {
      let visible = shown();

      Scene::<D>::with(|scene| {
        if visible {
          scene.insert_in_segment(start, node, None);
        }
        else {
          scene.remove_from_segment(start, node);
        }
      });
    });

    start
  }
}

#[cfg(test)]
mod tests {
  use skene_reactive::{Get, Owner, Set, signal};

  use super::Show;
  use crate::next::{
    Scene,
    View,
    testing::{TestBox, TestDriver, children},
  };

  /// Hiding detaches the child and showing reattaches the same node, never rebuilding.
  #[test]
  fn show_detaches_and_reattaches_the_same_node() {
    TestDriver::reset();

    let owner = Owner::new();
    let (shown, set_shown) = signal(true);
    let parent = TestBox::node();
    let start = owner.enter(|| {
      Show::<TestDriver>::new()
        .derive_shown(move || shown.get())
        .set_child(TestBox::new())
        .build()
    });

    Scene::<TestDriver>::with(|scene| scene.attach(parent, start, None));

    let end = Scene::<TestDriver>::with(|scene| scene.segment_end(start)).unwrap();
    let child = children(parent)[1];

    assert_eq!(children(parent), [start, child, end]);

    set_shown.set(false);

    assert_eq!(children(parent), [start, end]);
    assert!(Scene::<TestDriver>::with(|scene| scene
      .record(child)
      .is_some()));

    set_shown.set(true);

    assert_eq!(children(parent), [start, child, end]);

    set_shown.set(false);
    owner.dispose();

    assert!(Scene::<TestDriver>::with(|scene| scene
      .record(child)
      .is_none()));
  }
}
