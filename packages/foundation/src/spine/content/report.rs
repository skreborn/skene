//! A controlled content's two paths, written once for every platform: the truth
//! written to the platform, and a reader's change reported back and answered.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_reactive::{Effect, batch, untrack};

use super::{Content, ContentValue, contents};
use crate::spine::{
  Edit,
  Handler,
  NodeId,
  Realization,
  Scene,
  TextSelection,
  Verdict,
  task::run_handler,
};

/// A reader's change a realization reports at a node.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum Report<'a> {
  /// The reader changed `content` to `proposed`, which the toolkit shows already where
  /// `applied`; a moved caret is a change of [`Content::Edit`] whose text is the one
  /// shown.
  Change {
    /// The content changed.
    content: Content,
    /// The value the reader proposes.
    proposed: ContentValue<'a>,
    /// Whether the toolkit shows the proposal already.
    applied: bool,
  },
}

/// Reports `report` at `node` and answers with the verdict the realization applies.
///
/// The node's [`Controller`] of the content answers, alone and in one batch: a toolkit
/// that has not applied the change applies the verdict in its place, and one that has
/// is repaired to the verdict's truth as the smallest edit. A content no controller
/// holds stands as [`Verdict::Accept`].
pub fn report<P: Realization>(node: NodeId, report: Report<'_>) -> Verdict {
  let Report::Change {
    content,
    proposed,
    applied,
  } = report;
  let Some(controller) = Scene::<P>::read(|scene| scene.handlers(node)?.reporter(content))
  else {
    return Verdict::Accept;
  };

  batch(|| run_handler(controller.owner(), || controller(proposed, applied)))
}

/// The truth a controlled entry holds: its text and its selection, each `None` where
/// the app leaves it to the reader.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use]
pub struct EditTruth {
  /// The text, where it is derived.
  pub text: Option<Rc<str>>,
  /// The selection, in grapheme clusters of the text, where it is derived.
  pub selection: Option<TextSelection>,
}

impl EditTruth {
  /// The truth `text` and `selection` hold.
  #[inline(always)]
  pub const fn new(text: Option<Rc<str>>, selection: Option<TextSelection>) -> Self {
    Self { text, selection }
  }

  /// Whether the app derives nothing: the entry is the reader's.
  #[inline(always)]
  #[must_use]
  pub const fn is_free(&self) -> bool {
    self.text.is_none() && self.selection.is_none()
  }

  /// The edit to write so the entry shows the truth over `shown`: a text the truth leaves
  /// free is the one shown; `None` where nothing is derived, where a selection is
  /// derived beside no text and nothing is shown yet, where `shown` holds the truth, and
  /// while the reader's text is `composing`, the write waiting for the report that ends
  /// it.
  fn written(&self, shown: Option<&Edit>, composing: bool) -> Option<Edit> {
    if composing {
      return None;
    }

    let text = match &self.text {
      Some(text) => Rc::clone(text),
      None if self.selection.is_some() => Rc::clone(&shown?.text),
      None => return None,
    };
    let edit = Edit {
      text,
      selection: self.selection,
    };

    (!shown.is_some_and(|shown| edit.holds(shown))).then_some(edit)
  }

  /// What a reader's `proposed` change asks of the app over `shown`: each part it
  /// changes from the truth's, or from the one shown where the truth leaves the part
  /// free.
  fn asked(&self, shown: Option<&Edit>, proposed: &Edit) -> Asked {
    let text = self
      .text
      .as_deref()
      .or_else(|| shown.map(|shown| &*shown.text));
    let selection = self
      .selection
      .or_else(|| shown.and_then(|shown| shown.selection));

    Asked {
      text: (text != Some(&*proposed.text)).then(|| Rc::clone(&proposed.text)),
      selection: proposed
        .selection
        .filter(|proposed| selection != Some(*proposed)),
    }
  }

  /// The verdict on `proposed` once asked: it stands where the truth holds it or derives
  /// nothing, and is replaced by the truth otherwise, a text the truth leaves free being
  /// the reader's.
  fn verdict(&self, proposed: &Edit) -> Verdict {
    if self.is_free() {
      return Verdict::Accept;
    }

    let truth = Edit {
      text: self
        .text
        .clone()
        .unwrap_or_else(|| Rc::clone(&proposed.text)),
      selection: self.selection,
    };

    if truth.holds(proposed) {
      Verdict::Accept
    }
    else {
      Verdict::Replace(truth)
    }
  }
}

