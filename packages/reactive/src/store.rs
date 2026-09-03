//! Fine-grained reactive stores: one plain value, field- and key-level subscriptions.
//!
//! A [`Store`] owns a single value of a plain type—methods on it keep working, and a
//! tagged store snapshots as one unit. `#[derive(Store)]` on the type generates an
//! accessor per field returning a lens into the store; reading through a lens
//! subscribes the running effect to exactly that location, and a write notifies
//! exactly the readers it can affect: the location itself, the locations containing
//! it, and everything beneath it. `Vec` fields address elements by position
//! ([`IndexedField::at`]); keyed collections address entries by key
//! ([`KeyedField::key`]).

use std::{
  collections::{BTreeMap, HashMap},
  hash::{BuildHasher, Hash, Hasher},
  marker::PhantomData,
};

use rustc_hash::{FxHashMap, FxHasher};

use crate::{Owner, Signal, Trigger, access::With, effect::batch, signal::update_silently};

/// One step of a store path: a struct field by declaration order, or a collection
/// entry by its position or hashed key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathSegment {
  /// A named field, by declaration order.
  Field(u32),
  /// A collection entry, by its index or its key's hash.
  Key(u64),
}

/// The address of a location inside a store's value, from the root.
pub type StorePath = Vec<PathSegment>;

/// A store's shared plumbing: the trigger registry and the owning owner.
///
/// Every lens of one store carries the same core; custom lenses only forward it.
#[derive(Clone, Copy)]
#[must_use]
pub struct StoreCore {
  triggers: Signal<FxHashMap<StorePath, Trigger>>,
  owner: Owner,
}

impl StoreCore {
  fn new() -> Self {
    Self {
      triggers: Signal::new(FxHashMap::default()),
      owner: Owner::current(),
    }
  }

  /// Subscribes the running effect to the location at `path`.
  fn track(&self, path: &[PathSegment]) {
    let existing = self
      .triggers
      .try_with_untracked(|triggers| triggers.get(path).copied())
      .flatten();

    let trigger = match existing {
      Some(trigger) => trigger,
      None => {
        let trigger = self.owner.enter(Trigger::new);

        let _ = update_silently(
          self.triggers.id(),
          |triggers: &mut FxHashMap<StorePath, Trigger>| {
            triggers.insert(path.to_vec(), trigger);
          },
        );

        trigger
      }
    };

    trigger.track();
  }

  /// Notifies every reader a write at `path` can affect: the location itself, the
  /// locations containing it, and everything beneath it.
  fn notify(&self, path: &[PathSegment]) {
    let affected: Vec<Trigger> = self
      .triggers
      .try_with_untracked(|triggers| {
        triggers
          .iter()
          .filter(|(tracked, _)| tracked.starts_with(path) || path.starts_with(tracked))
          .map(|(_, trigger)| *trigger)
          .collect()
      })
      .unwrap_or_default();

    batch(|| {
      for trigger in affected {
        trigger.notify();
      }
    });
  }
}

/// Fine-grained reactive state over one plain value.
///
/// Created with [`new`](Self::new) (or [`tagged`](Self::tagged) to join state
/// snapshots). The store itself is the root location—[`StoreField`] gives it and every
/// lens the same read/write surface—and `#[derive(Store)]` on the value's type
/// generates the per-field accessors that descend into it.
#[must_use]
pub struct Store<T: 'static> {
  value: Signal<T>,
  core: StoreCore,
}

impl<T: 'static> Clone for Store<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: 'static> Copy for Store<T> {}

impl<T: 'static> Store<T> {
  /// Creates a store owned by the current [`Owner`].
  pub fn new(value: T) -> Self {
    Self {
      value: Signal::new(value),
      core: StoreCore::new(),
    }
  }
}

