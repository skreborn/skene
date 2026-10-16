//! Text entries: a field or an editor drawn by Skene over the foundation's editing core
//! ([`Editor`]), its text laid out by the same Parley engine as every label (masked for a
//! secure entry, [`Masked`]), its caret, selection and composition drawn here.
//!
//! Every change goes through the editor, which proposes it to the app's truth through the
//! seam ([`SeamArbiter`]) before it stands, and announces it; [`Drawn::with_entry`] runs an
//! operation and then answers the announcements: what is repainted, the selection reported
//! where the app controls it, a composition's end settled, the primary selection offered, the
//! input method and the reader told.

use std::{
  cell::{Cell, RefCell},
  ops::Range,
  rc::Rc,
  time::Duration,
};

use skene_foundation::{
  Color, Corners,
  layout::Rect,
  seam::{
    content::{Content, Edit, PropValue, Unit, Verdict},
    report::{Answer, Report},
    runtime::Cancel,
    tree,
  },
  text::{
    Laid as _, MaskState, Masked, ParagraphStyle, RunStyle, TextIndex, Wrap,
    editing::{Arbiter, Clause, ClauseStyle, Command, Editor, Notice, Origin, Outcome},
  },
};
use skene_parley::ParleyEngine;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
  Linux,
  clipboard::{self, Selection},
  look::RunProps,
  node::{Drawn, Node},
  paint::Canvas,
  palette::{DEFAULT_SIZE, Palette},
  shell, text,
};

/// The engine every entry lays its text out with: Parley's, masked where the entry is secure.
pub type Engine = Masked<ParleyEngine>;

/// The space between an entry's edges and its text, across and along.
pub const INSET: (f32, f32) = (10., 7.);

/// What a secure entry shows for each cluster of its text.
const MARK: char = '•';

/// How long a secure entry shows the cluster just typed.
const REVEAL: Duration = Duration::from_millis(900);

/// How long the caret stays on and off while it blinks.
const BLINK: Duration = Duration::from_millis(530);

/// How long after the last change the caret stops blinking, shown.
const BLINK_FOR: Duration = Duration::from_secs(10);

/// What decides an entry's changes: the app's truth, through the seam's controller.
///
/// A composition's end is settled by the seam ([`Report::Composition`]) once the editor has
/// ended it, so the editor is asked nothing then.
pub struct SeamArbiter(Rc<Cell<skene_foundation::NodeId>>);

impl Arbiter for SeamArbiter {
  fn propose(&mut self, proposed: &Edit, applied: bool, _: bool) -> Verdict {
    let answer = tree::report::<Linux>(
      self.0.get(),
      Report::Change {
        content: Content::Edit,
        proposed: PropValue::Edit(proposed.clone()),
        applied,
      },
    );

    crate::trace::log(|| {
      format!(
        "[entry] propose {:?} applied {applied} -> {answer:?}",
        proposed.text
      )
    });

    match answer {
      Answer::Verdict(verdict) => verdict,
      _ => Verdict::Accept,
    }
  }

  fn settle(&mut self) -> Option<Edit> {
    None
  }
}

/// An entry's editing state.
pub struct Entry {
  /// The editing core.
  pub editor: Editor<Engine>,
  /// Whether the text is masked, and which cluster shows as typed.
  pub mask: Rc<MaskState>,
  /// What the editor announced since the entry last answered.
  notices: Rc<RefCell<Vec<Notice>>>,
  /// How far a field's text is scrolled left, in pixels.
  pub scroll: f32,
  /// Whether it holds several lines.
  pub multiline: bool,
  /// Whether the caret shows now (it blinks).
  pub caret_on: bool,
  /// Stops the caret's blinking.
  blink: Option<Cancel>,
  /// When the caret last started blinking.
  blink_started: std::time::Instant,
  /// Ends the moment a secure entry shows the cluster just typed.
  reveal: Option<Cancel>,
  /// The selection last offered as the primary selection.
  offered: Option<String>,
}

impl Entry {
  /// The entry of `node`, empty.
  fn new(node: &Node, multiline: bool) -> Self {
    let mask = Rc::new(MaskState::new(MARK));
    let engine = Masked::new(text::engine(), Rc::clone(&mask));
    let mut editor = Editor::new(
      engine,
      RunStyle::new(DEFAULT_SIZE),
      ParagraphStyle::default().with_wrap(if multiline { Wrap::Words } else { Wrap::None }),
    );
    let notices = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&notices);
    let id = Rc::new(Cell::new(node.id.get()));

