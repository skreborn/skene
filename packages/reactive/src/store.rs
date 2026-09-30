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
//!
//! A location computes a [`Memo`] over its value ([`StoreField::memo`]), and a keyed
//! location hands out a lens per entry, subscribed to its shape alone
//! ([`KeyedField::entries`]), so a keyed list follows the collection and each item
//! follows its entry. A `Vec` is keyed by a key its elements carry
//! ([`IndexedField::keyed_by`]).

use std::{
  any::Any,
  collections::{BTreeMap, HashMap},
  fmt,
  hash::{BuildHasher, Hash, Hasher},
  marker::PhantomData,
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHasher};
use smallvec::SmallVec;

use crate::{
  Memo,
  Owner,
  Runtime,
  Signal,
  Trigger,
  access::With,
  effect::batch,
  encode::Snapshot,
  signal::update_silently,
};

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

/// Which of a location's triggers a read picks.
type Slot = fn(&mut Node) -> &mut Option<Trigger>;

/// The triggers one read tracks, each with the location and the slot it was created in
/// where the read created it.
type Tracked = SmallVec<[(Trigger, Option<(usize, Slot)>); 8]>;

/// One location: its parent and its three triggers, created on first use.
///
/// `below` wakes on a write at this location or beneath it; `here` wakes on a write
/// exactly at this location; `shape` wakes on a change to the collection this location
/// holds: an element added, removed or moved, or the whole replaced. A reader tracks its
/// own `below` and every ancestor's `here`, a reader of the shape its own `shape` and
/// every ancestor's `here`; a write notifies its own `here` and `shape` and the `below`
/// of itself and every ancestor, and a change of shape notifies the `shape` and the
/// `below` of itself and every ancestor, and the elements it moved.
struct Node {
  parent: u32,
  here: Option<Trigger>,
  below: Option<Trigger>,
  shape: Option<Trigger>,
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
        shape: None,
      }],
      edges: FxHashMap::default(),
    }
  }

  /// The trigger `slot` holds on `node`, created when absent, with the node and the
  /// slot where it was created.
  fn trigger(&mut self, node: usize, slot: Slot) -> (Trigger, Option<(usize, Slot)>) {
    let held = slot(&mut self.nodes[node]);
    let created = held.is_none();

    (
      *held.get_or_insert_with(Trigger::new),
      created.then_some((node, slot)),
    )
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
      shape: None,
    });

    next
  }

  /// The child of `parent` at `index`, where one was ever addressed.
  fn existing_index(&self, parent: u32, index: usize) -> Option<u32> {
    self
      .edges
      .get(&(parent, edge_hash(1, index)))?
      .iter()
      .find(|(edge, _)| matches!(edge, Edge::Index(existing) if *existing == index))
      .map(|(_, node)| *node)
  }

  /// The `below` triggers of `node` and of every location containing it.
  fn below_upward(&self, mut node: usize, triggers: &mut SmallVec<[Trigger; 8]>) {
    loop {
      triggers.extend(self.nodes[node].below);

      let parent = self.nodes[node].parent;

      if parent == NO_PARENT {
        break;
      }

      node = parent as usize;
    }
  }

  /// The `here` and `below` triggers of the element of `parent` at each of `indexes`
  /// ever addressed.
  fn elements(
    &self,
    parent: u32,
    indexes: impl IntoIterator<Item = usize>,
    triggers: &mut SmallVec<[Trigger; 8]>,
  ) {
    for index in indexes {
      if let Some(element) = self.existing_index(parent, index) {
        let element = &self.nodes[element as usize];

        triggers.extend(element.here);
        triggers.extend(element.below);
      }
    }
  }
}

/// The bucket hash of one edge kind and its key.
fn edge_hash(kind: u8, key: impl Hash) -> u64 {
  let mut hasher = FxHasher::default();

  kind.hash(&mut hasher);
  key.hash(&mut hasher);

  hasher.finish()
}

/// The key-to-index maps keyed vectors read through, each under its vector's location
/// and its key function.
#[derive(Default)]
struct Indexes(FxHashMap<(u32, usize), Box<dyn Any>>);

/// A store's shared plumbing: the location trie, the keyed vectors' indexes and the
/// owning owner.
///
/// Every lens of one store carries the same core; custom lenses only forward it and
/// intern their locations through it.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct StoreCore {
  trie: Signal<Trie>,
  indexes: Signal<Indexes>,
  owner: Owner,
}