#[cfg(feature = "snapshot")]
impl<T: serde::Serialize + serde::de::DeserializeOwned + 'static> Store<T> {
  /// Creates a store whose `tag` is its identity in state snapshots.
  ///
  /// The whole value serializes as one unit under the tag, exactly as
  /// [`Signal::tagged`] does for a single value: the store starts from the value
  /// staged under its tag by [`restore`](crate::restore) when one is present and
  /// decodes as `T`, and from `value` otherwise.
  ///
  /// # Panics
  ///
  /// Panics if `tag` is already carried by a live tagged signal or store.
  pub fn tagged(tag: &'static str, value: T) -> Self {
    let value = crate::snapshot::take_staged(tag)
      .and_then(|bytes| postcard::from_bytes(&bytes).ok())
      .unwrap_or(value);
    let store = Self::new(value);
    let cell = store.value;

    crate::snapshot::register(
      tag,
      Box::new(move || {
        cell
          .try_with_untracked(|value| postcard::to_allocvec(value).ok())
          .flatten()
      }),
    );

    store
  }
}

/// A location inside a store: the store itself, or a lens into part of its value.
///
/// Every location shares one surface: [`get`](Self::get)/[`with`](Self::with)
/// subscribe the running effect to exactly this location,
/// [`set`](Self::set)/[`update`](Self::update) notify exactly the readers the write
/// can affect. The `try_` variants return `None` where the plain calls panic: when
/// the location no longer exists (a removed key, an out-of-range index).
pub trait StoreField: Clone {
  /// The addressed value's type.
  type Value: 'static;

  /// The owning store's shared plumbing.
  fn core(&self) -> StoreCore;

  /// Appends this location's address onto `path`.
  fn write_path(&self, path: &mut StorePath);

  /// Runs `read` on the addressed value without subscribing; `None` when the location
  /// no longer exists.
  fn read_untracked<R>(&self, read: impl FnOnce(&Self::Value) -> R) -> Option<R>;

  /// Runs `write` on the addressed value without notifying; `None` when the location
  /// no longer exists.
  fn write_untracked<R>(&self, write: impl FnOnce(&mut Self::Value) -> R) -> Option<R>;

  /// This location's address from the store's root.
  fn path(&self) -> StorePath {
    let mut path = StorePath::new();

    self.write_path(&mut path);

    path
  }

  /// Subscribes the running effect to this location.
  fn track(&self) {
    self.core().track(&self.path());
  }

  /// Runs `read` on the value, subscribed; `None` when the location no longer exists.
  fn try_with<R>(&self, read: impl FnOnce(&Self::Value) -> R) -> Option<R> {
    self.track();

    self.read_untracked(read)
  }

  /// Runs `read` on the value, subscribed.
  ///
  /// # Panics
  ///
  /// Panics when the location no longer exists; see [`try_with`](Self::try_with).
  fn with<R>(&self, read: impl FnOnce(&Self::Value) -> R) -> R {
    self
      .try_with(read)
      .expect("store location no longer exists")
  }

  /// The value, cloned, subscribed; `None` when the location no longer exists.
  #[must_use]
  fn try_get(&self) -> Option<Self::Value>
  where
    Self::Value: Clone,
  {
    self.try_with(Clone::clone)
  }

  /// The value, cloned, subscribed.
  ///
  /// # Panics
  ///
  /// Panics when the location no longer exists; see [`try_get`](Self::try_get).
  #[must_use]
  fn get(&self) -> Self::Value
  where
    Self::Value: Clone,
  {
    self.try_get().expect("store location no longer exists")
  }

  /// Mutates the value through `write`, notifying affected readers; `None`—and no
  /// notification—when the location no longer exists.
  fn try_update<R>(&self, write: impl FnOnce(&mut Self::Value) -> R) -> Option<R> {
    let result = self.write_untracked(write)?;

    self.core().notify(&self.path());

    Some(result)
  }