    editor.set_multiline(multiline);
    editor.set_arbiter(SeamArbiter(id));
    editor.set_notifier(move |notice| sink.borrow_mut().push(notice));

    Self {
      editor,
      mask,
      notices,
      scroll: 0.,
      multiline,
      caret_on: true,
      blink: None,
      blink_started: std::time::Instant::now(),
      reveal: None,
      offered: None,
    }
  }

  /// Where the text's top left sits for an entry at `rect` (window pixels).
  pub fn origin(&self, rect: Rect) -> (f32, f32) {
    if self.multiline {
      (rect.x + INSET.0, rect.y + INSET.1)
    } else {
      let height = self.editor.laid().size().height;

      (
        rect.x + INSET.0 - self.scroll,
        rect.y + (rect.height - height) / 2.,
      )
    }
  }

  /// Scrolls a field so its caret shows within `width` (the room inside its insets).
  fn reveal_caret(&mut self, width: f32) {
    if self.multiline {
      self.scroll = 0.;

      return;
    }

    let caret = self.editor.caret_rect().x;
    let extent = self.editor.laid().size().width;
    let mut scroll = self.scroll;

    if caret - scroll < 0. {
      scroll = caret;
    } else if caret - scroll > width - 1. {
      scroll = caret - width + 1.;
    }

    self.scroll = scroll.clamp(0., (extent - width + 1.).max(0.));
  }
}

impl Drawn {
  /// Runs `op` on its entry, made where it has none yet, then answers what changed;
  /// `None` where it is no entry.
  pub fn with_entry<T>(self: &Rc<Self>, op: impl FnOnce(&mut Entry) -> T) -> Option<T> {
    let multiline = match self.spec.kind {
      skene_foundation::seam::node::NodeKind::Control(kind) => match kind {
        skene_foundation::seam::node::ControlKind::TextField => false,
        skene_foundation::seam::node::ControlKind::TextEditor => true,
        _ => return None,
      },
      _ => return None,
    };
    let result = {
      let mut entry = self.entry.try_borrow_mut().ok()?;
      let made = entry.is_none();
      let entry = entry.get_or_insert_with(|| Entry::new(self, multiline));

      if made && let Some(edit) = self.contents.borrow().edit.clone() {
        // The app's truth written before the entry was made shows from its start.
        entry.editor.write(&edit);
      }

      op(entry)
    };

    self.settle_entry();

    Some(result)
  }

  /// Whether it holds an entry now (made by its first preparation).
  pub fn has_entry(self: &Rc<Self>) -> bool {
    self
      .entry
      .try_borrow()
      .map(|entry| entry.is_some())
      .unwrap_or(true)
  }

  /// Answers what its entry announced since the last time.
  fn settle_entry(self: &Rc<Self>) {
    let Ok(mut borrowed) = self.entry.try_borrow_mut() else {
      return;
    };
    let Some(entry) = borrowed.as_mut() else {
      return;
    };
    let notices: Vec<Notice> = std::mem::take(&mut *entry.notices.borrow_mut());

    if notices.is_empty() && !self.edit_pending.get() {
      return;
    }

    let was_composing = self.composing.get();
    let composing = entry.editor.is_composing();
    let text_changed = notices.iter().any(|notice| notice.text);
    let reader_moved = notices
      .iter()
      .any(|notice| notice.selection && !notice.text && notice.origin != Origin::App);
    let input = notices.iter().any(|notice| notice.origin == Origin::Input);
    let selection = entry.editor.selection_in(Unit::Utf8);
    let secure = entry.mask.is_masked();
    let selected = entry.editor.selected();
    let primary = (!secure
      && !selected.is_empty()
      && notices
        .iter()
        .any(|notice| notice.origin != Origin::App && notice.selection))
    .then(|| entry.editor.text()[selected].to_owned())
    .filter(|text| entry.offered.as_deref() != Some(text.as_str()));

    if let Some(text) = &primary {
      entry.offered = Some(text.clone());
    }

    let width = (self.frame.get().width - INSET.0 * 2.).max(1.);

    entry.reveal_caret(width);
    entry.caret_on = true;
    entry.blink_started = std::time::Instant::now();
    drop(borrowed);

    self.composing.set(composing);
    shell::invalidate_paint(self);
    ime_dirty(input);
    ACCESS.with(|access| access.set(true));

    if text_changed
      && self.spec.kind
        == skene_foundation::seam::node::NodeKind::Control(
          skene_foundation::seam::node::ControlKind::TextEditor,
        )
    {
      shell::invalidate_layout();
    }

    if let Some(text) = primary {
      clipboard::write(Selection::Primary, text);
    }

    if was_composing && !composing {
      tree::report::<Linux>(self.id.get(), Report::Composition(false));
    } else if !was_composing && composing {
      tree::report::<Linux>(self.id.get(), Report::Composition(true));
    }

    if reader_moved {
      tree::report::<Linux>(
        self.id.get(),
        Report::Selection {
          anchor: selection.0,
          caret: selection.1,
        },
      );
    }

    if self.edit_pending.replace(false) {
      let edit = self.contents.borrow().edit.clone();

      if let Some(edit) = edit {
        self.write_entry(&edit);
      }
    }
  }

