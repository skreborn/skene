//! Boolean composition of conditions, shared by every facade.
//!
//! Each facade owns its condition enum—its leaves are typed to its medium—and adds
//! `And`/`Or` variants for composition. Implementing [`Composable`] (exposing the shape
//! of one level) gives it the shared machinery: [`holds`](Composable::holds) for a
//! platform that interprets conditions against live state, and
//! [`disjunctive_leaves`](Composable::disjunctive_leaves) for a platform that compiles
//! them into another language (the web platform's CSS rules).

/// One level of a composed condition: a leaf, or a combination of subconditions.
#[derive(Debug)]
pub enum ConditionShape<'condition, C> {
  /// A facade-specific leaf condition.
  Leaf(&'condition C),
  /// Holds when every subcondition holds; empty always holds.
  And(&'condition [C]),
  /// Holds when any subcondition holds; empty never holds.
  Or(&'condition [C]),
}

/// A condition type composed of facade-specific leaves under `and`/`or`.
pub trait Composable: Clone + Sized {
  /// This condition's top-level shape. A value reported as [`ConditionShape::Leaf`] must not be
  /// a composite.
  fn shape(&self) -> ConditionShape<'_, Self>;

  /// Whether the condition holds, with `leaf` deciding each leaf.
  fn holds(&self, leaf: &impl Fn(&Self) -> bool) -> bool {
    match self.shape() {
      ConditionShape::Leaf(condition) => leaf(condition),
      ConditionShape::And(subconditions) => subconditions
        .iter()
        .all(|subcondition| subcondition.holds(leaf)),
      ConditionShape::Or(subconditions) => subconditions
        .iter()
        .any(|subcondition| subcondition.holds(leaf)),
    }
  }

  /// The condition's disjunctive normal form: the conjunctions of leaves of which any
  /// one holding entirely makes the condition hold.
  ///
  /// An empty result never holds (`or` of nothing); an empty conjunction within the
  /// result always holds (`and` of nothing). A compiling platform emits one rule per
  /// conjunction.
  fn disjunctive_leaves(&self) -> Vec<Vec<&Self>> {
    match self.shape() {
      ConditionShape::Leaf(condition) => vec![vec![condition]],
      ConditionShape::Or(subconditions) => subconditions
        .iter()
        .flat_map(Self::disjunctive_leaves)
        .collect(),
      ConditionShape::And(subconditions) => {
        subconditions
          .iter()
          .fold(vec![Vec::new()], |terms, subcondition| {
            let subterms = subcondition.disjunctive_leaves();

            terms
              .iter()
              .flat_map(|term| {
                subterms.iter().map(move |subterm| {
                  term
                    .iter()
                    .copied()
                    .chain(subterm.iter().copied())
                    .collect()
                })
              })
              .collect()
          })
      }
    }
  }
}
