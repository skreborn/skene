//! The editor: an entry's text, selection, composition and history over its engine's
//! geometry, with the input-client surface every platform's adapter speaks.

use std::{cell::RefCell, fmt, ops::Range, rc::Rc};

use crate::{
  Edit,
  Rect,
  TextDecoration,
  TextDecorationStyle,
  TextSelection,
  Unit,
  Verdict,
  text::{
    Affinity,
    AttributedText,
    Direction,
    Laid,
    Paragraph,
    ParagraphStyle,
    Position,
    Round,
    Segments,
    TextBoundary,
    TextEngine,
    TextIndex,
    TextRun,
    TextStyle,
    VisualMove,
    editing::{
      Arbiter,
      Command,
      Movement,
      Outcome,
      history::{EditKind, History, Step},
    },
  },
};

/// How an input method shows one clause of its composition.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
#[must_use]
pub enum ClauseStyle {
  /// Not yet converted.
  #[default]
  Raw,
  /// Converted.
  Converted,
  /// The clause being converted now.
  Target,
}

/// One clause of a composition: a byte range of the marked text and how it is shown.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub struct Clause {
  /// The bytes of the marked text it covers.
  pub range: Range<usize>,
  /// How it is shown.
  pub style: ClauseStyle,
}

/// The text an input method is composing, marked in the entry's text.
#[derive(Clone, PartialEq, Eq, Debug)]
#[must_use]
pub struct Composition {
  /// The marked bytes of the text.
  pub range: Range<usize>,
  /// Its clauses, in byte ranges of the text.
  pub clauses: Vec<Clause>,
  before: Step,
}

/// An end of the selection, as a touch selection handle moves it.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub enum SelectionEnd {
  /// Its start, in the text's order.
  Start,
  /// Its end, in the text's order.
  End,
}

/// Who made a change the notifier hears of.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
pub enum Origin {
  /// The platform's input protocol, through the input-client surface.
  Input,
  /// The reader, through a command or the pointer.
  Reader,
  /// The app: a write of the truth, a repair, or a composition settled.
  App,
}

/// What changed in an entry, for the platform's adapter (to tell its input method) and the
/// realization (to repaint and report).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[must_use]
#[expect(
  clippy::struct_excessive_bools,
  reason = "one flag per kind of change the notifier hears of"
)]
pub struct Notice {
  /// Who changed it.
  pub origin: Origin,
  /// Whether the text changed.
  pub text: bool,
  /// Whether the selection moved.
  pub selection: bool,
  /// Whether the composition changed or ended.
  pub marked: bool,
  /// Whether the composition was ended from outside the input method, which must start
  /// afresh.
  pub restart: bool,
}

/// How a press followed by a drag selects.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Granularity {
  Cluster,
  Word(Range<usize>),
  Line(Range<usize>),
}

/// The paragraph and its layout, made when a query first needs them after a change.
struct Geometry<P: Paragraph> {
  paragraph: P,
  laid: Option<P::Laid>,
}

/// A text entry's editing core, over the geometry its engine lays the text out in.
///
/// It keeps the text, its selection (an anchor and a focus, the focus the caret, before the
/// anchor when made backward), the composition an input method marks in it, and its undo
/// history.
///
/// Every change is proposed to the [`Arbiter`] where one is set, as the controlled contract
/// says, and announced to the notifier. A query of geometry after a change lays the text out
/// first, so every answer is synchronous.
///
/// Positions are bytes of the text inside; at its surface an adapter names them in its
/// protocol's [`Unit`] (`replace`, `select_in`) or in bytes where its protocol counts in
/// those (`replace_bytes`, `select_bytes`), and the public selection names grapheme clusters
/// ([`Editor::selection`]).
#[must_use]
pub struct Editor<E: TextEngine> {
  engine: E,
  style: TextStyle,
  paragraph_style: ParagraphStyle,
  width: Option<f32>,
  multiline: bool,
  read_only: bool,
  secure: bool,
  text: String,
  index: TextIndex,
  anchor: Position,
  focus: Position,
  marked: Option<Composition>,
  history: History,
  goal: Option<f32>,
  granularity: Granularity,
  held: Option<Edit>,
  geometry: RefCell<Option<Geometry<E::Paragraph>>>,
  arbiter: Option<Box<dyn Arbiter>>,
  notifier: Option<Box<dyn FnMut(Notice)>>,
}

impl<E: TextEngine> Editor<E> {
  /// An empty entry of one line laid out by `engine` in `style`.
  pub fn new(engine: E, style: TextStyle, paragraph_style: ParagraphStyle) -> Self {
    Self {
      engine,
      style,
      paragraph_style,
      width: None,
      multiline: false,
      read_only: false,
      secure: false,
      text: String::new(),
      index: TextIndex::new(""),
      anchor: Position::default(),
      focus: Position::default(),
      marked: None,
      history: History::default(),
      goal: None,
      granularity: Granularity::Cluster,
      held: None,
      geometry: RefCell::new(None),
      arbiter: None,
      notifier: None,
    }
  }

  /// Makes the entry one of several lines (an editor) where `multiline`: `Enter` breaks
  /// lines, and pasted line breaks stay.
  pub const fn set_multiline(&mut self, multiline: bool) {
    self.multiline = multiline;
  }