  /// Shows the app's truth `edit` in its entry: at once, or once the operation running on
  /// it ends.
  pub fn write_entry(self: &Rc<Self>, edit: &Edit) {
    if self.entry.try_borrow_mut().is_err() {
      self.edit_pending.set(true);

      return;
    }

    self.with_entry(|entry| entry.editor.write(edit));
  }

  /// Gives its entry the run properties it inherits and its paragraph style; whether a
  /// metric changed.
  pub fn prepare_entry(self: &Rc<Self>, props: &RunProps, palette: &Palette) -> bool {
    let disabled = self.contents.borrow().disabled;
    let ink = if disabled { palette.dim } else { palette.ink };
    let style = props.run_style(ink, DEFAULT_SIZE);
    let (read_only, secure) = {
      let contents = self.contents.borrow();

      (contents.read_only, contents.secure)
    };

    self
      .with_entry(|entry| {
        let paragraph = props.paragraph_style().with_wrap(if entry.multiline {
          Wrap::Words
        } else {
          Wrap::None
        });
        let before = entry.editor.laid().size();

        entry.editor.set_read_only(read_only);

        if entry.mask.is_masked() != secure {
          entry.mask.set_masked(secure);
          entry.editor.relayout();
        }

        entry.editor.set_style(style);
        entry.editor.set_paragraph_style(paragraph);

        entry.editor.laid().size() != before
      })
      .unwrap_or(false)
  }

  /// The height of one line of its entry, and its first baseline.
  pub fn entry_line_metrics(self: &Rc<Self>) -> (f32, f32) {
    self
      .with_entry(|entry| {
        let laid = entry.editor.laid();
        let lines = laid.lines();
        let height = lines.first().map_or(laid.size().height, |line| line.height);

        (height, laid.first_baseline())
      })
      .unwrap_or((17., 13.))
  }

  /// The height its editor takes at `width` (its box's), insets included.
  pub fn entry_height_at(self: &Rc<Self>, width: Option<f32>) -> f32 {
    self
      .with_entry(|entry| {
        entry
          .editor
          .set_width(width.map(|width| (width - INSET.0 * 2.).max(1.)));

        entry.editor.laid().size().height + INSET.1 * 2.
      })
      .unwrap_or(34.)
  }

  /// Lays its editor out at the width its box was given.
  pub fn fit_entry(self: &Rc<Self>) {
    let width = self.laid_width.get();

    self.with_entry(|entry| {
      if entry.multiline {
        entry.editor.set_width(Some((width - INSET.0 * 2.).max(1.)));
      }
    });
  }

