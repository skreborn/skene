//! A container's children: a list of segments, each mounted between its own anchors.

use std::{hash::Hash, marker::PhantomData};

use skene_reactive::{AtKey, KeyedField};

use super::{
  AnyView,
  IntoChildOf,
  NodeId,
  PlainChild,
  Realization,
  Scene,
  View,
  derived,
  reconcile,
};

/// A value that can be converted into children of kind `Kind`: a tuple of up to twelve
/// children, an array, or a `Vec`.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a set of children",
  label = "not a set of children",
  note = "pass a tuple, `Vec`, or an array of children"
)]
pub trait IntoChildren<P: Realization, Kind = PlainChild> {
  /// Converts this value into children.
  fn into_children(self) -> Vec<AnyView<P>>;
}

impl<P: Realization, Kind, V: IntoChildOf<P, Kind>> IntoChildren<P, Kind> for Vec<V> {
  fn into_children(self) -> Vec<AnyView<P>> {
    self.into_iter().map(IntoChildOf::into_child).collect()
  }
}

impl<P: Realization, Kind, V: IntoChildOf<P, Kind>, const N: usize> IntoChildren<P, Kind>
  for [V; N]
{
  fn into_children(self) -> Vec<AnyView<P>> {
    self.into_iter().map(IntoChildOf::into_child).collect()
  }
}

macro_rules! impl_tuple {
  ($($T:ident),+) => {
    impl<P: Realization, Kind, $($T: IntoChildOf<P, Kind>),+> IntoChildren<P, Kind> for ($($T,)+) {
      fn into_children(self) -> Vec<AnyView<P>> {
        #[expect(non_snake_case, clippy::min_ident_chars, reason = "each binding takes its element's type parameter name")]
        let ($($T,)+) = self;

        vec![$($T.into_child()),+]
      }
    }
  };
}

impl_tuple!(A);
impl_tuple!(A, B);
impl_tuple!(A, B, C);
impl_tuple!(A, B, C, D2);
impl_tuple!(A, B, C, D2, E);
impl_tuple!(A, B, C, D2, E, F);
impl_tuple!(A, B, C, D2, E, F, G);
impl_tuple!(A, B, C, D2, E, F, G, H);
impl_tuple!(A, B, C, D2, E, F, G, H, I);
impl_tuple!(A, B, C, D2, E, F, G, H, I, J);
impl_tuple!(A, B, C, D2, E, F, G, H, I, J, K);
impl_tuple!(A, B, C, D2, E, F, G, H, I, J, K, L);

/// A container's children of kind `Kind`: segments in order, each mounted between its
/// own pair of anchors so a static run and a keyed list sit side by side under one
/// parent.
#[must_use]
pub struct Children<P: Realization, Kind = PlainChild> {
  segments: Vec<ChildSegment<P>>,
  kind: PhantomData<fn() -> Kind>,
}

impl<P: Realization, Kind> std::fmt::Debug for Children<P, Kind> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Children").finish_non_exhaustive()
  }
}

enum ChildSegment<P: Realization> {
  Static(Vec<AnyView<P>>),
  Keyed(Box<dyn FnOnce(NodeId)>),
}

impl<P: Realization, Kind: 'static> Default for Children<P, Kind> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization, Kind: 'static> Children<P, Kind> {
  /// No children yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      segments: Vec::new(),
      kind: PhantomData,
    }
  }

  /// Whether no segment has been pushed.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.segments.is_empty()
  }

  /// Removes every segment pushed so far.
  #[inline(always)]
  pub fn clear(&mut self) {
    self.segments.clear();
  }

  /// Appends a static segment.
  pub fn push_static(&mut self, children: impl IntoChildren<P, Kind>) {
    self
      .segments
      .push(ChildSegment::Static(children.into_children()));
  }

  /// Appends a segment rebuilt whenever the signals `child` reads change.
  pub fn push_derived<V: IntoChildOf<P, Kind>>(&mut self, child: impl Fn() -> V + 'static) {
    self
      .segments
      .push(ChildSegment::Static(vec![AnyView::new(move || {
        derived::<P>(move || child().into_child().build())
      })]));
  }

  /// Appends a keyed segment: `items` yields the items, `key` gives each a stable
  /// identity, `view` renders one; see [`reconcile`].
  pub fn push_keyed<T, I, K, V>(
    &mut self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) where
    I: IntoIterator<Item = T>,
    K: Eq + Hash + 'static,
    V: IntoChildOf<P, Kind>,
  {
    self
      .segments
      .push(ChildSegment::Keyed(Box::new(move |start| {
        reconcile::<P, _, _, _>(start, items, key, move |item| {
          view(item).into_child().build()
        });
      })));
  }

  /// Mounts every segment under `parent`, in order.
  pub fn mount(self, parent: NodeId) {
    for segment in self.segments {
      let (start, _) = Scene::<P>::create_segment();

      Scene::<P>::with(|scene| scene.attach(parent, start, None));

      match segment {
        ChildSegment::Static(views) => {
          for view in views {
            let node = view.build();

            Scene::<P>::with(|scene| scene.insert_in_segment(start, node, None));
          }
        }
        ChildSegment::Keyed(install) => install(start),
      }
    }
  }
}

