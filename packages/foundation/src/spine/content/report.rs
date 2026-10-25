//! A controlled content's two paths, written once for every platform: the truth
//! written to the platform, and a reader's change reported back and answered, by a
//! [`Controller`] for a content held whole and an [`EditController`] for an entry's
//! [`Edit`].

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_reactive::{Effect, batch, untrack};

use super::{Choices, Content, ContentKey, ContentOps, ContentType, ContentValue, Range, contents};
use crate::spine::{
  CheckState,
  Edit,
  Handler,
  NodeId,
  Realization,
  ReportHandler,
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
/// The node's controller of the content answers, alone and in one batch: a toolkit
/// that has not applied an edit applies the verdict in its place, and one that has is
/// repaired to the verdict's truth as the smallest edit; a content held whole is
/// repaired by its [`Controller`]'s own write of the truth, answered
/// [`Verdict::Refuse`]. A content no controller holds stands as [`Verdict::Accept`].
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
/// [`register_with`](Self::register_with) writes the truth as the entry's content,
/// through the realization its [`ContentOps`] reach, whenever what it reads changes,
/// skipping a write of what the entry shows already and holding one back while the
/// reader's text is [composing](Realization::composing); and it answers every reader's
/// change [`report`]ed at the node: one the truth holds already asks nothing and stands,
/// and so does one made while it writes, but for a selection the write moved, which it
/// asks for; any other asks the app for each part it changes, the text then the
/// selection, and stands where the truth then holds it, replaced by the truth otherwise
/// ([`Verdict`]). A disabled entry asks nothing and is answered with its truth.
#[must_use]
pub struct EditController {
  truth: Box<dyn Fn() -> EditTruth>,
  ask: Box<dyn Fn(&Asked)>,
  disabled: Option<Box<dyn Fn() -> bool>>,
  shown: RefCell<Option<Edit>>,
  writing: Cell<bool>,
}

impl std::fmt::Debug for EditController {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("EditController")
      .field("shown", &self.shown)
      .finish_non_exhaustive()
  }
}

impl EditController {
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

  /// Controls the [`Edit`] content of `node` through `P`, as the current owner; see
  /// [`register_with`](Self::register_with).
  #[inline(always)]
  pub fn register<P: Realization>(self, node: NodeId) {
    self.register_with(ContentOps::of::<P>(), node);
  }

  /// Controls the [`Edit`] content of `node` through the realization `ops` reaches, as
  /// the current owner: the truth is written now and whenever it changes, and the node's
  /// reports of the content are answered by this controller from now on; a second
  /// controller of the content is a debug assertion.
  pub fn register_with(self, ops: ContentOps, node: NodeId) {
    let controller = Rc::new(self);
    let answering = Rc::clone(&controller);

    *controller.shown.borrow_mut() = contents::EDIT.read_with(ops, node);

    (ops.install)(
      node,
      Content::Edit,
      Rc::new(move |proposed, applied| answering.answer(ops, node, proposed, applied)),
    );

    Effect::render(move |_: Option<()>| controller.write(ops, node));
  }

  /// Writes the truth to `node` unless it is shown already or held back while the
  /// reader's text is composing; a selection the truth leaves free is noted as the one
  /// shown before the write.
  fn write(&self, ops: ContentOps, node: NodeId) {
    let truth = (self.truth)();

    untrack(|| {
      let shown = self.shown.borrow().clone();
      let Some(edit) = truth.written(shown.as_ref(), (ops.composing)(node))
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
      contents::EDIT.write_with(ops, node, &edit);
      self.writing.set(false);
    });
  }

  /// The verdict on the reader's change `proposed` of `node`, which its toolkit shows
  /// already where `applied`.
  fn answer(
    &self,
    ops: ContentOps,
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
        (ops.composing)(node),
        |asked| (self.ask)(asked),
      )
    });

    *self.shown.borrow_mut() = Self::shown_after(proposed, applied, &answer.verdict);

    answer.verdict
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
      Verdict::Replace(_) | Verdict::Refuse => None,
    }
  }
}

/// Installs `answer` as the answer to `node`'s reports of `content` in `P`'s scene, as
/// the current owner; a second controller of the content is a debug assertion.
pub(in crate::spine) fn install<P: Realization>(
  node: NodeId,
  content: Content,
  answer: Rc<ReportHandler>,
) {
  Scene::<P>::with(|scene| {
    debug_assert!(
      scene
        .handlers(node)
        .and_then(|handlers| handlers.reporter(content))
        .is_none(),
      "a node's content has one controller"
    );

    scene.handle(node, Handler::Report(content, answer));
  });
}

