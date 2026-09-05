//! State snapshots: every serializable signal serializes to bytes and restores on
//! creation.
//!
//! A signal's identity across two runs is its position: the path of ordinals from the
//! runtime's root owner to the owner it was created under, its index among that owner's
//! snapshot signals, and its type. [`Signal::tagged`] names the position instead, so a
//! tag anchors the match where a program edit would move it. [`snapshot`] serializes
//! every live signal's current value grouped by owner; [`restore`] stages a prior
//! snapshot so signals created afterwards adopt the staged value at their position.
//!
//! Per owner, restoration aligns the live sequence with the staged one as the live
//! side is created: a tagged signal takes the staged entry of its tag, and an untagged
//! one takes the next unconsumed untagged entry of its type, skipping entries the new
//! program no longer creates. An entry with no match, a match of another type and a
//! value that no longer decodes all fall back to the default: a restore never fails,
//! it degrades to fresh state.

use std::{
  any::type_name,
  cell::RefCell,
  hash::{Hash, Hasher},
  rc::Rc,
};

use rustc_hash::{FxHashMap, FxHashSet, FxHasher};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
  Signal,
  With,
  id::Id,
  runtime::{RUNTIME, Runtime, with_graph},
};

/// A snapshot provider's serializer; `None` omits the entry.
type Serializer = Box<dyn Fn() -> Option<Vec<u8>>>;

/// One live signal's place in the snapshot.
struct Registration {
  owner: Id,
  path: Rc<[u32]>,
  index: u32,
  order: u64,
  ty: u64,
  tag: Option<&'static str>,
  serialize: Serializer,
}

/// One serialized signal, as it crosses runs.
#[derive(Serialize, Deserialize)]
struct Entry {
  ty: u64,
  tag: Option<String>,
  bytes: Vec<u8>,
}

/// The whole snapshot: each owner's entries by path, then the provided extras by tag.
#[derive(Default, Serialize, Deserialize)]
struct Snapshot {
  owners: Vec<(Vec<u32>, Vec<Entry>)>,
  extras: Vec<(String, Vec<u8>)>,
}

/// One owner's staged entries and the alignment cursor over them.
struct Staged {
  entries: Vec<Option<Entry>>,
  cursor: usize,
}

/// This thread's registrations and staged values.
#[derive(Default)]
struct Registry {
  signals: FxHashMap<Id, Registration>,
  tags: FxHashSet<(Id, &'static str)>,
  extras: FxHashMap<&'static str, Serializer>,
  staged: FxHashMap<Rc<[u32]>, Staged>,
  staged_extras: FxHashMap<String, Vec<u8>>,
  next_order: u64,
}

thread_local! {
  static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

/// The hash a type is matched by across runs.
fn type_hash<T: 'static>() -> u64 {
  let mut hasher = FxHasher::default();

  type_name::<T>().hash(&mut hasher);

  hasher.finish()
}

impl<T: Serialize + DeserializeOwned + 'static> Signal<T> {
  /// Creates a signal whose `tag` names its position in state snapshots.
  ///
  /// A tag anchors the match where the positional identity of [`new`](Self::new)
  /// would move: the signal adopts the value staged under its tag in its owner,
  /// whatever its index, and a program edit around it shifts nothing. Tags must be
  /// unique among the live tagged signals of one owner.
  ///
  /// # Panics
  ///
  /// Panics if `tag` is already carried by a live tagged signal of the same owner.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn tagged(tag: &'static str, value: T) -> Self {
    Self::snapshotted(Some(tag), value)
  }

  /// Creates a snapshot signal at the current owner's next position, adopting the
  /// staged value there when one matches.
  #[cfg_attr(debug_assertions, track_caller)]
  pub(crate) fn snapshotted(tag: Option<&'static str>, value: T) -> Self {
    Runtime::assert_ui_thread();

    let (owner, path, index) = RUNTIME.with(|runtime| {
      let owner = runtime.current_owner.get();
      let mut graph = runtime.graph();

      (
        owner,
        graph.owner_path(owner),
        graph.next_signal_index(owner),
      )
    });
    let ty = type_hash::<T>();
    let value = take_staged(&path, ty, tag)
      .and_then(|bytes| postcard::from_bytes(&bytes).ok())
      .unwrap_or(value);
    let signal = Self::opaque(value);

    with_graph(|graph| graph.mark_snapshotted(signal.id));
    register(
      signal.id,
      owner,
      path,
      index,
      ty,
      tag,
      Box::new(move || {
        signal
          .try_with_untracked(|value| postcard::to_allocvec(value).ok())
          .flatten()
      }),
    );

    signal
  }
}

/// Takes the staged entry matching a signal at `path`: the one under `tag`, or the next
/// unconsumed untagged one of type `ty`. Entries skipped on the way are left to tagged
/// lookups only.
fn take_staged(path: &[u32], ty: u64, tag: Option<&str>) -> Option<Vec<u8>> {
  REGISTRY.with(|registry| {
    let mut registry = registry.borrow_mut();
    let staged = registry.staged.get_mut(path)?;
    let position = match tag {
      Some(tag) => staged.entries.iter().position(|entry| {
        entry
          .as_ref()
          .is_some_and(|entry| entry.tag.as_deref() == Some(tag))
      })?,
      None => (staged.cursor..staged.entries.len()).find(|&position| {
        staged.entries[position]
          .as_ref()
          .is_some_and(|entry| entry.tag.is_none() && entry.ty == ty)
      })?,
    };
    let entry = staged.entries[position].take()?;

    staged.cursor = staged.cursor.max(position + 1);

    (entry.ty == ty).then_some(entry.bytes)
  })
}