  /// Draws its entry text in the box at `rect`: the selection, the text (or the
  /// placeholder while it is empty), the caret where it has focus.
  pub fn paint_entry(self: &Rc<Self>, canvas: &mut Canvas<'_>, rect: Rect, focused: bool) {
    let palette = canvas.palette;
    let placeholder = self.text.borrow().clone();
    let Ok(borrowed) = self.entry.try_borrow() else {
      return;
    };
    let Some(entry) = borrowed.as_ref() else {
      return;
    };
    let laid = entry.editor.laid();
    let origin = entry.origin(rect);
    let inner = Rect {
      x: rect.x + INSET.0 / 2.,
      y: rect.y + 2.,
      width: (rect.width - INSET.0).max(0.),
      height: (rect.height - 4.).max(0.),
    };
    let selection = if focused {
      palette.accent.with_alpha(0x55)
    } else {
      palette.dim.with_alpha(0x40)
    };
    let empty = entry.editor.text().is_empty();
    let caret = entry.editor.caret_rect();
    let show_caret = focused
      && entry.caret_on
      && (entry.editor.selected().is_empty() || entry.editor.is_composing())
      && !self.contents.borrow().read_only;

    canvas.clipped(inner, |canvas| {
      for range in entry.editor.selection_rects() {
        canvas.fill(
          Rect {
            x: origin.0 + range.x,
            y: origin.1 + range.y,
            width: range.width.max(2.),
            height: range.height,
          },
          Corners::all(0.),
          selection,
        );
      }

      if empty && !entry.editor.is_composing() {
        if let Some(laid) = placeholder.as_ref().and_then(|cell| cell.laid(None)) {
          canvas.text(&laid, origin);
        }
      } else {
        canvas.text(laid.inner(), origin);
      }

      if show_caret {
        canvas.fill(
          Rect {
            x: (origin.0 + caret.x).round() - 0.5,
            y: origin.1 + caret.y,
            width: 1.5,
            height: caret.height,
          },
          Corners::all(0.),
          self.caret_ink(&palette),
        );
      }
    });
  }

  fn caret_ink(self: &Rc<Self>, palette: &Palette) -> Color {
    self.run.borrow().color.unwrap_or(palette.ink)
  }

  /// Starts or restarts the focused entry's caret blinking, where the reader's caret blinks.
  pub fn blink_caret(self: &Rc<Self>) {
    if !crate::environment::cursor_blinks() {
      self.stop_blink();

      return;
    }

    let target = Rc::clone(self);
    let cancel = crate::runtime::schedule(
      BLINK,
      Box::new(move || {
        let more = target
          .with_entry(|entry| {
            entry.blink = None;

            if entry.blink_started.elapsed() > BLINK_FOR {
              entry.caret_on = true;

              return false;
            }

            entry.caret_on = !entry.caret_on;

            true
          })
          .unwrap_or(false);

        shell::invalidate_paint(&target);

        if more && shell::is_focused(&target) {
          target.blink_caret();
        }
      }),
    );

    self.with_entry(|entry| {
      if let Some(previous) = entry.blink.replace(cancel) {
        previous.cancel();
      }
    });
  }

  /// Stops the caret blinking.
  pub fn stop_blink(self: &Rc<Self>) {
    self.with_entry(|entry| {
      if let Some(blink) = entry.blink.take() {
        blink.cancel();
      }

      entry.caret_on = true;
    });
  }

  /// Applies `command` from the reader, putting what it copies on the clipboard; what a secure
  /// entry would copy goes nowhere.
  pub fn perform(self: &Rc<Self>, command: Command) -> Outcome {
    let typed = matches!(&command, Command::Insert(text) if text.graphemes(true).count() == 1);
    let described = format!("{command:?}");
    let outcome = self
      .with_entry(|entry| entry.editor.perform(command))
      .unwrap_or(Outcome::IGNORED);

    crate::trace::log(|| format!("[entry] {described} -> {outcome:?}"));

    if let Some(text) = &outcome.clipboard {
      clipboard::write(Selection::Clipboard, text.clone());
    }

    if typed && outcome.handled {
      self.reveal_typed();
    }

    outcome
  }

  /// Shows the cluster before the caret of a secure entry as typed, for a moment.
  fn reveal_typed(self: &Rc<Self>) {
    let shown = self
      .with_entry(|entry| {
        if !entry.mask.is_masked() {
          return false;
        }

        let caret = entry.editor.focus().index;
        let text = entry.editor.text();
        let start = TextIndex::new(text)
          .snap_to_grapheme(caret.saturating_sub(1), skene_foundation::text::Round::Down);

        entry.mask.reveal(Some(start));
        entry.editor.relayout();

        true
      })
      .unwrap_or(false);

    if !shown {
      return;
    }

    let target = Rc::clone(self);
    let cancel = crate::runtime::schedule(
      REVEAL,
      Box::new(move || {
        target.with_entry(|entry| {
          entry.reveal = None;
          entry.mask.reveal(None);
          entry.editor.relayout();
        });
        shell::invalidate_paint(&target);
      }),
    );

    self.with_entry(|entry| {
      if let Some(previous) = entry.reveal.replace(cancel) {
        previous.cancel();
      }
    });
  }