  /// Makes the text secure where `secure`: it offers neither copy nor cut, it keeps no undo
  /// history (what it kept is dropped), and a word is the whole text, so a double press
  /// selects it all and a drag after one extends to it all.
  pub fn set_secure(&mut self, secure: bool) {
    if secure != self.secure {
      self.history.set_enabled(!secure);
    }

    self.secure = secure;
  }

  /// Makes the text read only where `read_only`: the selection still moves and copies.
  pub const fn set_read_only(&mut self, read_only: bool) {
    self.read_only = read_only;
  }

  /// Lays the text out within `width`, unbounded (one line per hard line) where `None`.
  pub fn set_width(&mut self, width: Option<f32>) {
    if width != self.width {
      self.width = width;

      if let Some(geometry) = self.geometry.get_mut() {
        geometry.laid = None;
      }
    }
  }

  /// Sets the text's style.
  pub fn set_style(&mut self, style: TextStyle) {
    if style != self.style {
      let paint_only = style.shapes_like(&self.style);
      let paint = style.paint;

      self.style = style;

      match self.geometry.get_mut() {
        Some(geometry) if paint_only && self.marked.is_none() => {
          if !geometry.paragraph.repaint(0..self.text.len(), &paint) {
            self.invalidate();
          }
        }
        _ => self.invalidate(),
      }
    }
  }

  /// The text's style.
  #[inline(always)]
  pub const fn style(&self) -> &TextStyle {
    &self.style
  }

  /// Sets the paragraph's style.
  pub fn set_paragraph_style(&mut self, style: ParagraphStyle) {
    if style != self.paragraph_style {
      self.paragraph_style = style;
      self.invalidate();
    }
  }

  /// Lays the text out again at the next query: what its engine shows changed (a secure
  /// entry's revealed cluster) though the text did not.
  pub fn relayout(&mut self) {
    self.invalidate();
  }

  /// Sets what decides the reader's changes.
  pub fn set_arbiter(&mut self, arbiter: impl Arbiter + 'static) {
    self.arbiter = Some(Box::new(arbiter));
  }

  /// Sets what hears of every change.
  pub fn set_notifier(&mut self, notifier: impl FnMut(Notice) + 'static) {
    self.notifier = Some(Box::new(notifier));
  }

  /// The text, the composition's marked text included.
  #[inline(always)]
  #[must_use]
  pub fn text(&self) -> &str {
    &self.text
  }

  /// The text's index.
  #[inline(always)]
  pub const fn index(&self) -> &TextIndex {
    &self.index
  }

  /// Whether the text is secure ([`set_secure`](Self::set_secure)).
  #[inline(always)]
  #[must_use]
  pub const fn is_secure(&self) -> bool {
    self.secure
  }

  /// The selection's fixed end.
  #[inline(always)]
  pub const fn anchor(&self) -> Position {
    self.anchor
  }

  /// The selection's moving end, the caret.
  #[inline(always)]
  pub const fn focus(&self) -> Position {
    self.focus
  }

  /// The selected bytes, in the text's order.
  #[must_use]
  pub fn selected(&self) -> Range<usize> {
    self.anchor.index.min(self.focus.index)..self.anchor.index.max(self.focus.index)
  }

  /// The selection in grapheme clusters: the anchor and the caret.
  pub fn selection(&self) -> TextSelection {
    TextSelection {
      start: self.index.grapheme_at_byte(self.anchor.index, Round::Down),
      end: self.index.grapheme_at_byte(self.focus.index, Round::Down),
    }
  }

  /// The composition, while an input method composes.
  #[inline(always)]
  pub const fn composition(&self) -> Option<&Composition> {
    self.marked.as_ref()
  }

  /// Whether an input method composes.
  #[inline(always)]
  #[must_use]
  pub const fn is_composing(&self) -> bool {
    self.marked.is_some()
  }

  /// The text laid out at the entry's width, laid out now where a change made it stale.
  pub fn laid(&self) -> <E::Paragraph as Paragraph>::Laid {
    let mut geometry = self.geometry.borrow_mut();
    let geometry = geometry.get_or_insert_with(|| Geometry {
      paragraph: self
        .engine
        .paragraph(&self.attributed(), &self.paragraph_style),
      laid: None,
    });

    geometry
      .laid
      .get_or_insert_with(|| geometry.paragraph.layout(self.width))
      .clone()
  }

  /// The text with its composition's clauses underlined.
  fn attributed(&self) -> AttributedText {
    let Some(marked) = &self.marked
    else {
      return AttributedText::plain(self.text.as_str(), self.style.clone());
    };
    let underline = |style: ClauseStyle| {
      self
        .style
        .clone()
        .with_paint(self.style.paint.with_decoration(
          TextDecoration::Underline,
          match style {
            ClauseStyle::Target => TextDecorationStyle::Double,
            ClauseStyle::Raw => TextDecorationStyle::Dotted,
            ClauseStyle::Converted => TextDecorationStyle::Solid,
          },
        ))
    };
    let runs = if marked.clauses.is_empty() {
      vec![TextRun {
        range: marked.range.clone(),
        style: underline(ClauseStyle::Raw),
      }]
    }
    else {
      marked
        .clauses
        .iter()
        .map(|clause| TextRun {
          range: clause.range.clone(),
          style: underline(clause.style),
        })
        .collect()
    };

    AttributedText::new(self.text.as_str(), self.style.clone(), runs)
  }