/// What a reader's change asks of the app: each part of an entry it changed that the
/// truth does not hold, the text first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use]
pub struct Asked {
  /// The text asked for, where the change asks for one.
  pub text: Option<Rc<str>>,
  /// The selection asked for, where the change asks for one.
  pub selection: Option<TextSelection>,
}

impl Asked {
  /// Whether the change asks for nothing.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.text.is_none() && self.selection.is_none()
  }
}

/// What an entry shows after a report of `proposed` answered `verdict`, as far as its
/// controller knows, `applied` saying whether its toolkit showed the change already.
///
/// A toolkit that applied the change shows it while the verdict accepts it, and the
/// truth it is repaired to otherwise; one that has not applied it shows it accepted, and
/// what it shows instead of it is not known until the truth is written or reported.
fn shown_after(proposed: &Edit, applied: bool, verdict: &Verdict) -> Option<Edit> {
  match verdict {
    Verdict::Accept => Some(proposed.clone()),
    Verdict::Replace(truth) if applied => Some(Edit {
      text: Rc::clone(&truth.text),
      selection: truth.selection,
    }),
    Verdict::Replace(_) => None,
  }
}

/// The answer to a reader's change: what it asks of the app, and the verdict on it once
/// asked.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answer {
  asked: Asked,
  verdict: Verdict,
}

impl Answer {
  /// The answer to `proposed` over `shown`: `truth` reads the truth as it stands, before
  /// the asks and after them, and `ask` asks the app.
  ///
  /// A disabled entry asks nothing. A change reported while the truth is `writing` is its
  /// echo: it stands, asking only for a selection the write moved that differs from the
  /// truth's, or from the one shown where the truth leaves it free, the text being the
  /// write's own. While the reader's text is `composing` the change stands and the
  /// repair waits for the report that ends the composition.
  fn of(
    truth: impl Fn() -> EditTruth,
    shown: Option<&Edit>,
    proposed: &Edit,
    disabled: bool,
    writing: bool,
    composing: bool,
    ask: impl FnOnce(&Asked),
  ) -> Self {
    let asked = if disabled {
      Asked::default()
    }
    else if writing {
      Asked {
        text: None,
        selection: truth().asked(shown, proposed).selection,
      }
    }
    else {
      truth().asked(shown, proposed)
    };

    if !asked.is_empty() {
      ask(&asked);
    }

    let verdict = if writing || composing {
      Verdict::Accept
    }
    else {
      truth().verdict(proposed)
    };

    Self { asked, verdict }
  }
}

/// A text entry's controlled [`Edit`]: the truth the platform is brought back to, and
/// the ask a reader's change makes of the app.
///
/// [`register`](Self::register) writes the truth as the entry's content whenever what it
/// reads changes, skipping a write of what the entry shows already and holding one back
/// while the reader's text is [composing](Realization::composing); and it answers every
/// reader's change [`report`]ed at the node: one the truth holds already asks nothing
/// and stands, and so does one made while it writes, but for a selection the write moved,
/// which it asks for; any other asks the app for each part it changes, the text then the
/// selection, and stands where the truth then holds it, replaced by the truth otherwise
/// ([`Verdict`]). A disabled entry asks nothing and is answered with its truth.
#[must_use]
pub struct Controller {
  truth: Box<dyn Fn() -> EditTruth>,
  ask: Box<dyn Fn(&Asked)>,
  disabled: Option<Box<dyn Fn() -> bool>>,
  shown: RefCell<Option<Edit>>,
  writing: Cell<bool>,
}

impl std::fmt::Debug for Controller {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Controller")
      .field("shown", &self.shown)
      .finish_non_exhaustive()
  }
}

impl Controller {
  /// The controller of an entry whose truth `truth` reads, tracking what it reads, and
  /// whose reader's changes `ask` asks the app for.
  pub fn new(truth: impl Fn() -> EditTruth + 'static, ask: impl Fn(&Asked) + 'static) -> Self {
    Self {
      truth: Box::new(truth),
      ask: Box::new(ask),
      disabled: None,
      shown: RefCell::new(None),
      writing: Cell::new(false),
    }
  }

