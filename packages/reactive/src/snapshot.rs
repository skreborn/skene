//! Opt-in state snapshots: tagged signals serialize to bytes and restore on creation.
//!
//! A signal created with [`Signal::tagged`] names itself with a unique tag—its
//! identity across two runs of the same application, resilient to code moving around.
//! [`snapshot`] serializes every live tagged signal's current value; [`restore`] stages
//! a prior snapshot so tagged signals created afterwards adopt their staged value
//! instead of the given default. A staged entry that no longer decodes (the type
//! changed) is dropped: a restore never fails, it degrades to fresh state.
//!
//! Untagged signals never appear in a snapshot, and nothing here costs anything unless
//! the `snapshot` cargo feature is enabled.

use std::cell::RefCell;

use rustc_hash::FxHashMap;
use serde::{Serialize, de::DeserializeOwned};

use crate::{Signal, With};

/// Each live tagged signal's serializer; `None` from a serializer means the signal was
/// disposed.
type Serializers = RefCell<FxHashMap<&'static str, Box<dyn Fn() -> Option<Vec<u8>>>>>;

thread_local! {
  static SERIALIZERS: Serializers = RefCell::new(FxHashMap::default());
  /// Restore values staged by tag, each consumed by the next matching [`Signal::tagged`].
  static STAGED: RefCell<FxHashMap<String, Vec<u8>>> = RefCell::new(FxHashMap::default());
}

impl<T: Serialize + DeserializeOwned + 'static> Signal<T> {
  /// Creates a signal whose `tag` is its identity in state snapshots.
  ///
  /// The signal starts from the value staged under its tag by [`restore`] when one is
  /// present and decodes as `T`, and from `value` otherwise; a staged value applies
  /// exactly once. The signal's current value is included in every later [`snapshot`].
  ///
  /// # Panics
  ///
  /// Panics if `tag` is already carried by a live tagged signal.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn tagged(tag: &'static str, value: T) -> Self {
    let value = take_staged(tag)
      .and_then(|bytes| postcard::from_bytes(&bytes).ok())
      .unwrap_or(value);
    let signal = Self::new(value);

    register(
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

/// Takes the value staged under `tag`, consuming it; the next creation under the same
/// tag starts fresh.
pub(crate) fn take_staged(tag: &str) -> Option<Vec<u8>> {
  STAGED.with(|staged| staged.borrow_mut().remove(tag))
}

/// Registers a tagged value's serializer, replacing a disposed predecessor.
///
/// # Panics
///
/// Panics if `tag` is already carried by a live tagged signal or store.
pub(crate) fn register(tag: &'static str, serialize: Box<dyn Fn() -> Option<Vec<u8>>>) {
  SERIALIZERS.with(|serializers| {
    let mut serializers = serializers.borrow_mut();

    if let Some(previous) = serializers.get(tag) {
      assert!(previous().is_none(), "duplicate snapshot tag: {tag}");
    }

    serializers.insert(tag, serialize);
  });
}

/// Serializes every live tagged signal's current value, ordered by tag.
#[must_use]
pub fn snapshot() -> Vec<u8> {
  let mut entries: Vec<(String, Vec<u8>)> = SERIALIZERS.with(|serializers| {
    serializers
      .borrow()
      .iter()
      .filter_map(|(tag, serialize)| serialize().map(|bytes| ((*tag).to_owned(), bytes)))
      .collect()
  });

  entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));

  postcard::to_allocvec(&entries).unwrap_or_default()
}

/// Stages a prior [`snapshot`], so tagged signals created afterwards adopt their staged
/// values.
///
/// Entries whose tag never comes up again simply stay unconsumed; bytes that are not a
/// snapshot stage nothing.
pub fn restore(bytes: &[u8]) {
  let entries: Vec<(String, Vec<u8>)> = postcard::from_bytes(bytes).unwrap_or_default();

  STAGED.with(|staged| {
    let mut staged = staged.borrow_mut();

    for (tag, value) in entries {
      staged.insert(tag, value);
    }
  });
}

/// Clears every registration and staged value, for a runtime reset.
#[cfg(feature = "test")]
pub(crate) fn clear() {
  SERIALIZERS.with(|serializers| serializers.borrow_mut().clear());
  STAGED.with(|staged| staged.borrow_mut().clear());
}
