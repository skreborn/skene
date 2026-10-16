//! Content and control values: their uniform names and types, the controlled contract and the
//! text arithmetic every realization shares.

use std::{
  cell::{Cell, RefCell},
  marker::PhantomData,
  rc::Rc,
};

use unicode_segmentation::UnicodeSegmentation;

use crate::{
  NodeId,
  seam::{
    Realization,
    picture::Picture,
    record::{Prop, Resolved},
    tree,
  },
  untrack,
};

/// A node's content or a text entry's value, by uniform name: what [`ContentKey`]s name and
/// what a realization dispatches on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Content {
  /// A text's plain text ([`PropValue::Text`]).
  Text,
  /// A text's rich text: runs of styled text, some of them links ([`PropValue::Runs`]).
  Runs,
  /// The texts a text lays out as wide as the widest of, beside its own: a picker's face
  /// showing its chosen option as wide as its widest ([`PropValue::Texts`]).
  Reserve,
  /// A text entry's text with its selection, written as one ([`PropValue::Edit`]).
  Edit,
  /// The text a text entry shows while empty ([`PropValue::Text`]).
  Placeholder,
  /// Whether a text entry's text can be selected but not changed ([`PropValue::Bool`]).
  ReadOnly,
  /// Whether a text entry hides what is typed ([`PropValue::Bool`]).
  Secure,
  /// The keyboard a text entry asks the platform's input for, by name: `text`, `email`,
  /// `number`, `phone` or `url` ([`PropValue::Text`]).
  Keyboard,
  /// The picture an image shows, a path among the app's resources ([`PropValue::Text`]).
  Source,
  /// Where an acting box leads, a link's destination ([`PropValue::Text`]).
  Destination,
  /// What a painted node shows: its picture, replayed in the scene's order
  /// ([`PropValue::Picture`]).
  Picture,
  /// Where an overlay is placed against its anchor ([`PropValue::Placement`]).
  Placement,
  /// A value of a library's widget adopted through the native door, by the library's own
  /// name: only the door's [`crate::seam::door::ControlSink`] knows it.
  Door(&'static str),
}

impl Content {
  /// Whether a realization must read the content back from its toolkit
  /// ([`crate::seam::testing::contents_gate`] checks it). Rich text is shown only; a reader's
  /// view of it is checked through the accessibility layer.
  #[must_use]
  pub const fn readable(self) -> bool {
    !matches!(self, Self::Runs)
  }
}

/// A text selection in grapheme clusters of the value: `end` is the caret, before `start`
/// when the selection was made backward.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct TextSelection {
  /// Where the selection was started (the anchor).
  pub start: usize,
  /// Where the caret is.
  pub end: usize,
}

/// A value on a range, with the range: what a slider's key table reads.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Range {
  /// The value.
  pub value: f64,
  /// The least value.
  pub minimum: f64,
  /// The greatest value.
  pub maximum: f64,
  /// The step the value moves by.
  pub step: f64,
}

/// The side of its anchor an overlay opens on.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Side {
  /// Above it.
  Top,
  /// Below it.
  #[default]
  Bottom,
  /// Before it in the reading direction.
  Leading,
  /// After it in the reading direction.
  Trailing,
}

/// Where an overlay is placed: against which node, on which side, and whether it is at least
/// as wide as that node.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OverlayPlacement {
  /// The node it is placed against, once built.
  pub anchor: Option<NodeId>,
  /// The side of the anchor it opens on.
  pub side: Side,
  /// Whether it is at least as wide as the anchor.
  pub widen: bool,
}

/// A text entry's text with its selection: one value, so a selection is never applied to a
/// text it does not index.
#[derive(Clone, PartialEq, Debug)]
pub struct Edit {
  /// The text.
  pub text: Rc<str>,
  /// The selection, where it is controlled; `None` leaves it to the platform, mapped
  /// through every write.
  pub selection: Option<TextSelection>,
}

impl Edit {
  /// `text` with an uncontrolled selection.
  #[must_use]
  pub fn text(text: impl Into<Rc<str>>) -> Self {
    Self {
      text: text.into(),
      selection: None,
    }
  }
}