  fn invalidate(&mut self) {
    *self.geometry.get_mut() = None;
  }

  fn notify(&mut self, notice: Notice) {
    if let Some(notifier) = &mut self.notifier {
      notifier(notice);
    }
  }

  fn step_now(&self) -> Step {
    Step {
      text: self.text.clone(),
      anchor: self.anchor,
      focus: self.focus,
    }
  }

  /// The edit the app hears: the text and the selection in grapheme clusters.
  fn edit_of(text: &str, index: &TextIndex, anchor: usize, focus: usize) -> Edit {
    Edit {
      text: Rc::from(text),
      selection: Some(TextSelection {
        start: index.grapheme_at_byte(anchor, Round::Down),
        end: index.grapheme_at_byte(focus, Round::Down),
      }),
    }
  }

  /// Shows `text` with the selection `anchor`..`focus` without asking: the index rebuilt, the
  /// layout made stale.
  fn show(&mut self, text: String, anchor: Position, focus: Position) {
    if text != self.text {
      self.text = text;
      self.index = TextIndex::new(&self.text);
      self.invalidate();
    }

    let len = self.text.len();

    self.anchor = Position::with(anchor.index.min(len), anchor.affinity);
    self.focus = Position::with(focus.index.min(len), focus.affinity);
    self.goal = None;
  }

  /// The truth `edit` as a text and a selection: its own, or the proposed one mapped
  /// through the smallest edit from `proposed` to it.
  fn resolve(edit: &Edit, proposed: &str, anchor: usize, focus: usize) -> (String, usize, usize) {
    let text = edit.text.to_string();
    let index = TextIndex::new(&text);

    if let Some(selection) = edit.selection {
      return (
        text,
        index.byte_of_grapheme(selection.start),
        index.byte_of_grapheme(selection.end),
      );
    }

    let (replaced, inserted) = edit.splice_bytes(proposed);
    let old = TextIndex::new(proposed);
    let map = |at: usize| old.map_through(&index, replaced.clone(), inserted.len(), at);

    let (anchor, focus) = (map(anchor), map(focus));

    (text, anchor, focus)
  }

  /// Proposes `text` with the selection `anchor`..`focus` as the reader's change of `kind`,
  /// then shows what the arbiter decides, as this entry takes it: `false` where nothing
  /// changed.
  fn propose(
    &mut self,
    text: String,
    anchor: Position,
    focus: Position,
    kind: EditKind,
    origin: Origin,
  ) -> bool {
    if text == self.text && anchor == self.anchor && focus == self.focus {
      return false;
    }

    let text_changed = text != self.text;

    if self.read_only && text_changed {
      return false;
    }

    let before = self.step_now();
    let composing = self.marked.is_some();
    let verdict = match &mut self.arbiter {
      Some(arbiter) if text_changed => {
        let index = TextIndex::new(&text);
        let edit = Self::edit_of(&text, &index, anchor.index, focus.index);

        arbiter.propose(&edit, composing, composing)
      }
      _ => Verdict::Accept,
    };
    let mut restart = false;
    let (text, anchor, focus) = match verdict {
      Verdict::Replace(truth) => {
        let truth = self.taken(&truth);
        let (text, anchor, focus) = Self::resolve(&truth, &text, anchor.index, focus.index);

        if composing {
          self.marked = None;
          restart = true;
        }

        (text, Position::new(anchor), Position::new(focus))
      }
      Verdict::Accept => (text, anchor, focus),
      Verdict::Refuse => (self.text.clone(), self.anchor, self.focus),
    };
    let text_changed = text != self.text;

    if !text_changed && anchor == self.anchor && focus == self.focus && !restart {
      return false;
    }

    if text_changed && self.marked.is_none() && !composing {
      self.history.record(before, kind);
    }
    else if !text_changed {
      self.history.break_run();
    }

    self.show(text, anchor, focus);
    self.notify(Notice {
      origin,
      text: text_changed,
      selection: true,
      marked: restart,
      restart,
    });

    true
  }

  /// Moves the selection without changing the text.
  fn select_positions(&mut self, anchor: Position, focus: Position, origin: Origin) -> bool {
    let len = self.text.len();
    let anchor = Position::with(anchor.index.min(len), anchor.affinity);
    let focus = Position::with(focus.index.min(len), focus.affinity);

    if anchor == self.anchor && focus == self.focus {
      return false;
    }

    self.anchor = anchor;
    self.focus = focus;
    self.history.break_run();
    self.notify(Notice {
      origin,
      text: false,
      selection: true,
      marked: false,
      restart: false,
    });

    true
  }

  /// Replaces `range` with `text`, the caret after it: after the cluster it ends in, where
  /// it joins what follows into one (a base typed before a combining mark).
  fn replace_range(
    &mut self,
    range: Range<usize>,
    text: &str,
    kind: EditKind,
    origin: Origin,
  ) -> bool {
    let mut next = String::with_capacity(self.text.len() - range.len() + text.len());

    next.push_str(&self.text[..range.start]);
    next.push_str(text);
    next.push_str(&self.text[range.end..]);

    let caret = TextIndex::new(&next).snap_to_grapheme(range.start + text.len(), Round::Up);
    let caret = Position::new(caret);

    self.propose(next, caret, caret, kind, origin)
  }