/// Proof that a call came from core; only the public setters of [`HasChild`] and
/// [`HasChildren`] hold one.
#[derive(Debug)]
#[must_use]
pub struct Seal(());

/// A view holding exactly one child, which each setter replaces.
///
/// One method is required and unreachable from outside core: the public setters are
/// the defaults.
pub trait HasChild<P: Realization>: Sized {
  /// The child kind this view accepts; wrapper views carry per-child properties only
  /// the matching view understands.
  type Kind: 'static;

  /// The child slot.
  fn child_mut(&mut self, seal: Seal) -> &mut Children<P, Self::Kind>;

  /// Sets the child, replacing the one set before.
  #[must_use]
  fn set_child(mut self, child: impl IntoChildOf<P, Self::Kind>) -> Self {
    let slot = self.child_mut(Seal(()));

    slot.clear();
    slot.push_static([child.into_child()]);

    self
  }

  /// Sets a child rebuilt whenever the signals `child` reads change, replacing the one
  /// set before.
  #[must_use]
  fn derive_child<V: IntoChildOf<P, Self::Kind>>(
    mut self,
    child: impl Fn() -> V + 'static,
  ) -> Self {
    let slot = self.child_mut(Seal(()));

    slot.clear();
    slot.push_derived(child);

    self
  }
}

/// A container that has children.
///
/// One method is required and unreachable from outside core: the public setters are
/// the defaults, and each appends a segment.
pub trait HasChildren<P: Realization>: Sized {
  /// The child kind this container accepts; wrapper views carry per-child properties
  /// only the matching container understands.
  type Kind: 'static;

  /// The children collected so far.
  fn children_mut(&mut self, seal: Seal) -> &mut Children<P, Self::Kind>;

  /// Appends one static child.
  #[must_use]
  fn add_child(mut self, child: impl IntoChildOf<P, Self::Kind>) -> Self {
    self
      .children_mut(Seal(()))
      .push_static([child.into_child()]);

    self
  }

  /// Appends a static set of children.
  #[must_use]
  fn set_children(mut self, children: impl IntoChildren<P, Self::Kind>) -> Self {
    self.children_mut(Seal(())).push_static(children);

    self
  }

  /// Appends one child rebuilt whenever the signals `child` reads change.
  #[must_use]
  fn derive_child<V: IntoChildOf<P, Self::Kind>>(
    mut self,
    child: impl Fn() -> V + 'static,
  ) -> Self {
    self.children_mut(Seal(())).push_derived(child);

    self
  }

  /// Appends a keyed set of children: `items` yields the items, `key` gives each a
  /// stable identity, `view` renders one.
  #[must_use]
  fn derive_children<T, I, K, V>(
    mut self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + Hash + 'static,
    V: IntoChildOf<P, Self::Kind>,
  {
    self.children_mut(Seal(())).push_keyed(items, key, view);

    self
  }

  /// Appends one child per entry of a keyed store location, keyed by the entry's key:
  /// `view` renders one from the lens onto its entry, and the set follows the
  /// location's shape; see [`KeyedField::entries`].
  #[must_use]
  fn derive_entries<F, V>(mut self, entries: F, view: impl Fn(AtKey<F>) -> V + 'static) -> Self
  where
    F: KeyedField,
    V: IntoChildOf<P, Self::Kind>,
  {
    self
      .children_mut(Seal(()))
      .push_keyed(move || entries.entries(), AtKey::entry_key, view);

    self
  }
}