  /// Pastes `selection`'s text into its entry once it arrives.
  pub fn paste(self: &Rc<Self>, selection: Selection) {
    let target = Rc::clone(self);

    clipboard::read(selection, move |text| {
      if let Some(text) = text {
        let _ = target.perform(Command::Paste(text));
      }
    });
  }

  /// A press of the pointer at `x`, `y` (window pixels) in its entry at `rect`, the
  /// `clicks`th in a row.
  pub fn press_entry(self: &Rc<Self>, rect: Rect, at: (f32, f32), clicks: u8, extend: bool) {
    self.with_entry(|entry| {
      let origin = entry.origin(rect);

      entry
        .editor
        .press(at.0 - origin.0, at.1 - origin.1, clicks, extend);
    });
  }

  /// A drag of the pointer to `x`, `y` (window pixels) after a press in its entry.
  pub fn drag_entry(self: &Rc<Self>, rect: Rect, at: (f32, f32)) {
    self.with_entry(|entry| {
      let origin = entry.origin(rect);

      entry.editor.drag(at.0 - origin.0, at.1 - origin.1);
    });
  }

  /// Focus left its entry: a composition is kept as typed, the selection collapses into
  /// nothing else.
  pub fn blur_entry(self: &Rc<Self>) {
    self.stop_blink();
    self.with_entry(|entry| entry.editor.unmark());
  }

  /// What the input method is told of its entry at `rect`.
  pub fn ime_state(self: &Rc<Self>, rect: Rect) -> Option<ImeState> {
    if self.contents.borrow().read_only || self.contents.borrow().disabled {
      return None;
    }

    let purpose = crate::ime::Purpose::of(self.contents.borrow().keyboard.as_deref());

    self.with_entry(|entry| {
      let editor = &entry.editor;
      let text = editor.text();
      let marked = editor.marked_range_in(Unit::Utf8);
      let (anchor, focus) = editor.selection_in(Unit::Utf8);
      let (mut surrounding, mut anchor, mut focus) = match &marked {
        Some(marked) => {
          let mut without = String::with_capacity(text.len());

          without.push_str(&text[..marked.start]);
          without.push_str(&text[marked.end..]);

          (without, marked.start, marked.start)
        }
        None => (text.to_owned(), anchor, focus),
      };

      if surrounding.len() > SURROUNDING {
        let mut start = anchor.min(focus).saturating_sub(SURROUNDING / 2);

        while !surrounding.is_char_boundary(start) {
          start -= 1;
        }

        let mut end = (start + SURROUNDING).min(surrounding.len());

        while !surrounding.is_char_boundary(end) {
          end -= 1;
        }

        surrounding = surrounding[start..end].to_owned();
        anchor = (anchor - start).min(surrounding.len());
        focus = (focus - start).min(surrounding.len());
      }

      let range = marked.unwrap_or(focus..focus);
      let local = editor.rect_for(range, Unit::Utf8);
      let origin = entry.origin(rect);
      let secure = entry.mask.is_masked();

      if secure {
        // A secure entry's input method is told what it shows: a mark per cluster.
        let shown = |index: usize| surrounding[..index].graphemes(true).count() * MARK.len_utf8();

        (anchor, focus) = (shown(anchor), shown(focus));
        surrounding = MARK.to_string().repeat(surrounding.graphemes(true).count());
      }

      ImeState {
        surrounding,
        cursor: focus,
        anchor,
        rect: Rect {
          x: origin.0 + local.x,
          y: origin.1 + local.y,
          width: local.width.max(1.),
          height: local.height,
        },
        secure,
        multiline: entry.multiline,
        purpose,
      }
    })
  }

  /// An input method shows `text` as its composition, its caret or selection `cursor` (bytes of
  /// `text`) where it shows one; an empty `text` ends the composition with nothing.
  pub fn ime_preedit(
    self: &Rc<Self>,
    text: &str,
    cursor: Option<Range<usize>>,
    clauses: Vec<Clause>,
  ) {
    let cursor = cursor.unwrap_or(text.len()..text.len());

    self.with_entry(|entry| {
      if text.is_empty() && !entry.editor.is_composing() {
        return;
      }

      let clauses = if clauses.is_empty() && !text.is_empty() {
        vec![Clause {
          range: 0..text.len(),
          style: ClauseStyle::Raw,
        }]
      } else {
        clauses
      };

      entry
        .editor
        .set_marked(text, clauses, cursor, None, Unit::Utf8);
    });
  }

