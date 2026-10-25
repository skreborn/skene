//! A drawn text entry's editing state, which the scene keeps per node for a realization
//! that draws its entries: the editing core over the realization's engine, masked where the
//! entry is secure, what it changed, the selection last reported, the operations waiting
//! for the one under way, and the arbiter reporting each edit at the entry's node.

use std::{
  cell::{Cell, RefCell},
  fmt,
  rc::Rc,
};

use crate::{
  Content,
  ContentValue,
  Edit,
  NodeId,
  Realization,
  Report,
  TextSelection,
  Verdict,
  report,
  text::{
    MaskState,
    Masked,
    TextEngine,
    editing::{Arbiter, Editor, Notice},
  },
};

/// An operation on an entry's editor that waits for the operation under way on it to end.
pub type Deferred<E> = Box<dyn FnOnce(&mut Editor<Masked<E>>)>;

/// How a reader's change is reported at a node: [`report`] as one realization reports it.
type ReportAt = fn(NodeId, Report<'_>) -> Verdict;

/// What an entry's editor changed since its changes were last taken.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub struct Changed {
  /// Whether the value changed.
  pub text: bool,
  /// Whether the selection moved.
  pub selection: bool,
}

/// The arbiter of an entry's edits: each is reported at the entry's node, its text and its
/// selection as one, and the verdict its controller answers with is what the entry shows.
struct Reporter {
  node: NodeId,
  report: ReportAt,
  reported: Rc<Cell<TextSelection>>,
}

impl Arbiter for Reporter {
  fn propose(&mut self, proposed: &Edit, applied: bool, _composing: bool) -> Verdict {
    if let Some(selection) = proposed.selection {
      self.reported.set(selection);
    }

    (self.report)(
      self.node,
      Report::Change {
        content: Content::Edit,
        proposed: ContentValue::Edit(proposed),
        applied,
      },
    )
  }

  fn settle(&mut self) -> Option<Edit> {
    None
  }
}

/// A drawn text entry's editing state: the editing core over the engine `E`, masked as its
/// [`MaskState`] says.
///
/// The scene of a realization that draws its entries keeps one per entry and hands it out
/// shared, so an operation on the core runs with the scene released.
///
/// Each edit the core proposes is reported at the entry's node as the realization reports
/// a change ([`report`]), its text and its selection as one, and the verdict answers it;
/// each change it makes is noted until [`take_changed`](Self::take_changed) takes it.
#[must_use]
pub struct Entry<E: TextEngine> {
  node: NodeId,
  report: ReportAt,
  editor: RefCell<Editor<Masked<E>>>,
  mask: Rc<MaskState>,
  changed: Rc<Cell<Changed>>,
  reported: Rc<Cell<TextSelection>>,
  deferred: RefCell<Vec<Deferred<E>>>,
}

impl<E: TextEngine> fmt::Debug for Entry<E> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Entry")
      .field("node", &self.node)
      .finish_non_exhaustive()
  }
}

impl<E: TextEngine + 'static> Entry<E> {
  /// The editing state of the entry `node` over `editor`, empty and masked as its engine's
  /// [`MaskState`] says, its edits reported at `node` as the realization `P` reports a
  /// change.
  pub fn new<P: Realization>(node: NodeId, mut editor: Editor<Masked<E>>) -> Self {
    let report: ReportAt = report::<P>;
    let mask = Rc::clone(editor.engine().substitute());
    let changed = Rc::new(Cell::new(Changed::default()));
    let reported = Rc::new(Cell::new(TextSelection::collapsed(0)));
    let notices = Rc::clone(&changed);

    editor.set_arbiter(Reporter {
      node,
      report,
      reported: Rc::clone(&reported),
    });
    editor.set_notifier(move |notice: Notice| {
      let held = notices.get();

      notices.set(Changed {
        text: held.text || notice.text,
        selection: held.selection || notice.selection || notice.text,
      });
    });

    Self {
      node,
      report,
      editor: RefCell::new(editor),
      mask,
      changed,
      reported,
      deferred: RefCell::new(Vec::new()),
    }
  }
}