/// A run of rich text: its text, its run properties resolved, and where it leads if it is a
/// link.
#[derive(Clone, PartialEq, Debug)]
pub struct Run {
  /// The text.
  pub text: Rc<str>,
  /// Its run properties ([`crate::seam::record::Group::Run`]), resolved
  /// ([`crate::seam::record::Record::resolve_run`]).
  pub props: Rc<[(Prop, Resolved)]>,
  /// Its destination, where it is a link: following it is reported
  /// ([`crate::seam::report::Report::Follow`]).
  pub destination: Option<Rc<str>>,
}

/// A content value as it crosses the seam, erased.
#[derive(Clone, PartialEq, Debug)]
pub enum PropValue {
  /// A flag.
  Bool(bool),
  /// A number.
  Number(f64),
  /// A text.
  Text(Rc<str>),
  /// Texts.
  Texts(Rc<[Rc<str>]>),
  /// A text with its selection.
  Edit(Edit),
  /// Rich text.
  Runs(Rc<[Run]>),
  /// A painted node's picture.
  Picture(Rc<Picture>),
  /// An overlay's placement.
  Placement(OverlayPlacement),
}

/// A type a content value holds.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a content value",
  note = "a content name holds a flag, a number, a text, texts, an edit, runs, a picture or a placement"
)]
pub trait ContentType: Clone + PartialEq + 'static {
  /// This value, erased.
  fn into_value(self) -> PropValue;

  /// The value `value` holds, where it is of this type.
  fn from_value(value: &PropValue) -> Option<Self>;
}

macro_rules! content_types {
  ($($ty:ty => $variant:ident,)*) => {$(
    impl ContentType for $ty {
      #[inline(always)]
      fn into_value(self) -> PropValue {
        PropValue::$variant(self)
      }

      #[inline(always)]
      fn from_value(value: &PropValue) -> Option<Self> {
        match value {
          PropValue::$variant(value) => Some(value.clone()),
          _ => None,
        }
      }
    }
  )*};
}

content_types! {
  bool => Bool,
  f64 => Number,
  Rc<str> => Text,
  Rc<[Rc<str>]> => Texts,
  Edit => Edit,
  Rc<[Run]> => Runs,
  Rc<Picture> => Picture,
  OverlayPlacement => Placement,
}

/// A typed content name: what a view binds or controls, so a value of the wrong type does
/// not compile.
///
/// ```
/// use skene_foundation::{NodeId, seam::{Realization, content::contents, tree}};
///
/// fn wire<R: Realization>(id: NodeId) {
///   tree::bind::<R, _>(id, contents::SECURE, || true);
/// }
/// ```
///
/// ```compile_fail,E0271
/// use skene_foundation::{NodeId, seam::{Realization, content::contents, tree}};
///
/// fn wire<R: Realization>(id: NodeId) {
///   // `SECURE` holds a flag: a number does not compile.
///   tree::bind::<R, _>(id, contents::SECURE, || 1.0_f64);
/// }
/// ```
pub struct ContentKey<T> {
  content: Content,
  ty: PhantomData<fn() -> T>,
}

impl<T> Clone for ContentKey<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T> Copy for ContentKey<T> {}

impl<T: ContentType> ContentKey<T> {
  const fn new(content: Content) -> Self {
    Self {
      content,
      ty: PhantomData,
    }
  }

  /// The key of a library's door value named `name`.
  #[must_use]
  pub const fn door(name: &'static str) -> Self {
    Self::new(Content::Door(name))
  }

  /// Its name.
  #[inline(always)]
  #[must_use]
  pub const fn content(self) -> Content {
    self.content
  }
}

/// The typed key of every content name.
pub mod contents {
  use std::rc::Rc;

  use super::{Content, ContentKey, Edit, OverlayPlacement, Run};
  use crate::seam::picture::Picture;

  /// A text's plain text.
  pub const TEXT: ContentKey<Rc<str>> = ContentKey::new(Content::Text);
  /// A text's rich text.
  pub const RUNS: ContentKey<Rc<[Run]>> = ContentKey::new(Content::Runs);
  /// The texts a text is as wide as the widest of.
  pub const RESERVE: ContentKey<Rc<[Rc<str>]>> = ContentKey::new(Content::Reserve);
  /// A text with its selection.
  pub const EDIT: ContentKey<Edit> = ContentKey::new(Content::Edit);
  /// The text shown while empty.
  pub const PLACEHOLDER: ContentKey<Rc<str>> = ContentKey::new(Content::Placeholder);
  /// Whether its text is read only.
  pub const READ_ONLY: ContentKey<bool> = ContentKey::new(Content::ReadOnly);
  /// Whether it hides what is typed.
  pub const SECURE: ContentKey<bool> = ContentKey::new(Content::Secure);
  /// The keyboard it asks for, by name.
  pub const KEYBOARD: ContentKey<Rc<str>> = ContentKey::new(Content::Keyboard);
  /// The picture shown.
  pub const SOURCE: ContentKey<Rc<str>> = ContentKey::new(Content::Source);
  /// Where it leads.
  pub const DESTINATION: ContentKey<Rc<str>> = ContentKey::new(Content::Destination);
  /// What it shows, recorded.
  pub const PICTURE: ContentKey<Rc<Picture>> = ContentKey::new(Content::Picture);
  /// Where an overlay is placed.
  pub const PLACEMENT: ContentKey<OverlayPlacement> = ContentKey::new(Content::Placement);
}