  /// Answers every change with the truth, asking nothing, while `disabled` says so.
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Some(Box::new(disabled));

    self
  }

  /// Controls the [`Edit`] content of `node` through `P`, as the current owner: the
  /// truth is written now and whenever it changes, and the node's reports of the content
  /// are answered by this controller from now on; a second controller of the content is
  /// a debug assertion.
  pub fn register<P: Realization>(self, node: NodeId) {
    let controller = Rc::new(self);
    let answering = Rc::clone(&controller);

    *controller.shown.borrow_mut() = contents::EDIT.read::<P>(node);
    Scene::<P>::with(|scene| {
      debug_assert!(
        scene
          .handlers(node)
          .and_then(|handlers| handlers.reporter(Content::Edit))
          .is_none(),
        "a node's content has one controller"
      );
      scene.handle(
        node,
        Handler::Report(
          Content::Edit,
          Rc::new(move |proposed, applied| answering.answer::<P>(node, proposed, applied)),
        ),
      );
    });
    Effect::render(move |_: Option<()>| controller.write::<P>(node));
  }

  /// Writes the truth to `node` unless it is shown already or held back while the
  /// reader's text is composing; a selection the truth leaves free is noted as the one
  /// shown before the write.
  fn write<P: Realization>(&self, node: NodeId) {
    let truth = (self.truth)();

    untrack(|| {
      let shown = self.shown.borrow().clone();
      let Some(edit) = truth.written(shown.as_ref(), P::composing(node))
      else {
        return;
      };

      *self.shown.borrow_mut() = Some(Edit {
        text: Rc::clone(&edit.text),
        selection: edit
          .selection
          .or_else(|| shown.and_then(|shown| shown.selection)),
      });
      self.writing.set(true);
      contents::EDIT.write::<P>(node, &edit);
      self.writing.set(false);
    });
  }

  /// The verdict on the reader's change `proposed` of `node`, which its toolkit shows
  /// already where `applied`.
  fn answer<P: Realization>(
    &self,
    node: NodeId,
    proposed: ContentValue<'_>,
    applied: bool,
  ) -> Verdict {
    let ContentValue::Edit(proposed) = proposed
    else {
      return Verdict::Accept;
    };

    let shown = self.shown.borrow().clone();
    let answer = untrack(|| {
      Answer::of(
        || (self.truth)(),
        shown.as_ref(),
        proposed,
        self.disabled.as_ref().is_some_and(|disabled| disabled()),
        self.writing.get(),
        P::composing(node),
        |asked| (self.ask)(asked),
      )
    });

    *self.shown.borrow_mut() = shown_after(proposed, applied, &answer.verdict);

    answer.verdict
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use super::{Answer, Asked, EditTruth, shown_after};
  use crate::spine::{Edit, TextSelection, Unit, Verdict};

  /// The truth of an entry deriving `text`, and `caret` where given.
  fn truth(text: &str, caret: Option<usize>) -> EditTruth {
    EditTruth::new(Some(Rc::from(text)), caret.map(TextSelection::collapsed))
  }

  /// `text` with the caret at `caret`.
  fn edit(text: &str, caret: usize) -> Edit {
    Edit::new(text).with_selection(TextSelection::collapsed(caret))
  }

  /// The state of the entry a change is reported in.
  #[derive(Clone, Copy, Default)]
  struct During {
    disabled: bool,
    writing: bool,
    composing: bool,
  }

  /// The answer to `proposed` over `shown` while the truth reads `truth`, the asks
  /// collected rather than run.
  fn answer_during(
    truth: &EditTruth,
    shown: Option<&Edit>,
    proposed: &Edit,
    during: During,
  ) -> (Answer, Vec<Asked>) {
    let asks = RefCell::new(Vec::new());
    let answer = Answer::of(
      || truth.clone(),
      shown,
      proposed,
      during.disabled,
      during.writing,
      during.composing,
      |asked| asks.borrow_mut().push(asked.clone()),
    );

    (answer, asks.into_inner())
  }

  /// The answer to `proposed` over `shown` in an enabled entry, the truth not writing,
  /// while `composing` says so.
  fn answer(
    truth: &EditTruth,
    shown: Option<&Edit>,
    proposed: &Edit,
    disabled: bool,
    composing: bool,
  ) -> (Answer, Vec<Asked>) {
    answer_during(
      truth,
      shown,
      proposed,
      During {
        disabled,
        composing,
        ..During::default()
      },
    )
  }

  /// A change the truth holds already, as the echo of the truth's own write, asks
  /// nothing and stands; so does a moved caret the truth leaves free and the entry shows.
  #[test]
  fn an_echo_asks_nothing_and_stands() {
    let held = truth("ab", Some(1));
    let (answered, asks) = answer(&held, None, &edit("ab", 1), false, false);

    assert_eq!(answered.verdict, Verdict::Accept);
    assert_eq!(asks, Vec::<Asked>::new());

    let free = truth("ab", None);
    let (answered, asks) = answer(&free, Some(&edit("ab", 2)), &edit("ab", 2), false, false);

    assert_eq!(answered.verdict, Verdict::Accept);
    assert_eq!(asks, Vec::<Asked>::new());
  }

  /// A change reported while the truth is written is its echo: it stands and asks
  /// nothing of the text, even one the write did not leave as written, and asks only
  /// for a selection the write moved.
  #[test]
  fn a_change_reported_while_writing_is_an_echo() {
    let writing = During {
      writing: true,
      ..During::default()
    };
    let held = truth("hi", Some(1));
    let (answered, asks) = answer_during(&held, Some(&edit("hi", 1)), &edit("hi", 1), writing);

    assert_eq!(answered.verdict, Verdict::Accept);
    assert_eq!(asks, Vec::<Asked>::new());

    let (answered, asks) = answer_during(&held, Some(&edit("hi", 1)), &edit("h", 1), writing);

    assert_eq!(
      answered.verdict,
      Verdict::Accept,
      "never repaired mid-write"
    );
    assert_eq!(asks, Vec::<Asked>::new());

    let free = truth("hi", None);
    let (answered, asks) = answer_during(&free, Some(&edit("hi", 5)), &edit("hi", 2), writing);

    assert_eq!(answered.verdict, Verdict::Accept);
    assert_eq!(
      asks,
      [Asked {
        text: None,
        selection: Some(TextSelection::collapsed(2)),
      }],
      "the caret the write moved is asked for"
    );

    let (_, asks) = answer_during(&free, Some(&edit("hi", 2)), &edit("hi", 2), writing);

    assert_eq!(asks, Vec::<Asked>::new(), "an unmoved caret asks nothing");
  }

  /// A caret moved in an entry that derives its selection asks for the move and, where
  /// nothing takes it, is answered with the truth's selection.
  #[test]
  fn a_derived_selection_brings_a_moved_caret_back() {
    let held = truth("ab", Some(1));
    let (answered, asks) = answer(&held, Some(&edit("ab", 1)), &edit("ab", 2), false, false);

    assert_eq!(
      asks,
      [Asked {
        text: None,
        selection: Some(TextSelection::collapsed(2)),
      }]
    );
    assert_eq!(answered.verdict, Verdict::Replace(edit("ab", 1)));
  }

  /// A change asks for each part it changes: the text where it differs from the truth's,
  /// and a selection the truth leaves free where it differs from the one shown.
  #[test]
  fn a_change_asks_for_each_part_it_changes() {
    let free = truth("ab", None);
    let (_, asks) = answer(&free, Some(&edit("ab", 2)), &edit("ab", 1), false, false);

    assert_eq!(
      asks,
      [Asked {
        text: None,
        selection: Some(TextSelection::collapsed(1)),
      }],
      "a moved caret asks for the selection alone"
    );

    let (_, asks) = answer(&free, Some(&edit("ab", 2)), &edit("abc", 3), false, false);

    assert_eq!(
      asks,
      [Asked {
        text: Some(Rc::from("abc")),
        selection: Some(TextSelection::collapsed(3)),
      }]
    );

    let reader = EditTruth::default();
    let (answered, asks) = answer(&reader, None, &edit("typed", 5), false, false);

    assert_eq!(answered.verdict, Verdict::Accept, "an uncontrolled entry");
    assert_eq!(asks[0].text.as_deref(), Some("typed"));
  }

  /// The truth is written only where the entry does not show it already, and a
  /// selection derived beside no text is placed in the text shown.
  #[test]
  fn an_equal_write_is_skipped() {
    let held = truth("ab", Some(2));

    assert_eq!(held.written(Some(&edit("ab", 2)), false), None);
    assert_eq!(
      held.written(Some(&edit("ab", 1)), false),
      Some(edit("ab", 2))
    );
    assert_eq!(held.written(None, false), Some(edit("ab", 2)));
    assert_eq!(
      truth("ab", None).written(Some(&edit("ab", 1)), false),
      None,
      "a free selection is not written"
    );
    assert_eq!(
      truth("ab", None).written(Some(&edit("a", 1)), false),
      Some(Edit::new("ab"))
    );

    let caret = EditTruth::new(None, Some(TextSelection::collapsed(1)));

    assert_eq!(
      caret.written(None, false),
      None,
      "nothing shown to place it in"
    );
    assert_eq!(
      caret.written(Some(&edit("xyz", 3)), false),
      Some(edit("xyz", 1))
    );
    assert_eq!(
      EditTruth::default().written(Some(&edit("x", 0)), false),
      None
    );
  }

  /// While the reader's text composes, a write of the truth waits, and a change the
  /// truth refuses still asks but stands, its repair held back; the report ending the
  /// composition settles both.
  #[test]
  fn a_composition_holds_the_repair_until_it_ends() {
    let held = truth("AB", Some(2));
    let composed = edit("abx", 3);

    assert_eq!(
      held.written(Some(&composed), true),
      None,
      "the write waits for the composition"
    );
    let (answered, asks) = answer(&held, Some(&edit("AB", 2)), &composed, false, true);

    assert_eq!(
      answered.verdict,
      Verdict::Accept,
      "the input method is not interrupted"
    );
    assert_eq!(asks.len(), 1);
    assert_eq!(
      shown_after(&composed, true, &answered.verdict),
      Some(composed.clone()),
      "the entry shows the composed text"
    );

    let (answered, _) = answer(&held, Some(&composed), &composed, false, false);

    assert_eq!(answered.verdict, Verdict::Replace(edit("AB", 2)));
    assert_eq!(
      shown_after(&composed, true, &answered.verdict),
      Some(edit("AB", 2)),
      "the composition's end repairs it"
    );
  }

  /// A disabled entry asks nothing and answers every change with its truth, but one its
  /// truth holds.
  #[test]
  fn a_disabled_entry_answers_with_its_truth() {
    let held = truth("12", None);
    let (answered, asks) = answer(&held, None, &Edit::new("123"), true, false);

    assert_eq!(asks, Vec::<Asked>::new());
    assert_eq!(answered.verdict, Verdict::Replace(Edit::new("12")));

    let (answered, _) = answer(&held, None, &Edit::new("12"), true, false);

    assert_eq!(answered.verdict, Verdict::Accept);
    assert_eq!(
      answer(&EditTruth::default(), None, &Edit::new("x"), true, false)
        .0
        .verdict,
      Verdict::Accept,
      "an uncontrolled entry keeps no truth"
    );
  }

  /// A whole-text transform is answered with the truth's text alone, so the caret is
  /// mapped through the smallest edit to it and stays where the reader left it.
  #[test]
  fn a_transform_keeps_the_caret_through_the_smallest_edit() {
    let transformed = truth("a-b", None);
    let typed = edit("a--b", 2);
    let (answered, _) = answer(&transformed, Some(&edit("a-b", 1)), &typed, false, false);
    let Verdict::Replace(truth) = answered.verdict
    else {
      panic!("the transform replaces the change");
    };

    assert_eq!(
      truth,
      Edit::new("a-b"),
      "the selection is left to be mapped"
    );

    let (replaced, inserted) = truth.splice(&typed.text, Unit::CodePoint);

    assert_eq!(
      Edit::map_through(2, replaced, inserted.chars().count()),
      2,
      "the caret stays after the first dash"
    );
    assert_eq!(
      shown_after(&typed, false, &Verdict::Replace(truth)),
      None,
      "what a toolkit applying the truth shows is known once written or reported"
    );
  }
}
