//! The pure reconcile decisions, with no platform involved: which keyed items a run
//! keeps, builds and drops, and which kept ones a reorder can leave in place.

use std::collections::VecDeque;

use rustc_hash::FxHashMap;
use skene_reactive::Key;

/// Decides which kept entries a reorder can leave in place.
///
/// Holds its working buffers so a repeated reconcile allocates nothing.
/// Platform-agnostic: its caller feeds it positions and applies the result with its own
/// tree operations.
#[derive(Debug, Default)]
#[must_use]
pub struct Reorder {
  tails: Vec<usize>,
  predecessors: Vec<Option<usize>>,
  settled: Vec<usize>,
}

impl Reorder {
  /// The positions in the new order that need no re-insertion.
  ///
  /// `previous_positions[i]` is the previous position of the kept entry now at
  /// `new_positions[i]`; both are in the new order. Entries on the longest increasing
  /// run of previous positions stay put—every other position is re-inserted before its
  /// successor, the fewest re-insertions that restore the order.
  ///
  /// The result is in descending order.
  pub fn settled_positions(
    &mut self,
    previous_positions: &[usize],
    new_positions: &[usize],
  ) -> &[usize] {
    self.tails.clear();
    self.predecessors.clear();

    for (index, &position) in previous_positions.iter().enumerate() {
      let length = self
        .tails
        .partition_point(|&tail| previous_positions[tail] < position);

      self
        .predecessors
        .push(length.checked_sub(1).map(|shorter| self.tails[shorter]));

      if length == self.tails.len() {
        self.tails.push(index);
      }
      else {
        self.tails[length] = index;
      }
    }

    self.settled.clear();

    let mut current = self.tails.last().copied();

    while let Some(index) = current {
      self.settled.push(new_positions[index]);
      current = self.predecessors[index];
    }

    &self.settled
  }
}

/// What a keyed reconcile keeps between its runs: the items by key in order, each with
/// what it mounted, and the scratch each run reuses.
///
/// A run is a plan its caller applies: [`update`](Self::update) builds the new items and
/// hands back the gone ones, and [`place_displaced`](Self::place_displaced) names the
/// items to re-insert, so a segment's content follows the items in order.
#[derive(Debug)]
#[must_use]
pub(in crate::spine) struct KeyedItems<M> {
  mounted: Vec<(Key, M)>,
  old: FxHashMap<Key, VecDeque<(usize, M)>>,
  occurrences: FxHashMap<Key, u32>,
  old_positions: Vec<usize>,
  new_positions: Vec<usize>,
  reorder: Reorder,
}

impl<M> Default for KeyedItems<M> {
  fn default() -> Self {
    Self {
      mounted: Vec::new(),
      old: FxHashMap::default(),
      occurrences: FxHashMap::default(),
      old_positions: Vec::new(),
      new_positions: Vec::new(),
      reorder: Reorder::default(),
    }
  }
}

impl<M> KeyedItems<M> {
  /// Brings the items to those `keys` names in order, returning what the gone ones
  /// mounted, the last first, for the caller to take down.
  ///
  /// A kept item keeps what it mounted, the nth item of a key matching the nth before it,
  /// and a new one mounts what `build` makes, in order, from its index and the key its
  /// scope is owned under: its own key, or for a repeat, its key with how many came
  /// before it.
  pub(in crate::spine) fn update(
    &mut self,
    keys: &[Key],
    mut build: impl FnMut(usize, Key) -> M,
  ) -> Vec<M> {
    let Self {
      mounted,
      old,
      occurrences,
      old_positions,
      new_positions,
      reorder: _,
    } = self;

    for (index, (key, entry)) in mounted.drain(..).enumerate() {
      old.entry(key).or_default().push_back((index, entry));
    }

    occurrences.clear();
    old_positions.clear();
    new_positions.clear();

    for (index, &key) in keys.iter().enumerate() {
      let occurrence = occurrences.entry(key).or_insert(0);
      let repeat = *occurrence;

      *occurrence += 1;

      #[cfg(debug_assertions)]
      if repeat == 1 {
        warn_repeated_key();
      }

      let kept = old.get_mut(&key).and_then(VecDeque::pop_front);
      let entry = if let Some((position, entry)) = kept {
        old_positions.push(position);
        new_positions.push(mounted.len());

        entry
      }
      else {
        build(
          index,
          if repeat == 0 {
            key
          }
          else {
            Key::of(&(key, repeat))
          },
        )
      };

      mounted.push((key, entry));
    }

    // The last goes first, so each removal shifts as little as it can.
    let mut dropped: Vec<(usize, M)> = old.drain().flat_map(|(_, entries)| entries).collect();

    dropped.sort_unstable_by_key(|(index, _)| std::cmp::Reverse(*index));
    dropped.into_iter().map(|(_, entry)| entry).collect()
  }