impl StoreCore {
  fn new() -> Self {
    Self {
      trie: Signal::transient(Trie::new()),
      indexes: Signal::transient(Indexes::default()),
      owner: Owner::current(),
    }
  }

  /// The index of the element `key_of` keys `key` among `elements`, the vector at
  /// `location`, read through a key-to-index map rebuilt only when it no longer
  /// answers.
  fn keyed_index<T, K: Hash + Eq + Clone + 'static>(
    &self,
    location: StoreLocation,
    elements: &[T],
    key_of: fn(&T) -> K,
    key: &K,
  ) -> Option<usize> {
    let answers = |map: &FxHashMap<K, usize>| {
      map.get(key).copied().filter(|index| {
        elements
          .get(*index)
          .is_some_and(|element| key_of(element) == *key)
      })
    };
    let slot = (location.0, key_of as usize);

    update_silently(self.indexes.id(), |indexes: &mut Indexes| {
      if let Some(index) = indexes
        .0
        .get(&slot)
        .and_then(|map| map.downcast_ref::<FxHashMap<K, usize>>())
        .and_then(answers)
      {
        return Some(index);
      }

      let map: FxHashMap<K, usize> = elements
        .iter()
        .enumerate()
        .map(|(index, element)| (key_of(element), index))
        .collect();
      let index = answers(&map);

      indexes.0.insert(slot, Box::new(map));

      index
    })
    .flatten()
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
    self.track_with(location, |node| &mut node.below);
  }

  /// Subscribes the running effect to writes at `location` and at the locations
  /// containing it, not to writes beneath it.
  fn track_shallow(&self, location: StoreLocation) {
    self.track_with(location, |node| &mut node.here);
  }

  /// Subscribes the running effect to the shape of the collection at `location` and to
  /// writes at the locations containing it.
  fn track_shape(&self, location: StoreLocation) {
    self.track_with(location, |node| &mut node.shape);
  }

  /// Subscribes the running effect, if any, to the trigger `own` picks on `location`
  /// and to `here` on every location containing it; a trigger nothing tracks any more
  /// is dropped.
  fn track_with(&self, location: StoreLocation, own: Slot) {
    if !Runtime::is_tracking() {
      return;
    }

    let owner = self.owner;
    let trie_id = self.trie.id();
    let here: Slot = |node| &mut node.here;
    let triggers: Tracked = update_silently(trie_id, |trie: &mut Trie| {
      owner.enter(|| {
        let mut triggers = SmallVec::new();
        let mut node = location.0 as usize;

        triggers.push(trie.trigger(node, own));

        while trie.nodes[node].parent != NO_PARENT {
          node = trie.nodes[node].parent as usize;

          triggers.push(trie.trigger(node, here));
        }

        triggers
      })
    })
    .unwrap_or_default();

    for (trigger, created) in triggers {
      if let Some((node, slot)) = created {
        trigger.dispose_when_unobserved(move || {
          let _ = update_silently(trie_id, |trie: &mut Trie| {
            *slot(&mut trie.nodes[node]) = None;
          });
        });
      }

      trigger.track();
    }
  }

  /// Notifies every reader a write at `location` can affect: the location itself and
  /// its shape, the locations containing it, and everything beneath it.
  fn notify(&self, location: StoreLocation) {
    self.wake(|trie, triggers| {
      let node = &trie.nodes[location.0 as usize];

      triggers.extend(node.here);
      triggers.extend(node.shape);
      trie.below_upward(location.0 as usize, triggers);
    });
  }

  /// Notifies every reader a change of shape at `location` can affect: its shape's, its
  /// own and its ancestors' whole-value readers, and the readers of each element at
  /// `moved`, the positions whose element the change replaced; an element's readers
  /// elsewhere sleep.
  fn notify_shape(&self, location: StoreLocation, moved: std::ops::Range<usize>) {
    self.wake(|trie, triggers| {
      triggers.extend(trie.nodes[location.0 as usize].shape);
      trie.below_upward(location.0 as usize, triggers);
      trie.elements(location.0, moved, triggers);
    });
  }

  /// Notifies the readers of each element of the collection at `location` at
  /// `changed`, written in place, and the whole-value readers of the collection and of
  /// every location containing it.
  fn notify_elements(&self, location: StoreLocation, changed: &[usize]) {
    self.wake(|trie, triggers| {
      trie.below_upward(location.0 as usize, triggers);
      trie.elements(location.0, changed.iter().copied(), triggers);
    });
  }

  /// Notifies, as one batch, the triggers `gather` collects from the trie.
  fn wake(&self, gather: impl FnOnce(&Trie, &mut SmallVec<[Trigger; 8]>)) {
    let triggers: SmallVec<[Trigger; 8]> = self
      .trie
      .try_with_untracked(|trie| {
        let mut triggers = SmallVec::new();

        gather(trie, &mut triggers);

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
/// [`transient`](Self::transient), which take part in state snapshots exactly as the
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

impl<T: 'static> fmt::Debug for Store<T> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Store")
      .field("value", &self.value)
      .finish_non_exhaustive()
  }
}

impl<T: Snapshot> Store<T> {
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
  /// snapshots; see [`Signal::transient`].
  pub fn transient(value: T) -> Self {
    Self::over(Signal::transient(value))
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
/// can affect, and [`memo`](Self::memo) memoizes a read of it. The `try_`
/// variants return `None` where the plain calls panic: when the location no longer
/// exists (a removed key, an out-of-range index).
pub trait StoreField: Clone + 'static {
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

  /// Subscribes the running effect to writes at this location and at the locations
  /// containing it, not to writes beneath it.
  fn track_shallow(&self) {
    self.core().track_shallow(self.location());
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

  /// A memo of `compute` over the value, subscribed to this location; its dependents
  /// wake only when the result changes.
  ///
  /// # Panics
  ///
  /// The memo panics when it computes while the location no longer exists; see
  /// [`try_memo`](Self::try_memo).
  fn memo<R: PartialEq + 'static>(&self, compute: impl Fn(&Self::Value) -> R + 'static) -> Memo<R> {
    let location = self.clone();

    Memo::new(move |_| location.with(&compute))
  }

  /// A memo of `compute` over the value, subscribed to this location, `None` while the
  /// location no longer exists; its dependents wake only when the result changes.
  fn try_memo<R: PartialEq + 'static>(
    &self,
    compute: impl Fn(&Self::Value) -> R + 'static,
  ) -> Memo<Option<R>> {
    let location = self.clone();

    Memo::new(move |_| location.try_with(&compute))
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
#[derive(Debug)]
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
#[derive(Debug)]
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

/// Index addressing on a store location holding a `Vec`, and the writes that change
/// its shape while the readers of the elements they leave in place sleep.
pub trait IndexedField<T>: StoreField<Value = Vec<T>> + Sized {
  /// The element at `index`, by position.
  fn at(self, index: usize) -> AtIndex<Self, T> {
    AtIndex {
      parent: self,
      index,
      element: PhantomData,
    }
  }

  /// The number of elements, subscribed to the vector's shape alone: a write beneath an
  /// element leaves it asleep.
  fn len(&self) -> usize {
    self.core().track_shape(self.location());

    self.read_untracked(Vec::len).unwrap_or(0)
  }

  /// Whether the vector is empty, subscribed to its shape alone; see [`len`](Self::len).
  fn is_empty(&self) -> bool {
    self.len() == 0
  }

  /// Appends `element`, waking the vector's shape and whole-value readers and no
  /// element's.
  fn push(&self, element: T) {
    let Some(index) = self.write_untracked(|elements| {
      elements.push(element);

      elements.len() - 1
    })
    else {
      return;
    };

    self.core().notify_shape(self.location(), index..index + 1);
  }

  /// Inserts `element` at `index`, shifting the elements from there on: wakes the
  /// vector's shape and whole-value readers and the readers of each position from
  /// `index` on.
  ///
  /// # Panics
  ///
  /// Panics when `index` is past the end.
  fn insert(&self, index: usize, element: T) {
    let Some(end) = self.write_untracked(|elements| {
      elements.insert(index, element);

      elements.len()
    })
    else {
      return;
    };

    self.core().notify_shape(self.location(), index..end);
  }

  /// Removes and returns the element at `index`, shifting the elements after it: wakes
  /// the vector's shape and whole-value readers and the readers of each position from
  /// `index` on; `None` where the location no longer exists.
  ///
  /// # Panics
  ///
  /// Panics when `index` is out of range.
  fn remove(&self, index: usize) -> Option<T> {
    let (element, end) = self.write_untracked(|elements| {
      let end = elements.len();

      (elements.remove(index), end)
    })?;

    self.core().notify_shape(self.location(), index..end);

    Some(element)
  }

  /// The elements by the key `key_of` reads from each; see [`KeyedBy`].
  fn keyed_by<K: Hash + Eq + Clone + 'static>(self, key_of: fn(&T) -> K) -> KeyedBy<Self, K, T> {
    KeyedBy {
      parent: self,
      key_of,
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

  /// The keys, in the collection's iteration order.
  fn keys(&self) -> impl Iterator<Item = &Self::Key>;
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

  fn keys(&self) -> impl Iterator<Item = &K> {
    Self::keys(self)
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

  fn keys(&self) -> impl Iterator<Item = &K> {
    Self::keys(self)
  }
}

/// A `Vec` location addressed by a key each element carries; built by
/// [`IndexedField::keyed_by`].
///
/// A read by key finds the first element carrying it, so keys are distinct. An element
/// addressed by key and the same element addressed by position
/// ([`IndexedField::at`]) are one location: a write through either wakes the readers
/// through both, and a key's location follows its element across a reorder.
#[derive(Debug)]
#[must_use]
pub struct KeyedBy<Parent, K, T> {
  parent: Parent,
  key_of: fn(&T) -> K,
}

impl<Parent: Clone, K, T> Clone for KeyedBy<Parent, K, T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      parent: self.parent.clone(),
      key_of: self.key_of,
    }
  }
}

impl<Parent: Copy, K, T> Copy for KeyedBy<Parent, K, T> {}

impl<Parent, K, T> StoreField for KeyedBy<Parent, K, T>
where
  Parent: StoreField<Value = Vec<T>>,
  K: 'static,
  T: 'static,
{
  type Value = Vec<T>;

  #[inline(always)]
  fn core(&self) -> StoreCore {
    self.parent.core()
  }

  #[inline(always)]
  fn location(&self) -> StoreLocation {
    self.parent.location()
  }

  fn read_untracked<R>(&self, read: impl FnOnce(&Vec<T>) -> R) -> Option<R> {
    self.parent.read_untracked(read)
  }

  fn write_untracked<R>(&self, write: impl FnOnce(&mut Vec<T>) -> R) -> Option<R> {
    self.parent.write_untracked(write)
  }
}

impl<Parent, K, T> KeyedBy<Parent, K, T>
where
  Parent: StoreField<Value = Vec<T>>,
  K: Hash + Eq + Clone + 'static,
  T: PartialEq + 'static,
{
  /// Replaces the elements with `next`, waking what changed alone.
  ///
  /// While the keys keep their order, each element whose value changed wakes its
  /// readers and the rest sleep; where a key is added, removed or moved, the shape's
  /// readers wake too, with those of every position from the first the change moved.
  pub fn reconcile(&self, next: Vec<T>) {
    let key_of = self.key_of;
    let Some((changed, moved)) = self.parent.write_untracked(|elements| {
      let shared = elements.len().min(next.len());
      let first_moved = elements
        .iter()
        .zip(&next)
        .position(|(held, coming)| key_of(held) != key_of(coming))
        .or((elements.len() != next.len()).then_some(shared));
      let end = elements.len().max(next.len());
      let changed: Vec<usize> = elements
        .iter()
        .zip(&next)
        .take(first_moved.unwrap_or(shared))
        .enumerate()
        .filter(|(_, (held, coming))| held != coming)
        .map(|(index, _)| index)
        .collect();

      *elements = next;

      (changed, first_moved.map(|first| first..end))
    })
    else {
      return;
    };
    let location = self.location();

    batch(|| {
      if !changed.is_empty() {
        self.core().notify_elements(location, &changed);
      }

      if let Some(moved) = moved {
        self.core().notify_shape(location, moved);
      }
    });
  }
}

impl<Parent, K, T> KeyedField for KeyedBy<Parent, K, T>
where
  Parent: StoreField<Value = Vec<T>>,
  K: Hash + Eq + Clone + 'static,
  T: 'static,
{
  type Entry = T;
  type Key = K;

  fn entry<'a>(&self, value: &'a Vec<T>, key: &K) -> Option<&'a T> {
    let index = self
      .core()
      .keyed_index(self.location(), value, self.key_of, key)?;

    value.get(index)
  }

  fn entry_mut<'a>(&self, value: &'a mut Vec<T>, key: &K) -> Option<&'a mut T> {
    let index = self
      .core()
      .keyed_index(self.location(), value, self.key_of, key)?;

    value.get_mut(index)
  }

  fn keys(&self, value: &Vec<T>) -> impl Iterator<Item = K> {
    value.iter().map(self.key_of)
  }

  fn entry_location(&self, key: &K) -> StoreLocation {
    let parent = self.location();
    let position = self
      .read_untracked(|value| self.core().keyed_index(parent, value, self.key_of, key))
      .flatten();

    match position {
      Some(index) => self.core().index(parent, index),
      None => self.core().key(parent, key),
    }
  }
}