/// The answer to a reported change.
#[derive(Clone, PartialEq, Debug)]
pub enum Verdict {
  /// The truth is what the platform shows once the change applies: let the toolkit apply it
  /// natively, or keep it where it already did (caret, undo grouping, the reader's events and
  /// the input method stay intact).
  Accept,
  /// The truth is this value: where the toolkit asks before applying, stop it and apply
  /// this instead; where it applied first, repair it to this as the smallest edit, the
  /// selection mapped through it.
  Replace(PropValue),
  /// Not a change: the realization's own write, or an echo of one.
  Ignore,
}

/// The unit a toolkit counts text offsets in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Unit {
  /// UTF-8 code units, bytes (Wayland's text-input-v3, Rust's `str`).
  Utf8,
  /// Unicode scalar values (GTK, Pango, IBus, Fcitx).
  CodePoint,
  /// UTF-16 code units (the web's DOM, UIKit and AppKit's `NSString`, Android's `String`,
  /// WinRT's `HSTRING`).
  Utf16,
}

impl Unit {
  /// The length of `text` in this unit.
  #[must_use]
  pub fn count(self, text: &str) -> usize {
    match self {
      Self::Utf8 => text.len(),
      Self::CodePoint => text.chars().count(),
      Self::Utf16 => text.encode_utf16().count(),
    }
  }

  /// The byte index of `offset`, clamped to the text, an offset inside a character taken to
  /// its start.
  #[must_use]
  pub fn byte(self, text: &str, offset: usize) -> usize {
    let mut units = 0;

    for (byte, character) in text.char_indices() {
      let width = match self {
        Self::Utf8 => character.len_utf8(),
        Self::CodePoint => 1,
        Self::Utf16 => character.len_utf16(),
      };

      if units + width > offset {
        return byte;
      }

      units += width;
    }

    text.len()
  }

  /// The offset of byte index `byte`, which lies on a character boundary.
  #[must_use]
  pub fn offset(self, text: &str, byte: usize) -> usize {
    self.count(&text[..byte.min(text.len())])
  }
}

/// Grapheme clusters against a toolkit's offsets.
pub struct Graphemes;

impl Graphemes {
  /// The cluster at byte index `byte`, rounded down inside a cluster.
  #[must_use]
  pub fn cluster_at_byte(text: &str, byte: usize) -> usize {
    text
      .grapheme_indices(true)
      .take_while(|(start, cluster)| start + cluster.len() <= byte)
      .count()
  }

  /// The byte index of cluster `index`, clamped into the text.
  #[must_use]
  pub fn byte_of_cluster(text: &str, index: usize) -> usize {
    text
      .grapheme_indices(true)
      .nth(index)
      .map_or(text.len(), |(start, _)| start)
  }

  /// The cluster at `offset` in `unit`, rounded down inside a cluster.
  #[must_use]
  pub fn cluster(text: &str, unit: Unit, offset: usize) -> usize {
    Self::cluster_at_byte(text, unit.byte(text, offset))
  }

  /// The offset in `unit` of cluster `index`, clamped into the text.
  #[must_use]
  pub fn offset(text: &str, unit: Unit, index: usize) -> usize {
    unit.offset(text, Self::byte_of_cluster(text, index))
  }
}

/// The smallest edit turning `old` into `new`, in `unit`: the range of `old` replaced and
/// the text replacing it. It compares bytes, so it costs a scan of the common prefix and
/// suffix and a count of the prefix.
#[must_use]
pub fn splice(old: &str, new: &str, unit: Unit) -> (usize, usize, String) {
  let (start, end, replacement) = splice_bytes(old, new);

  (
    unit.offset(old, start),
    unit.offset(old, end),
    replacement.to_owned(),
  )
}

