//! A keyed list: each item is built once per key, under an owner of its own, and kept while its
//! key stays in the list, wherever it moves.

use std::{collections::HashMap, hash::Hash};

use crate::Owner;

/// The items of a keyed list, each a key, the owner its item was built under, and the handle
/// the build returned (a realization's node).
pub struct Keyed<K, H> {
  owner: Option<Owner>,
  entries: Vec<(K, Owner, H)>,
}

impl<K: Eq + Hash + Clone, H: Clone> Keyed<K, H> {
  /// An empty list whose items' owners belong to the current owner.
  #[must_use]
  pub fn new() -> Self {
    Self {
      owner: Owner::current(),
      entries: Vec::new(),
    }
  }

  /// The handles in order.
  pub fn handles(&self) -> impl Iterator<Item = &H> {
    self.entries.iter().map(|(_, _, handle)| handle)
  }

  /// Brings the list to `items` and returns the handles in their new order.
  ///
  /// An item whose key the list holds keeps its handle and its owner. Each new key's item is
  /// built by `build` under an owner of its own; each key no longer in the list has its owner
  /// disposed and its handle given to `remove`. Of items sharing a key, the first is kept and
  /// the rest are built anew.
  pub fn update<T>(
    &mut self,
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> K,
    mut build: impl FnMut(T) -> H,
    mut remove: impl FnMut(H),
  ) -> Vec<H> {
    let mut old: HashMap<K, (Owner, H)> = self
      .entries
      .drain(..)
      .map(|(key, owner, handle)| (key, (owner, handle)))
      .collect();
    let parent = self.owner;

    for item in items {
      let key = key(&item);
      let (owner, handle) = match old.remove(&key) {
        Some(kept) => kept,
        None => {
          let create = || {
            let owner = Owner::new();
            let handle = owner.run(|| build(item));

            (owner, handle)
          };

          match parent {
            Some(parent) => parent.run(create),
            None => create(),
          }
        }
      };

      self.entries.push((key, owner, handle));
    }

    for (_, (owner, handle)) in old {
      owner.dispose();
      remove(handle);
    }

    self.handles().cloned().collect()
  }
}

impl<K: Eq + Hash + Clone, H: Clone> Default for Keyed<K, H> {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn kept_keys_keep_their_handles() {
    let mut list = Keyed::new();
    let mut built = 0;
    let mut removed = Vec::new();
    let mut build = |item: (u32, &str)| {
      built += 1;
      format!("{}#{built}", item.1)
    };

    let first = list.update([(1, "a"), (2, "b")], |item| item.0, &mut build, |_| {});
    let second = list.update(
      [(2, "b"), (3, "c")],
      |item| item.0,
      &mut build,
      |handle| removed.push(handle),
    );

    assert_eq!(first, ["a#1", "b#2"]);
    assert_eq!(second, ["b#2", "c#3"]);
    assert_eq!(removed, ["a#1"]);
  }
}