/// A lens onto one entry of the parent location, by key; built by
/// [`KeyedField::key`] and [`KeyedField::entries`].
///
/// A read of an entry no element carries yet also follows the collection's shape, so it
/// wakes when a write adds the entry.
#[derive(Debug)]
#[must_use]
pub struct AtKey<Parent: KeyedField> {
  parent: Parent,
  key: Parent::Key,
}

impl<Parent: KeyedField> AtKey<Parent> {
  /// The key this lens addresses.
  #[inline(always)]
  pub fn entry_key(&self) -> Parent::Key {
    self.key.clone()
  }
}

#[expect(
  clippy::expl_impl_clone_on_copy,
  reason = "a lens clones where its parent or key cannot copy"
)]
impl<Parent: KeyedField> Clone for AtKey<Parent> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      parent: self.parent.clone(),
      key: self.key.clone(),
    }
  }
}

impl<Parent: KeyedField + Copy> Copy for AtKey<Parent> where Parent::Key: Copy {}

impl<Parent: KeyedField> StoreField for AtKey<Parent> {
  type Value = Parent::Entry;

  #[inline(always)]
  fn core(&self) -> StoreCore {
    self.parent.core()
  }

  fn location(&self) -> StoreLocation {
    self.parent.entry_location(&self.key)
  }