impl<E: TextEngine> Entry<E> {
  /// The entry's node.
  #[inline(always)]
  pub const fn node(&self) -> NodeId {
    self.node
  }

  /// The editing core; borrowed only by an operation on it, and by readers between them.
  #[inline(always)]
  pub const fn editor(&self) -> &RefCell<Editor<Masked<E>>> {
    &self.editor
  }

  /// Whether the editor's engine masks the value, shared with the engine.
  #[inline(always)]
  #[must_use]
  pub const fn mask(&self) -> &Rc<MaskState> {
    &self.mask
  }

  /// What the editor changed since the last take, taken.
  pub fn take_changed(&self) -> Changed {
    self.changed.take()
  }

  /// The selection last reported, or written from outside, shared with what writes it.
  #[must_use]
  pub fn reported(&self) -> Rc<Cell<TextSelection>> {
    Rc::clone(&self.reported)
  }

  /// Runs `operation` on the editor once the operation holding it ends, which runs it
  /// through [`take_deferred`](Self::take_deferred).
  pub fn defer(&self, operation: impl FnOnce(&mut Editor<Masked<E>>) + 'static) {
    self.deferred.borrow_mut().push(Box::new(operation));
  }

  /// Whether an operation waits for the one under way on the editor to end.
  #[must_use]
  pub fn has_deferred(&self) -> bool {
    !self.deferred.borrow().is_empty()
  }

  /// The operations waiting for the one under way on the editor to end, taken, in the
  /// order they were deferred.
  #[must_use]
  pub fn take_deferred(&self) -> Vec<Deferred<E>> {
    self.deferred.take()
  }

  /// The value with the selection, when the selection differs from the one last
  /// reported or written from outside, which it becomes; `None` too while an operation
  /// holds the editor.
  #[must_use]
  pub fn selection_report(&self) -> Option<Edit> {
    let editor = self.editor.try_borrow().ok()?;
    let selection = editor.selection();

    (selection != self.reported.replace(selection))
      .then(|| Edit::new(editor.text()).with_selection(selection))
  }

  /// Reports the selection where it moved since it was last reported or written from
  /// outside, at the entry's node as a change of what it shows that its toolkit applied:
  /// the truth to show where the verdict answers with one.
  #[must_use]
  pub fn report_selection(&self) -> Option<Edit> {
    let shown = self.selection_report()?;
    let verdict = (self.report)(
      self.node,
      Report::Change {
        content: Content::Edit,
        proposed: ContentValue::Edit(&shown),
        applied: true,
      },
    );

    match verdict {
      Verdict::Replace(truth) => Some(truth),
      Verdict::Accept | Verdict::Refuse => None,
    }
  }

  /// The value with the selection, `None` while an operation holds the editor.
  #[must_use]
  pub fn shown(&self) -> Option<Edit> {
    let editor = self.editor.try_borrow().ok()?;

    Some(Edit::new(editor.text()).with_selection(editor.selection()))
  }

  /// The value as it shows, a mark a cluster where the entry is secure; `None` while an
  /// operation holds the editor.
  #[must_use]
  pub fn value(&self) -> Option<String> {
    Some(self.mask.shown(self.editor.try_borrow().ok()?.text()))
  }

  /// Whether the entry is secure: its value masked, and kept from the clipboard.
  #[must_use]
  pub fn is_secure(&self) -> bool {
    self.mask.is_masked()
  }

  /// The selected text, `None` while the selection is collapsed, the entry is secure or an
  /// operation holds the editor.
  #[must_use]
  pub fn selected_text(&self) -> Option<String> {
    let editor = self.editor.try_borrow().ok()?;
    let selected = editor.selected();

    (!selected.is_empty() && !editor.is_secure()).then(|| editor.text()[selected].to_owned())
  }

  /// Whether a range of the value is selected.
  #[must_use]
  pub fn has_selection(&self) -> bool {
    self
      .editor
      .try_borrow()
      .is_ok_and(|editor| !editor.selected().is_empty())
  }
}
