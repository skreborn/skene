//! The pure minimal-reorder decision, with no platform or reactive involvement.

use longest_increasing_subsequence::lis_with;

/// Decides which kept entries a reorder can leave in place.
///
/// Holds its working buffers so a repeated reconcile allocates nothing. Platform-agnostic:
/// a facade's reconciliation feeds it positions and applies the result with its own tree
/// operations.
#[derive(Default)]
#[must_use]
pub struct Reorder {
  subsequence: Vec<usize>,
  predecessors: Vec<usize>,
  starts: Vec<usize>,
  settled: Vec<usize>,
}

impl Reorder {
  /// The positions in the new order that need no re-insertion.
  ///
  /// `previous_positions[i]` is the previous position of the kept entry now at
  /// `new_positions[i]`; both are in the new order. Entries on the longest increasing run of
  /// previous positions are already in the right relative order and can stay put — every
  /// other position must be re-inserted before its successor, which is what makes the
  /// reorder minimal.
  ///
  /// The result is in descending order, matching the back-to-front insertion walk.
  pub fn settled_positions(
    &mut self,
    previous_positions: &[usize],
    new_positions: &[usize],
  ) -> &[usize] {
    let kept = previous_positions.len();

    self.subsequence.clear();
    self.predecessors.clear();
    self.predecessors.resize(kept, 0);
    self.starts.clear();
    self.starts.resize(kept, 0);

    lis_with(
      previous_positions,
      &mut self.subsequence,
      |a, b| a < b,
      self.predecessors.as_mut_slice(),
      self.starts.as_mut_slice(),
    );

    self.settled.clear();
    self
      .settled
      .extend(self.subsequence.iter().map(|index| new_positions[*index]));

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
}