  /// `text` as this entry takes it: line breaks kept as line feeds in an entry of several
  /// lines, dropped in one.
  #[must_use]
  pub fn sanitized(&self, text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");

    if self.multiline {
      normalized
    }
    else {
      normalized.replace('\n', "")
    }
  }

  /// `edit` as this entry takes it: its text [`sanitized`](Self::sanitized), its selection
  /// as it is.
  fn taken(&self, edit: &Edit) -> Edit {
    Edit {
      text: Rc::from(self.sanitized(&edit.text)),
      selection: edit.selection,
    }
  }

  /// Writes the app's truth: `edit`'s text as this entry takes it
  /// ([`sanitized`](Self::sanitized)), and its selection where it gives one (else the
  /// selection mapped through the change). Nothing happens where it is shown already, a
  /// write during a composition waits for its end, and a write is an undo step of its own
  /// but for the first value of an entry with no history. `false` where nothing changed.
  pub fn write(&mut self, edit: &Edit) -> bool {
    if self.marked.is_some() {
      self.held = Some(edit.clone());

      return false;
    }

    let edit = &self.taken(edit);
    let shown = *edit.text == *self.text
      && edit
        .selection
        .is_none_or(|selection| selection == self.selection());

    if shown {
      return false;
    }

    let (text, anchor, focus) =
      Self::resolve(edit, &self.text, self.anchor.index, self.focus.index);
    let text_changed = text != self.text;

    if text_changed && !(self.text.is_empty() && self.history.is_empty()) {
      self.history.record(self.step_now(), EditKind::Other);
    }

    self.show(text, Position::new(anchor), Position::new(focus));
    self.notify(Notice {
      origin: Origin::App,
      text: text_changed,
      selection: true,
      marked: false,
      restart: text_changed,
    });

    true
  }

  /// Places the selection from outside, each end clamped into the text: `false` where it is
  /// the selection already.
  pub fn set_selection(&mut self, selection: TextSelection) -> bool {
    let anchor = Position::new(self.index.byte_of_grapheme(selection.start));
    let focus = Position::new(self.index.byte_of_grapheme(selection.end));

    self.select_positions(anchor, focus, Origin::App)
  }

  /// Applies `command`.
  pub fn perform(&mut self, command: Command) -> Outcome {
    self.perform_from(command, Origin::Reader)
  }

  fn perform_from(&mut self, command: Command, origin: Origin) -> Outcome {
    let edits = matches!(
      command,
      Command::Delete(..)
        | Command::Insert(_)
        | Command::InsertNewline
        | Command::Transpose
        | Command::Cut
        | Command::Paste(_)
        | Command::Undo
        | Command::Redo
    );

    if edits {
      self.unmark();
    }

    let selected = self.selected();
    let goal = self.goal.take();

    match command {
      Command::Move { movement, extend } => {
        let (anchor, focus) = self.moved(movement, extend, goal);
        let handled = self.select_positions(anchor, focus, origin);

        if let Movement::Lines(_) = movement {
          self.goal = self.goal.or(goal);
        }

        Outcome::handled(handled)
      }
      Command::Delete(..)
      | Command::Insert(_)
      | Command::InsertNewline
      | Command::Transpose
      | Command::Cut
      | Command::Paste(_)
        if self.read_only =>
      {
        Outcome::IGNORED
      }
      Command::Delete(unit, direction) => self.delete(unit, direction, origin),
      Command::Insert(text) => {
        let text = self.sanitized(&text);
        let kind = self.typed_over(&selected, &text);

        Outcome::handled(self.replace_range(selected, &text, kind, origin))
      }
      Command::InsertNewline if self.multiline => {
        Outcome::handled(self.replace_range(selected, "\n", EditKind::Other, origin))
      }
      Command::InsertNewline => Outcome::IGNORED,
      Command::Transpose => Outcome::handled(self.transpose(origin)),
      Command::SelectAll => Outcome::handled(self.select_positions(
        Position::new(0),
        Position::new(self.text.len()),
        origin,
      )),
      Command::Copy => {
        if selected.is_empty() || self.secure {
          return Outcome::IGNORED;
        }

        Outcome {
          handled: true,
          clipboard: Some(self.text[selected].to_owned()),
        }
      }
      Command::Cut => {
        if selected.is_empty() || self.secure {
          return Outcome::IGNORED;
        }

        let clipboard = self.text[selected.clone()].to_owned();

        self.replace_range(selected, "", EditKind::Other, origin);

        Outcome {
          handled: true,
          clipboard: Some(clipboard),
        }
      }
      Command::Paste(text) => {
        let text = self.sanitized(&text);

        Outcome::handled(self.replace_range(selected, &text, EditKind::Other, origin))
      }
      Command::Undo => Outcome::handled(!self.read_only && self.step(true, origin)),
      Command::Redo => Outcome::handled(!self.read_only && self.step(false, origin)),
    }
  }

  /// The kind of `text` replacing `range`: one cluster is typed, and typed over a selection
  /// it starts an undo step the clusters typed after it join.
  fn typed_over(&mut self, range: &Range<usize>, text: &str) -> EditKind {
    if TextIndex::new(text).grapheme_count() != 1 {
      return EditKind::Other;
    }

    if !range.is_empty() {
      self.history.break_run();
    }

    EditKind::Typed
  }