  /// Mutates the value through `write`, notifying affected readers.
  ///
  /// # Panics
  ///
  /// Panics when the location no longer exists; see [`try_update`](Self::try_update).
  fn update(&self, write: impl FnOnce(&mut Self::Value)) {
    self
      .try_update(write)
      .expect("store location no longer exists");
  }

  /// Replaces the value, notifying affected readers.
  ///
  /// # Panics
  ///
  /// Panics when the location no longer exists; see [`try_update`](Self::try_update).
  fn set(&self, value: Self::Value) {
    self.update(|current| *current = value);
  }
}

impl<T: 'static> StoreField for Store<T> {
  type Value = T;

  #[inline(always)]
  fn core(&self) -> StoreCore {
    self.core
  }

  #[inline(always)]
  fn write_path(&self, _path: &mut StorePath) {}

  fn read_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
    self.value.try_with_untracked(read)
  }

  fn write_untracked<R>(&self, write: impl FnOnce(&mut T) -> R) -> Option<R> {
    update_silently(self.value.id(), write)
  }
}

/// A lens onto one named field of the parent location's value; built by the accessors
/// `#[derive(Store)]` generates.
#[must_use]
pub struct Subfield<Parent: StoreField, T> {
  parent: Parent,
  ordinal: u32,
  read: fn(&Parent::Value) -> &T,
  write: fn(&mut Parent::Value) -> &mut T,
}

impl<Parent: StoreField, T> Subfield<Parent, T> {
  /// Builds a field lens: the field's declaration ordinal plus its two projections.
  #[inline(always)]
  pub fn new(
    parent: Parent,
    ordinal: u32,
    read: fn(&Parent::Value) -> &T,
    write: fn(&mut Parent::Value) -> &mut T,
  ) -> Self {
    Self {
      parent,
      ordinal,
      read,
      write,
    }
  }
}

impl<Parent: StoreField, T> Clone for Subfield<Parent, T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      parent: self.parent.clone(),
      ordinal: self.ordinal,
      read: self.read,
      write: self.write,
    }
  }
}

impl<Parent: StoreField + Copy, T> Copy for Subfield<Parent, T> {}

impl<Parent: StoreField, T: 'static> StoreField for Subfield<Parent, T> {
  type Value = T;

  #[inline(always)]
  fn core(&self) -> StoreCore {
    self.parent.core()
  }

  fn write_path(&self, path: &mut StorePath) {
    self.parent.write_path(path);
    path.push(PathSegment::Field(self.ordinal));
  }

  fn read_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
    self.parent.read_untracked(|value| read((self.read)(value)))
  }

  fn write_untracked<R>(&self, write: impl FnOnce(&mut T) -> R) -> Option<R> {
    self
      .parent
      .write_untracked(|value| write((self.write)(value)))
  }
}

/// A lens onto one element of the parent location's `Vec`, by position.
///
/// Positional: a structural change to the `Vec` shifts what it addresses, and such a
/// change notifies every element reader.
#[must_use]
pub struct AtIndex<Parent, T> {
  parent: Parent,
  index: usize,
  element: PhantomData<T>,
}

impl<Parent: Clone, T> Clone for AtIndex<Parent, T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      parent: self.parent.clone(),
      index: self.index,
      element: PhantomData,
    }
  }
}

impl<Parent: Copy, T> Copy for AtIndex<Parent, T> {}

impl<Parent: StoreField<Value = Vec<T>>, T: 'static> StoreField for AtIndex<Parent, T> {
  type Value = T;

  #[inline(always)]
  fn core(&self) -> StoreCore {
    self.parent.core()
  }

  fn write_path(&self, path: &mut StorePath) {
    self.parent.write_path(path);
    path.push(PathSegment::Key(self.index as u64));
  }

  fn read_untracked<R>(&self, read: impl FnOnce(&T) -> R) -> Option<R> {
    self
      .parent
      .read_untracked(|elements| elements.get(self.index).map(read))
      .flatten()
  }

  fn write_untracked<R>(&self, write: impl FnOnce(&mut T) -> R) -> Option<R> {
    self
      .parent
      .write_untracked(|elements| elements.get_mut(self.index).map(write))
      .flatten()
  }
}