  fn track(&self) {
    self.core().track(self.location());

    if self.read_untracked(|_| ()).is_none() {
      self.core().track_shape(self.parent.location());
    }
  }

  fn read_untracked<R>(&self, read: impl FnOnce(&Self::Value) -> R) -> Option<R> {
    self
      .parent
      .read_untracked(|entries| self.parent.entry(entries, &self.key).map(read))
      .flatten()
  }

  fn write_untracked<R>(&self, write: impl FnOnce(&mut Self::Value) -> R) -> Option<R> {
    self
      .parent
      .write_untracked(|entries| self.parent.entry_mut(entries, &self.key).map(write))
      .flatten()
  }
}

/// Key addressing on a store location: the entries of a [`KeyedCollection`], or a
/// `Vec`'s elements by a key each carries ([`KeyedBy`]).
pub trait KeyedField: StoreField + Sized {
  /// The key addressing an entry.
  type Key: Hash + Eq + Clone + 'static;
  /// The entry type.
  type Entry: 'static;

  /// The entry under `key` in `value`.
  fn entry<'a>(&self, value: &'a Self::Value, key: &Self::Key) -> Option<&'a Self::Entry>;

  /// The entry under `key` in `value`, mutably.
  fn entry_mut<'a>(
    &self,
    value: &'a mut Self::Value,
    key: &Self::Key,
  ) -> Option<&'a mut Self::Entry>;

  /// The keys of `value`, in its order.
  fn keys(&self, value: &Self::Value) -> impl Iterator<Item = Self::Key>;

  /// The location of the entry under `key`: the key's own beneath this location,
  /// unless the collection addresses the entry another way as well, in which case the
  /// one location both ways share.
  fn entry_location(&self, key: &Self::Key) -> StoreLocation {
    self.core().key(self.location(), key)
  }

  /// The entry under `key`.
  fn key(self, key: Self::Key) -> AtKey<Self> {
    AtKey { parent: self, key }
  }

  /// A lens per entry, in the collection's order.
  ///
  /// Subscribes the running effect to the collection's shape alone: it wakes when an
  /// entry is added or removed, the collection reordered or replaced, never when an
  /// entry is written beneath; each lens subscribes its own readers to its entry.
  fn entries(&self) -> Vec<AtKey<Self>> {
    self.core().track_shape(self.location());

    self
      .read_untracked(|value| {
        self
          .keys(value)
          .map(|key| AtKey {
            parent: self.clone(),
            key,
          })
          .collect()
      })
      .unwrap_or_default()
  }
}

impl<S: StoreField> KeyedField for S
where
  S::Value: KeyedCollection,
{
  type Entry = <S::Value as KeyedCollection>::Entry;
  type Key = <S::Value as KeyedCollection>::Key;

  fn entry<'a>(&self, value: &'a S::Value, key: &Self::Key) -> Option<&'a Self::Entry> {
    value.entry(key)
  }

  fn entry_mut<'a>(&self, value: &'a mut S::Value, key: &Self::Key) -> Option<&'a mut Self::Entry> {
    value.entry_mut(key)
  }

  fn keys(&self, value: &S::Value) -> impl Iterator<Item = Self::Key> {
    value.keys().cloned()
  }
}