  /// Where a Backspace at the caret `at` starts removing: the last code point of the cluster
  /// before it, or the whole cluster where it is an emoji sequence (a modifier, a joiner, a
  /// variation selector or a tag inside it), a keycap or a flag.
  fn backspace_start(&self, at: usize) -> usize {
    let start = self.index.previous_grapheme(at);
    let cluster = &self.text[start..at];
    let whole = cluster.chars().any(|character| {
      matches!(
        character,
        '\u{200D}'
          | '\u{20E3}'
          | '\u{FE00}'..='\u{FE0F}'
          | '\u{1F1E6}'..='\u{1F1FF}'
          | '\u{1F3FB}'..='\u{1F3FF}'
          | '\u{E0020}'..='\u{E007F}'
          | '\u{E0100}'..='\u{E01EF}'
      )
    });

    if whole {
      return start;
    }

    cluster
      .char_indices()
      .next_back()
      .map_or(start, |(last, _)| start + last)
  }

  /// Removes the selection, or the text from the caret to the boundary of `unit` in
  /// `direction`: Backspace removes one code point of a cluster
  /// ([`backspace_start`](Self::backspace_start)).
  fn delete(&mut self, unit: TextBoundary, direction: Direction, origin: Origin) -> Outcome {
    let selected = self.selected();
    let (range, kind) = if selected.is_empty() {
      let at = self.focus.index;
      let to = self.boundary(at, unit, direction);
      let single = unit == TextBoundary::Grapheme;

      match direction {
        Direction::Backward if single => (self.backspace_start(at)..at, EditKind::RemovedBackward),
        Direction::Forward if single => (at..to, EditKind::RemovedForward),
        Direction::Backward => (to..at, EditKind::Other),
        Direction::Forward => (at..to, EditKind::Other),
      }
    }
    else {
      (selected, EditKind::Other)
    };

    if range.is_empty() {
      return Outcome::IGNORED;
    }

    Outcome::handled(self.replace_range(range, "", kind, origin))
  }

  /// Undoes (`back`) or redoes one step: the state it returns to is proposed as any change
  /// is, and a refused one stays where it was.
  ///
  /// A step that returns to the text as it is changes nothing and is dropped for the one
  /// before it.
  fn step(&mut self, back: bool, origin: Origin) -> bool {
    let (text, anchor, focus) = loop {
      let taken = if back {
        self.history.take_undo()
      }
      else {
        self.history.take_redo()
      };
      let Some(state) = taken
      else {
        return false;
      };
      let arbiter = self.arbiter.take();
      let mut verdict = Verdict::Accept;

      if let Some(mut arbiter) = arbiter {
        let index = TextIndex::new(&state.text);
        let edit = Self::edit_of(&state.text, &index, state.anchor.index, state.focus.index);

        verdict = arbiter.propose(&edit, false, false);
        self.arbiter = Some(arbiter);
      }

      let (text, anchor, focus) = match &verdict {
        Verdict::Replace(truth) => {
          let truth = self.taken(truth);
          let (text, anchor, focus) =
            Self::resolve(&truth, &state.text, state.anchor.index, state.focus.index);

          (text, Position::new(anchor), Position::new(focus))
        }
        Verdict::Accept => (state.text.clone(), state.anchor, state.focus),
        Verdict::Refuse => (self.text.clone(), self.anchor, self.focus),
      };

      if text != self.text {
        break (text, anchor, focus);
      }

      if let Verdict::Replace(_) | Verdict::Refuse = verdict {
        if back {
          self.history.push_undo(state);
        }
        else {
          self.history.push_redo(state);
        }

        return false;
      }
    };
    let current = self.step_now();

    if back {
      self.history.push_redo(current);
    }
    else {
      self.history.push_undo(current);
    }

    self.show(text, anchor, focus);
    self.notify(Notice {
      origin,
      text: true,
      selection: true,
      marked: false,
      restart: false,
    });

    true
  }

  fn transpose(&mut self, origin: Origin) -> bool {
    if !self.selected().is_empty() || self.text.is_empty() {
      return false;
    }

    let mut at = self.focus.index;

    if at == self.text.len() {
      at = self.index.previous_grapheme(at);
    }

    let before = self.index.previous_grapheme(at);
    let after = self.index.next_grapheme(at);

    if before == at || after == at {
      return false;
    }

    let swapped = format!("{}{}", &self.text[at..after], &self.text[before..at]);

    let mut next = self.text.clone();

    next.replace_range(before..after, &swapped);

    let caret = Position::new(after);

    self.propose(next, caret, caret, EditKind::Other, origin)
  }

  /// The boundary of `unit` from `at` in `direction`, as the engine reads the text.
  fn boundary(&self, at: usize, unit: TextBoundary, direction: Direction) -> usize {
    if unit == TextBoundary::Grapheme {
      return match direction {
        Direction::Forward => self.index.next_grapheme(at),
        Direction::Backward => self.index.previous_grapheme(at),
      };
    }

    self.laid();

    let geometry = self.geometry.borrow();

    geometry.as_ref().map_or(at, |geometry| {
      geometry.paragraph.boundary(at, unit, direction)
    })
  }

