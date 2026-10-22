//! The controlled contract as the editing core meets it: every change a reader makes is
//! proposed before it stands, and the truth the app holds decides.

use std::fmt;

use crate::{Edit, Verdict};

/// What decides a reader's change to an entry: the app's truth.
///
/// Outside a composition the core proposes a change before applying it; during one it applies
/// the change first (`applied`), so an input method is never interrupted, and settles at the
/// composition's end.
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not decide an entry's changes",
  label = "an entry's changes are proposed to an arbiter here",
  note = "the controlled contract is `ControlledText`: a truth and a handler"
)]
pub trait Arbiter {
  /// The reader changed the entry to `proposed`, which the entry shows already where
  /// `applied`, while an input method composes where `composing`: the verdict.
  ///
  /// [`Verdict::Accept`] keeps the proposal; [`Verdict::Replace`] shows its [`Edit`]
  /// instead.
  fn propose(&mut self, proposed: &Edit, applied: bool, composing: bool) -> Verdict;

  /// A composition ended: the truth to show where a change during it was held.
  fn settle(&mut self) -> Option<Edit>;
}

/// The controlled contract over a truth and a handler: the handler hears every change, and
/// the truth after it is what the entry shows.
///
/// - A proposal equal to the truth is no change: the handler does not run, and it is accepted.
/// - A proposal the handler makes the truth is accepted.
/// - A proposal it does not is replaced by the truth (vetoed, or transformed), at once outside a
///   composition, and at the composition's end inside one.
#[must_use]
pub struct ControlledText {
  truth: Box<dyn Fn() -> Edit>,
  handler: Box<dyn FnMut(Edit)>,
  held: bool,
}

impl ControlledText {
  /// The contract over `truth` (read without subscribing) and `handler`.
  pub fn new(truth: impl Fn() -> Edit + 'static, handler: impl FnMut(Edit) + 'static) -> Self {
    Self {
      truth: Box::new(truth),
      handler: Box::new(handler),
      held: false,
    }
  }
}

impl fmt::Debug for ControlledText {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("ControlledText")
      .field("held", &self.held)
      .finish_non_exhaustive()
  }
}

impl Arbiter for ControlledText {
  fn propose(&mut self, proposed: &Edit, applied: bool, composing: bool) -> Verdict {
    if (self.truth)().holds(proposed) {
      return Verdict::Accept;
    }

    (self.handler)(proposed.clone());

    let truth = (self.truth)();

    if truth.holds(proposed) {
      Verdict::Accept
    }
    else if applied && composing {
      self.held = true;

      Verdict::Accept
    }
    else {
      Verdict::Replace(truth)
    }
  }

  fn settle(&mut self) -> Option<Edit> {
    std::mem::take(&mut self.held).then(|| (self.truth)())
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use skene_reactive::{Get as _, Set as _, Signal};

  use super::{Arbiter as _, ControlledText};
  use crate::{Edit, Verdict};

  /// A contract over a signal whose handler keeps what `keep` maps a proposal to, counting
  /// its runs.
  fn contract(
    value: &str,
    keep: impl Fn(&str) -> Option<String> + 'static,
  ) -> (ControlledText, Signal<String>, Rc<Cell<usize>>) {
    let truth = Signal::new(value.to_owned());
    let runs = Rc::new(Cell::new(0));
    let counted = runs.clone();
    let contract = ControlledText::new(
      move || Edit::new(truth.get_untracked()),
      move |edit| {
        counted.set(counted.get() + 1);

        if let Some(kept) = keep(&edit.text) {
          truth.set(kept);
        }
      },
    );

    (contract, truth, runs)
  }

  #[test]
  fn a_proposal_the_truth_holds_is_accepted_without_the_handler() {
    let (mut contract, _, runs) = contract("ab", |text| Some(text.to_owned()));

    assert_eq!(
      contract.propose(&Edit::new("ab"), false, false),
      Verdict::Accept
    );
    assert_eq!(runs.get(), 0);

    assert_eq!(
      contract.propose(&Edit::new("abc"), false, false),
      Verdict::Accept
    );
    assert_eq!(runs.get(), 1);
  }

  #[test]
  fn a_proposal_the_handler_does_not_keep_is_replaced_by_the_truth() {
    let (mut contract, truth, _) = contract("ab", |text| Some(text.to_uppercase()));

    assert_eq!(
      contract.propose(&Edit::new("abc"), false, false),
      Verdict::Replace(Edit::new("ABC"))
    );
    assert_eq!(truth.get_untracked(), "ABC");
    assert_eq!(contract.settle(), None, "nothing was held");
  }

  #[test]
  fn a_composition_keeps_its_change_and_settles_to_the_truth() {
    let (mut contract, _, _) = contract("", |_| None);

    assert_eq!(
      contract.propose(&Edit::new("k"), true, true),
      Verdict::Accept,
      "an input method is not interrupted"
    );
    assert_eq!(contract.settle(), Some(Edit::new("")));
    assert_eq!(contract.settle(), None, "settled once");
  }
}