/// A content a [`Controller`] holds whole, written at once: what of a reader's change of
/// it the truth must hold for the change to stand.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a content a controller holds whole",
  label = "a controller holds a checked state, a range, choices, a group's chosen key or a door's value",
  note = "a text entry's `Edit` is controlled by an `EditController`"
)]
pub trait Controllable: ContentType {
  /// Whether the truth `self` holds a reader's `proposed` change: every part of it a
  /// reader changes is the truth's.
  fn holds(&self, proposed: &Self) -> bool;
}

impl Controllable for CheckState {
  #[inline(always)]
  fn holds(&self, proposed: &Self) -> bool {
    self == proposed
  }
}

/// A reader changes a range's value; its bounds and its step are the app's.
impl Controllable for Range {
  #[inline(always)]
  fn holds(&self, proposed: &Self) -> bool {
    (self.value - proposed.value).abs() <= f32::EPSILON
  }
}

/// A reader changes which option is chosen; the options are the app's.
impl Controllable for Choices {
  #[inline(always)]
  fn holds(&self, proposed: &Self) -> bool {
    self.chosen == proposed.chosen
  }
}

impl Controllable for Option<Rc<str>> {
  #[inline(always)]
  fn holds(&self, proposed: &Self) -> bool {
    self == proposed
  }
}

/// A controlled content held whole: the truth the platform is brought back to, and the
/// ask a reader's change makes of the app.
///
/// The content is a checked state, a range, a choice's options, a group's chosen key or
/// a value of a view adopted through the native door.
///
/// [`register_with`](Self::register_with) writes the truth as the node's content,
/// through the realization its [`ContentOps`] reach, whenever what it reads changes,
/// skipping a write of what the node shows already, which it reads once through
/// [`Realization::read`] and then keeps from what it writes and what is reported, and
/// holding one back while the reader's change is [composing](Realization::composing); a
/// door's content is written, read and composed through the
/// [`ControlSink`](crate::ControlSink) registered on its node instead
/// ([`ContentKey::door`]). It answers every reader's change [`report`]ed at the node: one
/// the truth holds ([`Controllable::holds`]) asks nothing and stands, and so does one
/// made while it writes; any other asks the app, and stands where the truth then holds
/// it, else is refused and repaired by the write of the truth ([`Verdict::Refuse`]). A
/// disabled control asks nothing and is answered with its truth.
#[must_use]
pub struct Controller<T: Controllable> {
  key: ContentKey<T>,
  truth: Box<dyn Fn() -> T>,
  ask: Box<dyn Fn(&T)>,
  disabled: Option<Box<dyn Fn() -> bool>>,
  shown: RefCell<Option<T>>,
  writing: Cell<bool>,
}

impl<T: Controllable> std::fmt::Debug for Controller<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Controller")
      .field("key", &self.key)
      .field("shown", &self.shown)
      .finish_non_exhaustive()
  }
}