  /// The selection after `movement`: the anchor kept where `extend`, else collapsed.
  fn moved(&mut self, movement: Movement, extend: bool, goal: Option<f32>) -> (Position, Position) {
    let selected = self.selected();
    let laid = self.laid();
    let collapsing = !extend && !selected.is_empty();
    let focus = match movement {
      Movement::Visual(VisualMove::Left | VisualMove::Right) if collapsing => {
        let start = laid.caret(Position::new(selected.start)).primary.x;
        let end = laid.caret(Position::new(selected.end)).primary.x;
        let left = matches!(movement, Movement::Visual(VisualMove::Left));

        Position::new(if (start <= end) == left {
          selected.start
        }
        else {
          selected.end
        })
      }
      Movement::Visual(movement) => laid.move_visual(self.focus, movement),
      Movement::Logical(unit, direction) => {
        Position::new(self.boundary(self.focus.index, unit, direction))
      }
      Movement::Lines(lines) => {
        let from = if collapsing {
          Position::new(if lines < 0 {
            selected.start
          }
          else {
            selected.end
          })
        }
        else {
          self.focus
        };

        if self.multiline {
          let (position, kept) = laid.move_vertical(from, lines, goal);

          self.goal = Some(kept);

          position
        }
        else if lines < 0 {
          Position::new(0)
        }
        else {
          Position::new(self.text.len())
        }
      }
    };

    if extend {
      (self.anchor, focus)
    }
    else {
      (focus, focus)
    }
  }

  /// A press of the pointer at `x`, `y` (relative to the text's layout), the `clicks`th in a
  /// row: one places the caret (extending the selection where `extend`), two select the
  /// word, three the line in an entry of several lines and the whole text in one. A drag
  /// after it extends by the same unit.
  pub fn press(&mut self, x: f32, y: f32, clicks: u8, extend: bool) -> bool {
    let hit = self.laid().hit_test(x, y).position;
    let segments = Segments::new(&self.text);

    match (clicks.max(1) - 1) % 3 + 1 {
      1 => {
        self.granularity = Granularity::Cluster;

        let anchor = if extend { self.anchor } else { hit };

        self.select_positions(anchor, hit, Origin::Reader)
      }
      2 => {
        let word = self.word_around(&segments, hit.index);

        self.granularity = Granularity::Word(word.clone());
        self.select_positions(
          Position::new(word.start),
          Position::new(word.end),
          Origin::Reader,
        )
      }
      _ => {
        let line = if self.multiline {
          segments.paragraph_around(hit.index)
        }
        else {
          0..self.text.len()
        };

        self.granularity = Granularity::Line(line.clone());
        self.select_positions(
          Position::new(line.start),
          Position::new(line.end),
          Origin::Reader,
        )
      }
    }
  }

  /// The word around byte `index`: the whole text where it is secure.
  fn word_around(&self, segments: &Segments<'_>, index: usize) -> Range<usize> {
    if self.secure {
      0..self.text.len()
    }
    else {
      segments.word_around(index)
    }
  }

  /// A drag of the pointer to `x`, `y` after a press: extends the selection by the press's
  /// unit.
  pub fn drag(&mut self, x: f32, y: f32) -> bool {
    let hit = self.laid().hit_test(x, y).position;
    let segments = Segments::new(&self.text);
    let (pivot, reached) = match &self.granularity {
      Granularity::Cluster => return self.select_positions(self.anchor, hit, Origin::Reader),
      Granularity::Word(pivot) => (pivot.clone(), self.word_around(&segments, hit.index)),
      Granularity::Line(pivot) => (pivot.clone(), segments.paragraph_around(hit.index)),
    };
    let (anchor, focus) = if reached.start < pivot.start {
      (pivot.end, reached.start)
    }
    else {
      (pivot.start, reached.end.max(pivot.end))
    };

    self.select_positions(Position::new(anchor), Position::new(focus), Origin::Reader)
  }

  /// The selection's `end` moved to `x`, `y` (relative to the text's layout), as a touch
  /// selection handle moves it: kept at least a grapheme cluster short of the other end, so
  /// the two never cross, and becoming the caret, the other end the anchor.
  pub fn move_selection_end(&mut self, end: SelectionEnd, x: f32, y: f32) -> bool {
    let hit = self.laid().hit_test(x, y).position.index;
    let selected = self.selected();

    self.granularity = Granularity::Cluster;

    match end {
      SelectionEnd::Start => {
        let limit = self.index.previous_grapheme(selected.end);

        self.select_positions(
          Position::new(selected.end),
          Position::new(hit.min(limit)),
          Origin::Reader,
        )
      }
      SelectionEnd::End => {
        let limit = self.index.next_grapheme(selected.start);

        self.select_positions(
          Position::new(selected.start),
          Position::new(hit.max(limit)),
          Origin::Reader,
        )
      }
    }
  }

  /// The caret's rectangle, relative to the text's layout.
  pub fn caret_rect(&self) -> Rect<f32> {
    self.laid().caret(self.focus).primary.rect()
  }

  /// The rectangles the selection covers.
  #[must_use]
  pub fn selection_rects(&self) -> Vec<Rect<f32>> {
    let selected = self.selected();

    if selected.is_empty() {
      return Vec::new();
    }

    self.laid().range_rects(selected)
  }