/// The smallest edit turning `old` into `new`, in bytes on character boundaries.
#[must_use]
pub fn splice_bytes<'a>(old: &str, new: &'a str) -> (usize, usize, &'a str) {
  let (a, b) = (old.as_bytes(), new.as_bytes());
  let mut prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();

  while !old.is_char_boundary(prefix) {
    prefix -= 1;
  }

  let room = a.len().min(b.len()) - prefix;
  let mut suffix = a
    .iter()
    .rev()
    .zip(b.iter().rev())
    .take(room)
    .take_while(|(x, y)| x == y)
    .count();

  while !old.is_char_boundary(a.len() - suffix) || !new.is_char_boundary(b.len() - suffix) {
    suffix -= 1;
  }

  (prefix, a.len() - suffix, &new[prefix..b.len() - suffix])
}

/// An offset mapped through a splice of `start..end` replaced by `inserted` units: after the
/// range it moves with it, inside it it goes to the replacement's end.
#[must_use]
pub const fn map_through(offset: usize, start: usize, end: usize, inserted: usize) -> usize {
  if offset <= start {
    offset
  } else if offset >= end {
    offset - (end - start) + inserted
  } else {
    start + inserted
  }
}

thread_local! {
  static WRITING: Cell<bool> = const { Cell::new(false) };
}

/// Runs `write` as Skene's own write: a report the toolkit makes during it is not the
/// reader's, and is ignored.
pub fn writing<T>(write: impl FnOnce() -> T) -> T {
  let before = WRITING.with(|writing| writing.replace(true));
  let result = write();

  WRITING.with(|writing| writing.set(before));

  result
}

/// Whether Skene is writing to the platform now.
#[must_use]
pub fn is_writing() -> bool {
  WRITING.with(Cell::get)
}

/// The view side of one controlled value: its truth (the view's derive closure) and its
/// handler, kept on the platform by an effect.
///
/// It reads and writes through the realization, or through the door's sink for a widget a
/// library adopted ([`crate::seam::tree::adopt`]).
pub struct Controller<R: Realization> {
  node: NodeId,
  content: Content,
  derive: Box<dyn Fn() -> PropValue>,
  handler: Option<Box<dyn Fn(PropValue)>>,
  reporting: Cell<bool>,
  pending: Cell<bool>,
  reports: Cell<u32>,
  shown: RefCell<Option<Rc<str>>>,
  realization: PhantomData<R>,
}

impl<R: Realization> Controller<R> {
  /// The controller of `content` on `node`.
  pub(crate) fn new(
    node: NodeId,
    content: Content,
    derive: Box<dyn Fn() -> PropValue>,
    handler: Option<Box<dyn Fn(PropValue)>>,
  ) -> Rc<Self> {
    Rc::new(Self {
      node,
      content,
      derive,
      handler,
      reporting: Cell::new(false),
      pending: Cell::new(false),
      reports: Cell::new(0),
      shown: RefCell::new(None),
      realization: PhantomData,
    })
  }

  /// The truth now, read without subscribing.
  #[must_use]
  pub fn truth(&self) -> PropValue {
    untrack(|| (self.derive)())
  }

  /// The truth now, subscribing the running effect.
  pub(crate) fn tracked_truth(&self) -> PropValue {
    (self.derive)()
  }

  /// The text an edit's offsets index: what the platform shows, as the controller last
  /// wrote it or accepted it, read from the platform only the first time.
  fn shown_text(&self) -> Rc<str> {
    if let Some(text) = self.shown.borrow().as_ref() {
      return Rc::clone(text);
    }

    let text = match tree::read_content::<R>(self.node, self.content) {
      Some(PropValue::Edit(edit)) => edit.text,
      _ => Rc::from(""),
    };

    *self.shown.borrow_mut() = Some(Rc::clone(&text));

    text
  }

  fn set_shown(&self, value: &PropValue) {
    if let PropValue::Edit(edit) = value {
      *self.shown.borrow_mut() = Some(Rc::clone(&edit.text));
    }
  }