  /// An input method commits `text` in place of its composition (or the selection).
  pub fn ime_commit(self: &Rc<Self>, text: &str) {
    self.with_entry(|entry| entry.editor.replace(None, text, Unit::Utf8));

    if text.graphemes(true).count() == 1 {
      self.reveal_typed();
    }
  }

  /// An input method deletes `range` (bytes of the text without the composition).
  pub fn ime_delete(self: &Rc<Self>, range: Range<usize>) {
    self.with_entry(|entry| {
      let marked = entry.editor.marked_range_in(Unit::Utf8);
      let shift = |at: usize| match &marked {
        Some(marked) if at > marked.start => at + marked.len(),
        _ => at,
      };

      entry
        .editor
        .replace(Some(shift(range.start)..shift(range.end)), "", Unit::Utf8);
    });
  }

  /// An input method deletes `before` bytes before the selection and `after` after it
  /// (text-input-v3's units), the composition left out; a secure entry's bytes are of the marks
  /// it showed the input method.
  pub fn ime_delete_surrounding(self: &Rc<Self>, before: usize, after: usize) {
    let Some((start, end, length, before, after)) = self.with_entry(|entry| {
      let (anchor, focus) = entry.editor.selection_in(Unit::Utf8);
      let marked = entry.editor.marked_range_in(Unit::Utf8);
      let length = entry.editor.count_in(Unit::Utf8) - marked.as_ref().map_or(0, Range::len);
      let (start, end) = match &marked {
        Some(marked) => (marked.start, marked.start),
        None => (anchor.min(focus), anchor.max(focus)),
      };
      let (before, after) = if entry.mask.is_masked() {
        let text = entry.editor.text();
        let clusters = |bytes: usize| bytes.div_ceil(MARK.len_utf8());
        let before = text[..start]
          .grapheme_indices(true)
          .rev()
          .take(clusters(before))
          .last()
          .map_or(0, |(index, _)| start - index);
        let after = text[end..]
          .graphemes(true)
          .take(clusters(after))
          .map(str::len)
          .sum();

        (before, after)
      } else {
        (before, after)
      };

      (start, end, length, before, after)
    }) else {
      return;
    };

    if after > 0 {
      self.ime_delete(end..(end + after).min(length));
    }

    if before > 0 {
      self.ime_delete(start.saturating_sub(before)..start);
    }
  }
}

thread_local! {
  /// Whether the input method must hear the focused entry's state again, and whether the
  /// last change came from it.
  static IME: Cell<(bool, bool)> = const { Cell::new((true, false)) };
  /// Whether the reader must hear the entries' state again.
  static ACCESS: Cell<bool> = const { Cell::new(true) };
}

/// Whether the input method must hear the focused entry's state again: `Some(cause)` once,
/// `cause` whether the input method made the last change.
pub fn take_ime_dirty() -> Option<bool> {
  IME.with(|ime| {
    let (dirty, cause) = ime.replace((false, false));

    dirty.then_some(cause)
  })
}

/// The input method must hear the focused entry's state again.
pub fn ime_dirty(cause_input: bool) {
  IME.with(|ime| {
    let (_, cause) = ime.get();

    ime.set((true, cause || cause_input));
  });
}

/// Whether the reader must hear the entries again: once.
pub fn take_access_dirty() -> bool {
  ACCESS.with(|access| access.replace(false))
}

/// What an input method is told of the focused entry: its text without the composition, the
/// selection in it (bytes), where the caret is (window pixels), and its purpose.
#[derive(Clone, PartialEq, Debug)]
pub struct ImeState {
  /// The text around the caret, the composition left out.
  pub surrounding: String,
  /// The caret's byte in it.
  pub cursor: usize,
  /// The selection's other end.
  pub anchor: usize,
  /// The caret's (or the composition's) rectangle, in window pixels.
  pub rect: Rect,
  /// Whether what is typed is hidden.
  pub secure: bool,
  /// Whether the entry holds several lines.
  pub multiline: bool,
  /// What its text is for.
  pub purpose: crate::ime::Purpose,
}

/// The most bytes of surrounding text an input method is given (text-input-v3's limit).
const SURROUNDING: usize = 4000;