  // The input-client surface: what a platform's adapter answers its protocol with, in the
  // unit its protocol counts in, or in bytes of the text where it counts in those.

  /// The text's length in `unit`.
  #[must_use]
  pub fn count_in(&self, unit: Unit) -> usize {
    self.index.count(unit)
  }

  /// The text of `range` in `unit`, widened to whole characters.
  #[must_use]
  pub fn text_in(&self, range: Range<usize>, unit: Unit) -> String {
    self.text[self.index.byte_range(unit, range)].to_owned()
  }

  /// The selection in `unit`: the anchor and the focus.
  #[must_use]
  pub fn selection_in(&self, unit: Unit) -> (usize, usize) {
    (
      self.index.offset(unit, self.anchor.index),
      self.index.offset(unit, self.focus.index),
    )
  }

  /// The marked range in `unit`, while an input method composes.
  #[must_use]
  pub fn marked_range_in(&self, unit: Unit) -> Option<Range<usize>> {
    self
      .marked
      .as_ref()
      .map(|marked| self.index.unit_range(unit, marked.range.clone()))
  }

  /// Selects `anchor`..`focus` in `unit`, each end taken to its character's start, and to its
  /// grapheme cluster's outside a composition.
  pub fn select_in(&mut self, anchor: usize, focus: usize, unit: Unit) -> bool {
    let anchor = self.index.byte(unit, anchor, Round::Down);
    let focus = self.index.byte(unit, focus, Round::Down);

    self.select_bytes(anchor, focus)
  }

  /// Selects bytes `anchor`..`focus`, each end taken to its character's start, and to its
  /// grapheme cluster's outside a composition.
  pub fn select_bytes(&mut self, anchor: usize, focus: usize) -> bool {
    let snap = |at: usize| {
      let byte = self.index.snap_to_char(at, Round::Down);

      if self.marked.is_some() {
        byte
      }
      else {
        self.index.snap_to_grapheme(byte, Round::Down)
      }
    };
    let (anchor, focus) = (snap(anchor), snap(focus));

    self.select_positions(Position::new(anchor), Position::new(focus), Origin::Input)
  }

  /// Replaces `range` in `unit` (the composition's marked text where one is marked, else the
  /// selection, where `None`) with `text`, ending any composition: an input method's commit,
  /// or its own edit of the text around the caret.
  pub fn replace(&mut self, range: Option<Range<usize>>, text: &str, unit: Unit) -> bool {
    let range = range.map(|range| self.index.byte_range(unit, range));

    self.replace_bytes(range, text)
  }

  /// Replaces the bytes `range` (the composition's marked text where one is marked, else the
  /// selection, where `None`), widened to whole characters, with `text`, ending any
  /// composition: an input method's commit, or its own edit of the text around the caret.
  pub fn replace_bytes(&mut self, range: Option<Range<usize>>, text: &str) -> bool {
    let text = self.sanitized(text);
    let marked = self.marked.clone();
    let around = range.is_some();
    let range = match (range, &marked) {
      (Some(range), _) => self.index.char_range(range),
      (None, Some(marked)) => marked.range.clone(),
      (None, None) => self.selected(),
    };
    let surrounding = around && !range.is_empty() && marked.is_none() && text.is_empty();
    let kind = if surrounding {
      EditKind::Surrounding
    }
    else if marked.is_some() || around && !range.is_empty() {
      EditKind::Other
    }
    else {
      self.typed_over(&range, &text)
    };

    match marked {
      Some(marked) => {
        self.replace_range(range, &text, kind, Origin::Input);
        self.marked = None;
        self.commit(marked);

        true
      }
      None => self.replace_range(range, &text, kind, Origin::Input),
    }
  }

  /// Marks `text` as an input method's composition in place of `range` in `unit` (the marked
  /// text where one is marked, else the selection, where `None`), with its `clauses` (byte
  /// ranges of `text`) and its caret or selection `selected` (in `unit` of `text`). An empty
  /// `text` ends the composition with nothing.
  pub fn set_marked(
    &mut self,
    text: &str,
    clauses: Vec<Clause>,
    selected: Range<usize>,
    range: Option<Range<usize>>,
    unit: Unit,
  ) -> bool {
    let range = range.map(|range| self.index.byte_range(unit, range));
    let selected = TextIndex::new(text).byte_range(unit, selected);

    self.set_marked_bytes(text, clauses, selected, range)
  }

