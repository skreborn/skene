//! The pure minimal-reorder decision, with no platform or reactive involvement.

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

#[cfg(test)]
mod tests {
  use super::Reorder;

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