/// Index addressing on a store location holding a `Vec`.
pub trait IndexedField<T>: StoreField<Value = Vec<T>> + Sized {
  /// The element at `index`, by position.
  fn at(self, index: usize) -> AtIndex<Self, T> {
    AtIndex {
      parent: self,
      index,
      element: PhantomData,
    }
  }
}

impl<S: StoreField<Value = Vec<T>>, T: 'static> IndexedField<T> for S {}

/// A collection a store addresses entry-by-entry, by key.
pub trait KeyedCollection: 'static {
  /// The key addressing an entry.
  type Key: Hash + Clone + 'static;
  /// The entry type.
  type Entry: 'static;

  /// The entry under `key`.
  fn entry(&self, key: &Self::Key) -> Option<&Self::Entry>;

  /// The entry under `key`, mutably.
  fn entry_mut(&mut self, key: &Self::Key) -> Option<&mut Self::Entry>;
}

impl<K, V, S> KeyedCollection for HashMap<K, V, S>
where
  K: Eq + Hash + Clone + 'static,
  V: 'static,
  S: BuildHasher + 'static,
{
  type Entry = V;
  type Key = K;

  fn entry(&self, key: &K) -> Option<&V> {
    self.get(key)
  }

  fn entry_mut(&mut self, key: &K) -> Option<&mut V> {
    self.get_mut(key)
  }
}

impl<K, V> KeyedCollection for BTreeMap<K, V>
where
  K: Ord + Hash + Clone + 'static,
  V: 'static,
{
  type Entry = V;
  type Key = K;

  fn entry(&self, key: &K) -> Option<&V> {
    self.get(key)
  }

  fn entry_mut(&mut self, key: &K) -> Option<&mut V> {
    self.get_mut(key)
  }
}

/// A lens onto one entry of the parent location's keyed collection.
#[must_use]
pub struct AtKey<Parent: StoreField>
where
  Parent::Value: KeyedCollection,
{
  parent: Parent,
  key: <Parent::Value as KeyedCollection>::Key,
}

impl<Parent: StoreField> Clone for AtKey<Parent>
where
  Parent::Value: KeyedCollection,
{
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      parent: self.parent.clone(),
      key: self.key.clone(),
    }
  }
}

impl<Parent: StoreField> StoreField for AtKey<Parent>
where
  Parent::Value: KeyedCollection,
{
  type Value = <Parent::Value as KeyedCollection>::Entry;

  #[inline(always)]
  fn core(&self) -> StoreCore {
    self.parent.core()
  }

  fn write_path(&self, path: &mut StorePath) {
    let mut hasher = FxHasher::default();

    self.key.hash(&mut hasher);
    self.parent.write_path(path);
    path.push(PathSegment::Key(hasher.finish()));
  }

  fn read_untracked<R>(&self, read: impl FnOnce(&Self::Value) -> R) -> Option<R> {
    self
      .parent
      .read_untracked(|entries| entries.entry(&self.key).map(read))
      .flatten()
  }

  fn write_untracked<R>(&self, write: impl FnOnce(&mut Self::Value) -> R) -> Option<R> {
    self
      .parent
      .write_untracked(|entries| entries.entry_mut(&self.key).map(write))
      .flatten()
  }
}

/// Key addressing on a store location holding a keyed collection.
pub trait KeyedField: StoreField + Sized
where
  Self::Value: KeyedCollection,
{
  /// The entry under `key`.
  fn key(self, key: <Self::Value as KeyedCollection>::Key) -> AtKey<Self> {
    AtKey { parent: self, key }
  }
}

impl<S: StoreField> KeyedField for S where S::Value: KeyedCollection {}