  /// Shows `truth` on the platform unless it shows it already, holding it while a report
  /// runs or an input method composes.
  pub(crate) fn show(&self, truth: &PropValue) {
    if self.reporting.get() || tree::composing::<R>(self.node) {
      self.pending.set(true);

      return;
    }

    self.pending.set(false);

    let shown = match truth {
      PropValue::Edit(Edit {
        text,
        selection: None,
      }) => self
        .shown
        .borrow()
        .as_ref()
        .is_some_and(|shown| shown == text),
      _ => tree::read_content::<R>(self.node, self.content).as_ref() == Some(truth),
    };

    if shown {
      return;
    }

    writing(|| tree::write_content::<R>(self.node, self.content, truth));
    self.set_shown(truth);
  }

  /// The reader changed the value to `proposed`, which the toolkit has `applied` already or
  /// not: runs the handler and answers with the truth. A proposal equal to the truth is an
  /// echo of a write, not a change.
  pub fn report(&self, proposed: PropValue, applied: bool) -> Verdict {
    if is_writing() {
      return Verdict::Ignore;
    }

    let truth = self.truth();

    if proposed == truth || edit_matches(&proposed, &truth) {
      self.set_shown(&proposed);

      return Verdict::Ignore;
    }

    self.reports.set(self.reports.get() + 1);
    self.reporting.set(true);

    if let Some(handler) = &self.handler {
      untrack(|| handler(proposed.clone()));
    }

    self.reporting.set(false);
    self.pending.set(false);

    let truth = self.truth();

    if proposed == truth || edit_matches(&proposed, &truth) {
      self.set_shown(&proposed);

      Verdict::Accept
    } else if applied && tree::composing::<R>(self.node) {
      self.set_shown(&proposed);
      self.pending.set(true);

      Verdict::Accept
    } else {
      self.set_shown(&truth);

      Verdict::Replace(truth)
    }
  }

  /// The reader edited the text: `start..end` of the shown text, in the realization's
  /// [`Realization::UNIT`], replaced by `replacement`, the caret after it. Only the edit
  /// crosses; the whole text is built here, once, for the handler.
  pub fn report_edit(&self, start: usize, end: usize, replacement: &str, applied: bool) -> Verdict {
    if is_writing() {
      return Verdict::Ignore;
    }

    let base = self.shown_text();
    let (from, to) = (R::UNIT.byte(&base, start), R::UNIT.byte(&base, end));
    let mut text = String::with_capacity(base.len() - (to - from) + replacement.len());

    text.push_str(&base[..from]);
    text.push_str(replacement);
    text.push_str(&base[to..]);

    let caret = from + replacement.len();
    let selection = self.controls_selection().then(|| {
      let cluster = Graphemes::cluster_at_byte(&text, caret);

      TextSelection {
        start: cluster,
        end: cluster,
      }
    });

    self.report(
      PropValue::Edit(Edit {
        text: text.into(),
        selection,
      }),
      applied,
    )
  }

  /// The caret or the selection moved, in the realization's unit over the shown text:
  /// reported where the selection is controlled, ignored at no cost elsewhere.
  pub fn report_selection(&self, anchor: usize, caret: usize) -> Verdict {
    if is_writing() || !self.controls_selection() {
      return Verdict::Ignore;
    }

    let text = self.shown_text();
    let selection = TextSelection {
      start: Graphemes::cluster(&text, R::UNIT, anchor),
      end: Graphemes::cluster(&text, R::UNIT, caret),
    };

    self.report(
      PropValue::Edit(Edit {
        text,
        selection: Some(selection),
      }),
      true,
    )
  }

  fn controls_selection(&self) -> bool {
    matches!(
      self.truth(),
      PropValue::Edit(Edit {
        selection: Some(_),
        ..
      })
    )
  }

  /// Brings the platform back to the truth: after a report the toolkit applied before
  /// asking, at the moment the realization settles it, and when a composition ends.
  pub fn settle(&self) {
    self.show(&self.truth());
  }

  /// Whether a write waits for a composition to end.
  #[must_use]
  pub const fn is_pending(&self) -> bool {
    self.pending.get()
  }

  /// How many reports changed something.
  #[must_use]
  pub const fn reports(&self) -> u32 {
    self.reports.get()
  }
}

/// Whether `proposed` matches an edit truth whose selection is left to the platform.
fn edit_matches(proposed: &PropValue, truth: &PropValue) -> bool {
  matches!(
    (proposed, truth),
    (
      PropValue::Edit(Edit { text: proposed, .. }),
      PropValue::Edit(Edit { text: truth, selection: None }),
    ) if proposed == truth
  )
}