/// Registers a signal's place in the snapshot.
///
/// # Panics
///
/// Panics if `tag` is already carried by a live tagged signal of `owner`.
fn register(
  id: Id,
  owner: Id,
  path: Rc<[u32]>,
  index: u32,
  ty: u64,
  tag: Option<&'static str>,
  serialize: Serializer,
) {
  REGISTRY.with(|registry| {
    let mut registry = registry.borrow_mut();

    if let Some(tag) = tag {
      assert!(
        registry.tags.insert((owner, tag)),
        "duplicate snapshot tag in one owner: {tag}"
      );
    }

    let order = registry.next_order;

    registry.next_order += 1;
    registry.signals.insert(
      id,
      Registration {
        owner,
        path,
        index,
        order,
        ty,
        tag,
        serialize,
      },
    );
  });
}

/// Forgets a disposed signal's registration.
pub(crate) fn unregister(id: Id) {
  let _ = REGISTRY.try_with(|registry| {
    let mut registry = registry.borrow_mut();

    if let Some(registration) = registry.signals.remove(&id)
      && let Some(tag) = registration.tag
    {
      registry.tags.remove(&(registration.owner, tag));
    }
  });
}

/// Registers a snapshot provider: its bytes join every later [`snapshot`] under `tag`,
/// beside the signals. `None` omits the entry; a restored counterpart is adopted
/// through [`staged`].
///
/// # Panics
///
/// Panics if `tag` is already carried by a live provider.
pub fn provide(tag: &'static str, serialize: impl Fn() -> Option<Vec<u8>> + 'static) {
  REGISTRY.with(|registry| {
    let mut registry = registry.borrow_mut();

    if let Some(previous) = registry.extras.get(tag) {
      assert!(previous().is_none(), "duplicate snapshot provider: {tag}");
    }

    registry.extras.insert(tag, Box::new(serialize));
  });
}

/// Takes the bytes a [`restore`] staged under `tag`, consuming them—the adopting half
/// of a [`provide`]d entry.
#[must_use]
pub fn staged(tag: &str) -> Option<Vec<u8>> {
  REGISTRY.with(|registry| registry.borrow_mut().staged_extras.remove(tag))
}

/// Serializes every live snapshot signal's current value, grouped by owner path and
/// ordered by position, followed by the provided extras ordered by tag.
#[must_use]
pub fn snapshot() -> Vec<u8> {
  let snapshot = REGISTRY.with(|registry| {
    let registry = registry.borrow();
    let mut owners: FxHashMap<&[u32], Vec<(u32, u64, Entry)>> = FxHashMap::default();

    for registration in registry.signals.values() {
      let Some(bytes) = (registration.serialize)()
      else {
        continue;
      };

      owners.entry(&registration.path).or_default().push((
        registration.index,
        registration.order,
        Entry {
          ty: registration.ty,
          tag: registration.tag.map(str::to_owned),
          bytes,
        },
      ));
    }

    let mut owners: Vec<(Vec<u32>, Vec<Entry>)> = owners
      .into_iter()
      .map(|(path, mut entries)| {
        // An owner replaced in place overlaps its predecessor for a moment; the later
        // registration at a position is the live one.
        entries.sort_unstable_by_key(|(index, order, _)| (*index, *order));
        entries.dedup_by_key(|(index, _, _)| *index);

        (
          path.to_vec(),
          entries.into_iter().map(|(_, _, entry)| entry).collect(),
        )
      })
      .collect();

    owners.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));

    let mut extras: Vec<(String, Vec<u8>)> = registry
      .extras
      .iter()
      .filter_map(|(tag, serialize)| serialize().map(|bytes| ((*tag).to_owned(), bytes)))
      .collect();

    extras.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));

    Snapshot { owners, extras }
  });

  postcard::to_allocvec(&snapshot).unwrap_or_default()
}

/// Stages a prior [`snapshot`], so signals created afterwards adopt their staged
/// values.
///
/// Entries whose position never comes up again simply stay unconsumed; bytes that are
/// not a snapshot stage nothing.
pub fn restore(bytes: &[u8]) {
  let snapshot: Snapshot = postcard::from_bytes(bytes).unwrap_or_default();

  REGISTRY.with(|registry| {
    let mut registry = registry.borrow_mut();

    for (path, entries) in snapshot.owners {
      registry.staged.insert(
        Rc::from(path),
        Staged {
          entries: entries.into_iter().map(Some).collect(),
          cursor: 0,
        },
      );
    }

    for (tag, bytes) in snapshot.extras {
      registry.staged_extras.insert(tag, bytes);
    }
  });
}

/// Clears every registration and staged value, for a runtime reset.
pub(crate) fn clear() {
  let _ = REGISTRY.try_with(|registry| *registry.borrow_mut() = Registry::default());
}
