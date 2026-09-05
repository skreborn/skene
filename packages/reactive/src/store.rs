//! Fine-grained reactive stores: one plain value, field- and key-level subscriptions.
//!
//! A [`Store`] owns a single value of a plain type—methods on it keep working, and the
//! whole value snapshots as one unit. `#[derive(Store)]` on the type generates an
//! accessor per field returning a lens into the store; reading through a lens
//! subscribes the running effect to exactly that location, and a write notifies
//! exactly the readers it can affect: the location itself, the locations containing
//! it, and everything beneath it. `Vec` fields address elements by position
//! ([`IndexedField::at`]); keyed collections address entries by key
//! ([`KeyedField::key`]).
//!
//! Locations intern into a trie of [`StoreLocation`]s, one node per location ever
//! addressed, so tracking and notifying cost the location's depth and nothing of the
//! number of locations tracked.

use std::{
  any::Any,
  collections::{BTreeMap, HashMap},
  hash::{BuildHasher, Hash, Hasher},
  marker::PhantomData,
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHasher};
use serde::{Serialize, de::DeserializeOwned};
use smallvec::SmallVec;

use crate::{Owner, Signal, Trigger, access::With, effect::batch, signal::update_silently};

/// An interned location inside a store's value: the root, or one path of fields,
/// indexes and keys beneath it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct StoreLocation(u32);

impl StoreLocation {
  /// The store's own value.
  pub const ROOT: Self = Self(0);
}

/// A key compared exactly against another of any type.
trait DynKey: Any {
  fn eq_dyn(&self, other: &dyn Any) -> bool;
}

impl<K: Eq + 'static> DynKey for K {
  fn eq_dyn(&self, other: &dyn Any) -> bool {
    other.downcast_ref::<K>() == Some(self)
  }
}

/// The step from a location to one child.
enum Edge {
  Field(u32),
  Index(usize),
  Key(Rc<dyn DynKey>),
}

/// The absent parent, on the root.
const NO_PARENT: u32 = u32::MAX;

/// One location: its parent and its two triggers, created on first use.
///
/// `below` wakes on a write at this location or beneath it; `here` wakes on a write
/// exactly at this location. A reader tracks its own `below` and every ancestor's
/// `here`; a write notifies its own `here` and the `below` of itself and every
/// ancestor.
struct Node {
  parent: u32,
  here: Option<Trigger>,
  below: Option<Trigger>,
}

/// The children of one location sharing an edge hash, with their node ids.
type Bucket = SmallVec<[(Edge, u32); 1]>;

/// The trie of addressed locations.
struct Trie {
  nodes: Vec<Node>,
  edges: FxHashMap<(u32, u64), Bucket>,
}

impl Trie {
  fn new() -> Self {
    Self {
      nodes: vec![Node {
        parent: NO_PARENT,
        here: None,
        below: None,
      }],
      edges: FxHashMap::default(),
    }
  }

  /// The child of `parent` along the edge `matches` recognizes, created by `make`
  /// when absent.
  fn child(
    &mut self,
    parent: u32,
    hash: u64,
    matches: impl Fn(&Edge) -> bool,
    make: impl FnOnce() -> Edge,
  ) -> u32 {
    let next = u32::try_from(self.nodes.len()).expect("store locations exhausted");
    let bucket = HashMap::entry(&mut self.edges, (parent, hash)).or_default();

    if let Some((_, node)) = bucket.iter().find(|(edge, _)| matches(edge)) {
      return *node;
    }

    bucket.push((make(), next));
    self.nodes.push(Node {
      parent,
      here: None,
      below: None,
    });

    next
  }
}

/// The bucket hash of one edge kind and its key.
fn edge_hash(kind: u8, key: impl Hash) -> u64 {
  let mut hasher = FxHasher::default();

  kind.hash(&mut hasher);
  key.hash(&mut hasher);

  hasher.finish()
}

/// A store's shared plumbing: the location trie and the owning owner.
///
/// Every lens of one store carries the same core; custom lenses only forward it and
/// intern their locations through it.
#[derive(Clone, Copy)]
#[must_use]
pub struct StoreCore {
  trie: Signal<Trie>,
  owner: Owner,
}

impl StoreCore {
  fn new() -> Self {
    Self {
      trie: Signal::opaque(Trie::new()),
      owner: Owner::current(),
    }
  }

  /// The location of the field keyed `key` under `parent`.
  pub fn field(&self, parent: StoreLocation, key: u32) -> StoreLocation {
    self.intern(
      parent,
      edge_hash(0, key),
      |edge| matches!(edge, Edge::Field(existing) if *existing == key),
      || Edge::Field(key),
    )
  }

  /// The location of the element at `index` under `parent`.
  pub fn index(&self, parent: StoreLocation, index: usize) -> StoreLocation {
    self.intern(
      parent,
      edge_hash(1, index),
      |edge| matches!(edge, Edge::Index(existing) if *existing == index),
      || Edge::Index(index),
    )
  }

