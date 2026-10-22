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

  /// Whether the truth holds `proposed`: the same text, and the same selection where the
  /// truth controls it.
  fn holds(proposed: &Edit, truth: &Edit) -> bool {
    proposed.text == truth.text
      && truth
        .selection
        .is_none_or(|selection| proposed.selection == Some(selection))
  }
}

impl fmt::Debug for ControlledText {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("ControlledText")
      .field("truth", &(self.truth)())
      .field("held", &self.held)
      .finish_non_exhaustive()
  }
}

impl Arbiter for ControlledText {
  fn propose(&mut self, proposed: &Edit, applied: bool, composing: bool) -> Verdict {
    if Self::holds(proposed, &(self.truth)()) {
      return Verdict::Accept;
    }

    (self.handler)(proposed.clone());

    let truth = (self.truth)();

    if Self::holds(proposed, &truth) {
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