impl<T: Controllable> Controller<T> {
  /// The controller of the content `key` writes, whose truth `truth` reads, tracking
  /// what it reads, and whose reader's changes `ask` asks the app for.
  pub fn new(
    key: ContentKey<T>,
    truth: impl Fn() -> T + 'static,
    ask: impl Fn(&T) + 'static,
  ) -> Self {
    Self {
      key,
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

  /// Controls the content of `node` through `P`, as the current owner; see
  /// [`register_with`](Self::register_with).
  #[inline(always)]
  pub fn register<P: Realization>(self, node: NodeId) {
    self.register_with(ContentOps::of::<P>(), node);
  }

  /// Controls the content of `node` through the realization `ops` reaches, as the
  /// current owner: the truth is written now and whenever it changes, and the node's
  /// reports of the content are answered by this controller from now on; a second
  /// controller of the content is a debug assertion.
  pub fn register_with(self, ops: ContentOps, node: NodeId) {
    let controller = Rc::new(self);
    let answering = Rc::clone(&controller);

    *controller.shown.borrow_mut() = controller.key.read_with(ops, node);

    (ops.install)(
      node,
      controller.key.content(),
      Rc::new(move |proposed, applied| answering.answer(ops, node, proposed, applied)),
    );

    Effect::render(move |_: Option<()>| controller.write(ops, node));
  }

  /// Writes the truth to `node` unless it is shown already or held back while the
  /// reader's change is composing.
  fn write(&self, ops: ContentOps, node: NodeId) {
    let truth = (self.truth)();

    untrack(|| {
      let Some(truth) = Self::written(
        truth,
        self.shown.borrow().as_ref(),
        self.key.is_composing_with(ops, node),
      )
      else {
        return;
      };

      self.writing.set(true);
      self.key.write_with(ops, node, &truth);
      self.writing.set(false);
      *self.shown.borrow_mut() = Some(truth);
    });
  }

  /// The verdict on the reader's change `proposed` of `node`, which its toolkit shows
  /// already where `applied`; a refused change is repaired by the write of the truth.
  fn answer(
    &self,
    ops: ContentOps,
    node: NodeId,
    proposed: ContentValue<'_>,
    applied: bool,
  ) -> Verdict {
    let Some(proposed) = self.key.own(proposed)
    else {
      return Verdict::Accept;
    };
    let verdict = untrack(|| {
      Self::verdict(
        || (self.truth)(),
        &proposed,
        self.disabled.as_ref().is_some_and(|disabled| disabled()),
        self.writing.get(),
        |asked| (self.ask)(asked),
      )
    });
    let shown = self.shown.take();

    *self.shown.borrow_mut() = Self::shown_after(shown, &proposed, applied, &verdict);

    if verdict == Verdict::Refuse {
      untrack(|| self.write(ops, node));
    }

    verdict
  }

  /// What a content shows once the truth `truth` is written over `shown`: `None` where it
  /// shows the truth already, and while the reader's change is `composing`, the write
  /// waiting for the report that ends it.
  fn written(truth: T, shown: Option<&T>, composing: bool) -> Option<T> {
    (!composing && shown != Some(&truth)).then_some(truth)
  }

  /// The verdict on a reader's `proposed` change of a content held whole, `truth` reading
  /// the truth as it stands, before the ask and after it, and `ask` asking the app.
  ///
  /// A change reported while the truth is `writing` is its echo and stands, asking
  /// nothing; a change the truth holds stands, asking nothing; any other asks the app,
  /// unless the content is `disabled`, and stands where the truth then holds it, refused
  /// otherwise.
  fn verdict(
    truth: impl Fn() -> T,
    proposed: &T,
    disabled: bool,
    writing: bool,
    ask: impl FnOnce(&T),
  ) -> Verdict {
    if writing || truth().holds(proposed) {
      return Verdict::Accept;
    }

    if !disabled {
      ask(proposed);
    }

    if truth().holds(proposed) {
      Verdict::Accept
    }
    else {
      Verdict::Refuse
    }
  }

  /// What a content held whole shows after a report of `proposed` answered `verdict`,
  /// `applied` saying whether its toolkit showed the change already, where it showed
  /// `shown` before: the proposal where the toolkit shows it or applies it, else `shown`.
  fn shown_after(shown: Option<T>, proposed: &T, applied: bool, verdict: &Verdict) -> Option<T> {
    if applied || *verdict == Verdict::Accept {
      Some(proposed.clone())
    }
    else {
      shown
    }
  }
}

/// What controls a node's content: a [`Controller`] of a content held whole, or a text
/// entry's [`EditController`].
#[diagnostic::on_unimplemented(
  message = "`{Self}` does not control a node's content",
  label = "a content is controlled by a `Controller`, or an entry's edit by an `EditController`"
)]
pub trait Controls: Sized + 'static {
  /// Answers every change with the truth, asking nothing, while `disabled` says so.
  #[must_use]
  fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self;

  /// Controls the content of `node` through the realization `ops` reaches, as the
  /// current owner.
  fn register_with(self, ops: ContentOps, node: NodeId);
}

impl<T: Controllable> Controls for Controller<T> {
  #[inline(always)]
  fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self {
    Self::derive_disabled(self, disabled)
  }

  #[inline(always)]
  fn register_with(self, ops: ContentOps, node: NodeId) {
    Self::register_with(self, ops, node);
  }
}

impl Controls for EditController {
  #[inline(always)]
  fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self {
    Self::derive_disabled(self, disabled)
  }

  #[inline(always)]
  fn register_with(self, ops: ContentOps, node: NodeId) {
    Self::register_with(self, ops, node);
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::{Cell, RefCell},
    rc::Rc,
  };