  /// Marks `text` as an input method's composition in place of the bytes `range` (the marked
  /// text where one is marked, else the selection, where `None`), widened to whole
  /// characters, with its `clauses` and its caret or selection `selected`, both byte ranges
  /// of `text`. An empty `text` ends the composition with nothing.
  pub fn set_marked_bytes(
    &mut self,
    text: &str,
    clauses: Vec<Clause>,
    selected: Range<usize>,
    range: Option<Range<usize>>,
  ) -> bool {
    let range = match (range, &self.marked) {
      (Some(range), _) => self.index.char_range(range),
      (None, Some(marked)) => marked.range.clone(),
      (None, None) => self.selected(),
    };
    let before = match &self.marked {
      Some(marked) => marked.before.clone(),
      None => self.step_now(),
    };
    let inner_selected = TextIndex::new(text).char_range(selected);
    let mut next = String::with_capacity(self.text.len() - range.len() + text.len());

    next.push_str(&self.text[..range.start]);
    next.push_str(text);
    next.push_str(&self.text[range.end..]);

    let composition = (!text.is_empty()).then(|| Composition {
      range: range.start..range.start + text.len(),
      clauses: clauses
        .into_iter()
        .map(|clause| Clause {
          range: range.start + clause.range.start..range.start + clause.range.end,
          style: clause.style,
        })
        .collect(),
      before: before.clone(),
    });
    let ended = composition.is_none() && self.marked.is_some();

    self.marked = composition.or_else(|| self.marked.clone());

    let anchor = Position::new(range.start + inner_selected.start);
    let focus = Position::new(range.start + inner_selected.end);
    let changed = self.propose(next, anchor, focus, EditKind::Other, Origin::Input);

    if ended {
      if let Some(marked) = self.marked.take() {
        self.commit(marked);
      }
    }
    else if self.marked.is_some() {
      self.invalidate();
      self.notify(Notice {
        origin: Origin::Input,
        text: false,
        selection: false,
        marked: true,
        restart: false,
      });
    }

    changed
  }

  /// Ends the composition, its marked text kept as typed.
  pub fn unmark(&mut self) -> bool {
    match self.marked.take() {
      Some(marked) => {
        self.commit(marked);

        true
      }
      None => false,
    }
  }

  /// A composition ended: one undo step from the state before it, and the truth settled
  /// (what a change during it was held for, and a write held for its end).
  fn commit(&mut self, marked: Composition) {
    self.invalidate();

    if self.text != marked.before.text {
      self.history.record(marked.before, EditKind::Other);
    }

    let settled = self.arbiter.as_mut().and_then(|arbiter| arbiter.settle());

    self.notify(Notice {
      origin: Origin::Input,
      text: false,
      selection: false,
      marked: true,
      restart: false,
    });

    let held = self.held.take();

    if let Some(truth) = settled.or(held) {
      self.repair(&truth);
    }
  }

  /// Shows the app's truth after a change it was held for, as no undo step.
  fn repair(&mut self, truth: &Edit) {
    let (text, anchor, focus) =
      Self::resolve(truth, &self.text, self.anchor.index, self.focus.index);

    if text == self.text && anchor == self.anchor.index && focus == self.focus.index {
      return;
    }

    let text_changed = text != self.text;

    self.show(text, Position::new(anchor), Position::new(focus));
    self.notify(Notice {
      origin: Origin::App,
      text: text_changed,
      selection: true,
      marked: false,
      restart: true,
    });
  }

  /// The first rectangle `range` in `unit` covers (the caret's where it is empty), relative
  /// to the text's layout: where an input method places its candidates.
  pub fn rect_for(&self, range: Range<usize>, unit: Unit) -> Rect<f32> {
    self.rect_for_bytes(self.index.byte_range(unit, range))
  }

  /// The first rectangle the bytes `range` cover, widened to whole characters (the caret's
  /// where it is empty), relative to the text's layout: where an input method places its
  /// candidates.
  pub fn rect_for_bytes(&self, range: Range<usize>) -> Rect<f32> {
    let bytes = self.index.char_range(range);
    let laid = self.laid();

    if bytes.is_empty() {
      let affinity = if bytes.start == self.focus.index {
        self.focus.affinity
      }
      else {
        Affinity::Downstream
      };

      return laid
        .caret(Position::with(bytes.start, affinity))
        .primary
        .rect();
    }

    laid
      .range_rects(bytes)
      .into_iter()
      .next()
      .unwrap_or_default()
  }

  /// The offset in `unit` nearest `x`, `y`, relative to the text's layout.
  #[must_use]
  pub fn index_at(&self, x: f32, y: f32, unit: Unit) -> usize {
    self.index.offset(unit, self.byte_at(x, y))
  }

  /// The byte nearest `x`, `y`, relative to the text's layout.
  #[must_use]
  pub fn byte_at(&self, x: f32, y: f32) -> usize {
    self.laid().hit_test(x, y).position.index
  }

  /// Applies an input method's command (`deleteBackward:`, a key it forwards): as a reader's.
  pub fn perform_input(&mut self, command: Command) -> Outcome {
    self.perform_from(command, Origin::Input)
  }
}

impl<E: TextEngine> Editor<E> {
  /// The text as a log may show it: a mark per grapheme cluster where it is secure.
  fn shown(&self) -> String {
    if self.secure {
      "\u{2022}".repeat(self.index.grapheme_count())
    }
    else {
      self.text.clone()
    }
  }
}

impl<E: TextEngine + fmt::Debug> fmt::Debug for Editor<E> {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("Editor")
      .field("engine", &self.engine)
      .field("style", &self.style)
      .field("paragraph_style", &self.paragraph_style)
      .field("width", &self.width)
      .field("multiline", &self.multiline)
      .field("read_only", &self.read_only)
      .field("secure", &self.secure)
      .field("text", &self.shown())
      .field("anchor", &self.anchor)
      .field("focus", &self.focus)
      .field("marked", &self.marked.as_ref().map(|marked| &marked.range))
      .field("goal", &self.goal)
      .field("held", &self.held.is_some())
      .field("arbiter", &self.arbiter.is_some())
      .field("notifier", &self.notifier.is_some())
      .finish_non_exhaustive()
  }
}