  /// The location of the entry under `key` beneath `parent`; distinct keys never
  /// share a location.
  pub fn key<K: Hash + Eq + Clone + 'static>(
    &self,
    parent: StoreLocation,
    key: &K,
  ) -> StoreLocation {
    self.intern(
      parent,
      edge_hash(2, key),
      |edge| matches!(edge, Edge::Key(existing) if existing.eq_dyn(key)),
      || Edge::Key(Rc::new(key.clone())),
    )
  }

  fn intern(
    &self,
    parent: StoreLocation,
    hash: u64,
    matches: impl Fn(&Edge) -> bool,
    make: impl FnOnce() -> Edge,
  ) -> StoreLocation {
    update_silently(self.trie.id(), |trie: &mut Trie| {
      trie.child(parent.0, hash, matches, make)
    })
    .map_or(StoreLocation::ROOT, StoreLocation)
  }

  /// Subscribes the running effect to `location`.
  fn track(&self, location: StoreLocation) {
    let owner = self.owner;
    let triggers: SmallVec<[Trigger; 8]> = update_silently(self.trie.id(), |trie: &mut Trie| {
      owner.enter(|| {
        let mut triggers = SmallVec::new();
        let mut node = location.0 as usize;

        triggers.push(*trie.nodes[node].below.get_or_insert_with(Trigger::new));

        while trie.nodes[node].parent != NO_PARENT {
          node = trie.nodes[node].parent as usize;

          triggers.push(*trie.nodes[node].here.get_or_insert_with(Trigger::new));
        }

        triggers
      })
    })
    .unwrap_or_default();

    for trigger in triggers {
      trigger.track();
    }
  }

  /// Notifies every reader a write at `location` can affect: the location itself, the
  /// locations containing it, and everything beneath it.
  fn notify(&self, location: StoreLocation) {
    let triggers: SmallVec<[Trigger; 8]> = self
      .trie
      .try_with_untracked(|trie| {
        let mut triggers = SmallVec::new();
        let mut node = location.0 as usize;

        triggers.extend(trie.nodes[node].here);

        loop {
          triggers.extend(trie.nodes[node].below);

          let parent = trie.nodes[node].parent;

          if parent == NO_PARENT {
            break;
          }

          node = parent as usize;
        }

        triggers
      })
      .unwrap_or_default();

    batch(|| {
      for trigger in triggers {
        trigger.notify();
      }
    });
  }
}

/// Fine-grained reactive state over one plain value.
///
/// Created with [`new`](Self::new), [`tagged`](Self::tagged) or
/// [`opaque`](Self::opaque), which take part in state snapshots exactly as the
/// [`Signal`] constructors of the same names do. The store itself is the root
/// location—[`StoreField`] gives it and every lens the same read/write surface—and
/// `#[derive(Store)]` on the value's type generates the per-field accessors that
/// descend into it.
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

impl<T: Serialize + DeserializeOwned + 'static> Store<T> {
  /// Creates a store owned by the current [`Owner`]; the whole value is one snapshot
  /// signal at the owner's next position, see [`Signal::new`].
  pub fn new(value: T) -> Self {
    Self::over(Signal::new(value))
  }

  /// Creates a store whose `tag` names its position in state snapshots; see
  /// [`Signal::tagged`].
  ///
  /// # Panics
  ///
  /// Panics if `tag` is already carried by a live tagged signal of the same owner.
  pub fn tagged(tag: &'static str, value: T) -> Self {
    Self::over(Signal::tagged(tag, value))
  }
}

impl<T: 'static> Store<T> {
  /// Creates a store owned by the current [`Owner`] that takes no part in state
  /// snapshots; see [`Signal::opaque`].
  pub fn opaque(value: T) -> Self {
    Self::over(Signal::opaque(value))
  }

  fn over(value: Signal<T>) -> Self {
    Self {
      value,
      core: StoreCore::new(),
    }
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

  /// This location, interned in the store's trie.
  fn location(&self) -> StoreLocation;

  /// Runs `read` on the addressed value without subscribing; `None` when the location
  /// no longer exists.
  fn read_untracked<R>(&self, read: impl FnOnce(&Self::Value) -> R) -> Option<R>;

  /// Runs `write` on the addressed value without notifying; `None` when the location
  /// no longer exists.
  fn write_untracked<R>(&self, write: impl FnOnce(&mut Self::Value) -> R) -> Option<R>;

  /// Subscribes the running effect to this location.
  fn track(&self) {
    self.core().track(self.location());
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

    self.core().notify(self.location());

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
  fn location(&self) -> StoreLocation {
    StoreLocation::ROOT
  }

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
  key: u32,
  read: fn(&Parent::Value) -> &T,
  write: fn(&mut Parent::Value) -> &mut T,
}

impl<Parent: StoreField, T> Subfield<Parent, T> {
  /// Builds a field lens: the field's key—stable under reordering, a hash of its
  /// name—plus its two projections.
  #[inline(always)]
  pub fn new(
    parent: Parent,
    key: u32,
    read: fn(&Parent::Value) -> &T,
    write: fn(&mut Parent::Value) -> &mut T,
  ) -> Self {
    Self {
      parent,
      key,
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
      key: self.key,
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

  fn location(&self) -> StoreLocation {
    self.core().field(self.parent.location(), self.key)
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

  fn location(&self) -> StoreLocation {
    self.core().index(self.parent.location(), self.index)
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
  type Key: Hash + Eq + Clone + 'static;
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

  fn location(&self) -> StoreLocation {
    self.core().key(self.parent.location(), &self.key)
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