  use super::{Answer, Asked, Controllable, Controller, EditController, EditTruth};
  use crate::spine::{
    CheckState,
    ChoiceOption,
    Choices,
    Edit,
    Range,
    TextSelection,
    Unit,
    Verdict,
  };

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
      EditController::shown_after(&composed, true, &answered.verdict),
      Some(composed.clone()),
      "the entry shows the composed text"
    );

    let (answered, _) = answer(&held, Some(&composed), &composed, false, false);

    assert_eq!(answered.verdict, Verdict::Replace(edit("AB", 2)));
    assert_eq!(
      EditController::shown_after(&composed, true, &answered.verdict),
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
      EditController::shown_after(&typed, false, &Verdict::Replace(truth)),
      None,
      "what a toolkit applying the truth shows is known once written or reported"
    );
  }

  /// The verdict on `proposed` while the truth reads `truth`, with the asks collected
  /// rather than run.
  fn whole<T: Controllable>(
    truth: &T,
    proposed: &T,
    disabled: bool,
    writing: bool,
  ) -> (Verdict, Vec<T>) {
    let asks = RefCell::new(Vec::new());
    let verdict = Controller::<T>::verdict(
      || truth.clone(),
      proposed,
      disabled,
      writing,
      |asked| asks.borrow_mut().push(asked.clone()),
    );

    (verdict, asks.into_inner())
  }

  /// A change of a content held whole that the truth holds, or one reported while the
  /// truth is written, asks nothing and stands.
  #[test]
  fn a_whole_change_the_truth_holds_or_its_echo_asks_nothing() {
    assert_eq!(
      whole(&CheckState::On, &CheckState::On, false, false),
      (Verdict::Accept, Vec::new())
    );
    assert_eq!(
      whole(&CheckState::Off, &CheckState::On, false, true),
      (Verdict::Accept, Vec::new()),
      "the echo of a write"
    );
  }

  /// A change the truth does not hold asks the app, and stands where the truth then
  /// holds it, refused otherwise; a disabled control asks nothing and refuses it.
  #[test]
  fn a_whole_change_is_asked_for_and_refused_where_the_truth_keeps_it_out() {
    assert_eq!(
      whole(&CheckState::Off, &CheckState::On, false, false),
      (Verdict::Refuse, vec![CheckState::On]),
      "a handler that keeps the truth refuses it"
    );

    let truth = Cell::new(CheckState::Off);
    let verdict = Controller::verdict(
      || truth.get(),
      &CheckState::On,
      false,
      false,
      |asked| truth.set(*asked),
    );

    assert_eq!(verdict, Verdict::Accept, "a handler that takes it");
    assert_eq!(
      whole(&CheckState::Off, &CheckState::On, true, false),
      (Verdict::Refuse, Vec::new()),
      "a disabled control"
    );
  }

  /// A range is held by its value, the reader's, and a choice by the key chosen, the
  /// bounds and the options being the app's.
  #[test]
  fn a_range_holds_its_value_and_a_choice_its_key() {
    let truth = Range::new(5., 0., 10., 1.);

    assert!(truth.holds(&Range::new(5., 0., 100., 0.)));
    assert!(!truth.holds(&Range::new(6., 0., 10., 1.)));

    let options: Rc<[ChoiceOption]> = Rc::from([ChoiceOption::new("a", "A")]);
    let chosen = Choices::new(Rc::clone(&options), Some(Rc::from("a")));

    assert!(chosen.holds(&Choices::new(Rc::from([]), Some(Rc::from("a")))));
    assert!(!chosen.holds(&Choices::new(options, None)));
  }

  /// The truth is written only where the content does not show it already, and waits
  /// while the reader's change composes; a report leaves the proposal shown where the
  /// toolkit shows or applies it.
  #[test]
  fn a_whole_write_of_what_is_shown_is_skipped() {
    assert_eq!(
      Controller::written(CheckState::On, Some(&CheckState::On), false),
      None
    );
    assert_eq!(
      Controller::written(CheckState::On, Some(&CheckState::Off), false),
      Some(CheckState::On)
    );
    assert_eq!(
      Controller::written(CheckState::On, None, false),
      Some(CheckState::On)
    );
    assert_eq!(
      Controller::written(CheckState::On, None, true),
      None,
      "the write waits for the reader's change to be reported"
    );
    assert_eq!(
      Controller::shown_after(
        Some(CheckState::Off),
        &CheckState::On,
        true,
        &Verdict::Refuse
      ),
      Some(CheckState::On),
      "an applied change shows until the repair"
    );
    assert_eq!(
      Controller::shown_after(
        Some(CheckState::Off),
        &CheckState::On,
        false,
        &Verdict::Refuse
      ),
      Some(CheckState::Off),
      "a refused change never applied leaves what was shown"
    );
  }
}