  /// Names each item the last [`update`](Self::update) displaced, from the last on, to
  /// `place` with the item it goes before, `None` for the end: placed in turn, the items
  /// stand in order, the kept ones on the longest increasing run of their previous
  /// positions staying where they are.
  pub(in crate::spine) fn place_displaced(&mut self, mut place: impl FnMut(&M, Option<&M>)) {
    let settled = self
      .reorder
      .settled_positions(&self.old_positions, &self.new_positions);
    let mut stable = settled.iter().copied().peekable();

    for index in (0..self.mounted.len()).rev() {
      if stable.next_if_eq(&index).is_none() {
        place(
          &self.mounted[index].1,
          self.mounted.get(index + 1).map(|(_, entry)| entry),
        );
      }
    }
  }
}

/// Warns, once, that a reconciled collection repeated a key.
#[cfg(debug_assertions)]
fn warn_repeated_key() {
  static WARNED: std::sync::Once = std::sync::Once::new();

  WARNED.call_once(|| {
    eprintln!(
      "skene-foundation: a reconciled collection repeated a key; keys should be unique, and \
       items sharing one match in order"
    );
  });
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use skene_reactive::Key;

  use super::{KeyedItems, Reorder};

  /// A segment's content as a reconcile drives it, with what each owner was built under
  /// and from which index of its list: items are owners, plain ids.
  #[derive(Default)]
  struct Model {
    items: KeyedItems<u32>,
    content: Vec<u32>,
    scopes: HashMap<u32, Key>,
    built_at: HashMap<u32, usize>,
    next: u32,
  }

  impl Model {
    /// Reconciles the content to `items`, returning the owners built and disposed.
    fn run(&mut self, items: &[u8]) -> (Vec<u32>, Vec<u32>) {
      let keys: Vec<Key> = items.iter().map(Key::of).collect();
      let mut built = Vec::new();
      let dropped = self.items.update(&keys, |index, scope| {
        let owner = self.next;

        assert_eq!(scope_of(&keys, index), scope);
        self.next += 1;
        self.scopes.insert(owner, scope);
        self.built_at.insert(owner, index);
        built.push(owner);

        owner
      });

      self.content.retain(|owner| !dropped.contains(owner));

      let content = &mut self.content;

      self.items.place_displaced(|owner, before| {
        content.retain(|placed| placed != owner);

        let at = before.map_or(content.len(), |before| {
          content
            .iter()
            .position(|placed| placed == before)
            .expect("an item is placed before one already placed")
        });

        content.insert(at, *owner);
      });

      assert_eq!(
        self.content,
        self
          .items
          .mounted
          .iter()
          .map(|(_, owner)| *owner)
          .collect::<Vec<_>>(),
        "the content follows the items"
      );

      (built, dropped)
    }
  }

  /// The key the scope of the item at `index` is owned under: its own, or for a repeat,
  /// its own with how many came before it.
  fn scope_of(keys: &[Key], index: usize) -> Key {
    let key = keys[index];
    let repeat = keys[..index]
      .iter()
      .filter(|before| **before == key)
      .count();

    if repeat == 0 {
      key
    }
    else {
      Key::of(&(key, u32::try_from(repeat).unwrap()))
    }
  }

  /// A reconciled item that leaves has its owner handed back to dispose, and kept items
  /// keep their owners.
  #[test]
  fn reconcile_disposes_the_owner_of_a_removed_item() {
    let mut model = Model::default();

    assert_eq!(model.run(&[1, 2, 3]), (vec![0, 1, 2], vec![]));
    assert_eq!(model.content, [0, 1, 2]);
    assert_eq!(model.run(&[3, 1]), (vec![], vec![1]));
    assert_eq!(model.content, [2, 0]);
  }

  /// Items sharing a key match the ones before them in order, each its own owner under a
  /// scope of its own, and every one that leaves is handed back, so no owner outlives
  /// its item.
  #[test]
  fn reconcile_matches_repeated_keys_in_order() {
    let mut model = Model::default();

    assert_eq!(model.run(&[1, 1, 2]), (vec![0, 1, 2], vec![]));
    assert_ne!(model.scopes[&0], model.scopes[&1]);
    assert_eq!(
      model.run(&[2, 1]),
      (vec![], vec![1]),
      "the second of the repeated items is gone"
    );
    assert_eq!(model.content, [2, 0]);
    assert_eq!(model.run(&[1, 1, 1]), (vec![3, 4], vec![2]));
    assert_eq!(model.content, [0, 3, 4], "the first keeps its owner");
  }

  /// The settled positions for a run of kept entries, as owned values.
  fn settled(previous_positions: &[usize], new_positions: &[usize]) -> Vec<usize> {
    Reorder::default()
      .settled_positions(previous_positions, new_positions)
      .to_vec()
  }

  /// Positions in the new order, one per kept entry.
  fn in_order(count: usize) -> Vec<usize> {
    (0..count).collect()
  }

  /// Nothing to reorder.
  #[test]
  fn nothing_kept_settles_nothing() {
    assert_eq!(settled(&[], &[]), Vec::<usize>::new());
  }

  /// A lone kept entry is always already in place.
  #[test]
  fn one_kept_entry_settles() {
    assert_eq!(settled(&[0], &[0]), vec![0]);
  }

  /// An unchanged order moves nothing: every entry settles.
  #[test]
  fn unchanged_order_settles_every_entry() {
    assert_eq!(settled(&[0, 1, 2], &in_order(3)), vec![2, 1, 0]);
  }

  /// Moving the last entry to the front settles the other two, so only one insertion is
  /// needed. This is the `[1, 2, 3] -> [3, 1, 2]` case.
  #[test]
  fn rotating_one_entry_settles_the_rest() {
    assert_eq!(settled(&[2, 0, 1], &in_order(3)), vec![2, 1]);
  }

  /// A full reversal has no increasing run longer than one, so all but one entry moves.
  #[test]
  fn full_reversal_settles_a_single_entry() {
    assert_eq!(settled(&[2, 1, 0], &in_order(3)).len(), 1);
  }

  /// Entries that were not adjacent before still settle when their relative order holds.
  #[test]
  fn gaps_in_previous_positions_still_settle() {
    // Kept entries came from previous positions 0, 3 and 7, still in ascending order.
    assert_eq!(settled(&[0, 3, 7], &in_order(3)), vec![2, 1, 0]);
  }

  /// The settled positions index the new order, not the previous one.
  #[test]
  fn settled_positions_are_new_positions() {
    // Three kept entries sitting at new positions 1, 3 and 4 (others were newly built).
    assert_eq!(settled(&[0, 1, 2], &[1, 3, 4]), vec![4, 3, 1]);
  }

  /// The length of the longest increasing run in `values`.
  fn longest(values: &[usize]) -> usize {
    let mut ending = vec![1; values.len()];

    for right in 0..values.len() {
      for left in 0..right {
        if values[left] < values[right] {
          ending[right] = ending[right].max(ending[left] + 1);
        }
      }
    }

    ending.into_iter().max().unwrap_or(0)
  }

  /// Asserts that the entries `previous` names settle a longest increasing run of their
  /// previous positions, in descending new positions.
  fn assert_settles_a_longest_run(reorder: &mut Reorder, previous: &[usize]) {
    let new_positions: Vec<usize> = (0..previous.len()).map(|index| index * 2 + 1).collect();
    let settled = reorder.settled_positions(previous, &new_positions).to_vec();
    let entries: Vec<usize> = settled.iter().map(|position| (position - 1) / 2).collect();

    assert_eq!(settled.len(), longest(previous), "{previous:?}");
    assert!(
      entries.windows(2).all(|pair| pair[0] > pair[1]),
      "{previous:?}"
    );
    assert!(
      entries
        .windows(2)
        .all(|pair| previous[pair[1]] < previous[pair[0]]),
      "{previous:?}"
    );
  }

  /// Every order of up to seven kept entries settles a longest increasing run.
  #[test]
  fn every_small_order_settles_a_longest_run() {
    /// Every order of `items` from `taken` on, appended to `out`.
    fn permute(items: &mut Vec<usize>, taken: usize, out: &mut Vec<Vec<usize>>) {
      if taken == items.len() {
        out.push(items.clone());

        return;
      }

      for swap in taken..items.len() {
        items.swap(taken, swap);
        permute(items, taken + 1, out);
        items.swap(taken, swap);
      }
    }

    let mut reorder = Reorder::default();

    for count in 0..=7 {
      let mut all = Vec::new();
      let mut items: Vec<usize> = (0..count).map(|index| index * 3).collect();

      permute(&mut items, 0, &mut all);

      for previous in all {
        assert_settles_a_longest_run(&mut reorder, &previous);
      }
    }
  }

  #[cfg(not(target_arch = "wasm32"))]
  use proptest::prelude::*;

  #[cfg(not(target_arch = "wasm32"))]
  proptest! {
    /// Any run of keyed lists, repeated keys among them, reconciles to owners in the
    /// lists' order: an item kept from the list before keeps its owner, the nth item of a
    /// key matching the nth before it, and every owner left behind is handed back.
    #[test]
    fn any_run_of_lists_reconciles_like_its_model(
      lists in proptest::collection::vec(
        proptest::collection::vec(0..6_u8, 0..10),
        1..8,
      ),
    ) {
      let mut model = Model::default();
      let mut previous: Vec<(u8, u32)> = Vec::new();
      let mut item_of = HashMap::new();

      for list in lists {
        let (built, dropped) = model.run(&list);

        for owner in &built {
          item_of.insert(*owner, list[model.built_at[owner]]);
        }

        let items: Vec<u8> = model.content.iter().map(|owner| item_of[owner]).collect();

        prop_assert_eq!(&items, &list);

        // The nth occurrence of a key keeps the owner of the nth occurrence before it.
        let mut before: HashMap<u8, Vec<u32>> = HashMap::new();

        for (item, owner) in &previous {
          before.entry(*item).or_default().push(*owner);
        }

        let mut seen: HashMap<u8, usize> = HashMap::new();

        for (item, owner) in items.iter().zip(&model.content) {
          let occurrence = seen.entry(*item).or_insert(0);

          if let Some(kept) = before.get(item).and_then(|owners| owners.get(*occurrence)) {
            prop_assert_eq!(kept, owner, "the item keeps its owner");
          }

          *occurrence += 1;
        }

        for (_, owner) in &previous {
          prop_assert_eq!(
            !model.content.contains(owner),
            dropped.contains(owner),
            "an owner left behind is handed back, and only one"
          );
        }

        previous = items.into_iter().zip(model.content.iter().copied()).collect();
      }
    }

    /// Any order of up to three hundred kept entries settles a longest increasing run,
    /// the buffers reused from one order to the next.
    #[test]
    fn any_order_settles_a_longest_run(
      orders in proptest::collection::vec(
        (0..300_usize).prop_flat_map(|count| {
          Just((0..count).map(|index| index * 3).collect::<Vec<usize>>()).prop_shuffle()
        }),
        1..4,
      ),
    ) {
      let mut reorder = Reorder::default();

      for previous in orders {
        assert_settles_a_longest_run(&mut reorder, &previous);
      }
    }
  }
}
