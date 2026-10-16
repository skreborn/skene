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
  layout::{Axis, Measurement, Rect},
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

/// The width an entry's text takes where its layout gives it none: its natural width, and
/// its least, inside its padding and border.
const TEXT_WIDTH: (f32, f32) = (180., 40.);

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
  /// How far an editor's text is scrolled up, in pixels.
  pub vscroll: f32,
  /// The caret's color, the selection's and the placeholder's, as the entry inherits them.
  pub colors: EntryColors,
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
  fn new(node: &Node, multiline: bool) -> Box<Self> {
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

    Box::new(Self {
      editor,
      mask,
      notices,
      scroll: 0.,
      vscroll: 0.,
      colors: EntryColors::default(),
      multiline,
      caret_on: true,
      blink: None,
      blink_started: std::time::Instant::now(),
      reveal: None,
      offered: None,
    })
  }

  /// Where the text's top left sits for an entry whose content box is `content` (window
  /// pixels): a field's centered across it and scrolled along it, an editor's scrolled down
  /// it.
  pub fn origin(&self, content: Rect) -> (f32, f32) {
    if self.multiline {
      (content.x, content.y - self.vscroll)
    } else {
      let height = self.editor.laid().size().height;

      (
        content.x - self.scroll,
        content.y + (content.height - height) / 2.,
      )
    }
  }

  /// How far an editor's text can scroll in a content box `height` tall.
  fn vscroll_limit(&self, height: f32) -> f32 {
    (self.editor.laid().size().height - height).max(0.)
  }

  /// Scrolls the text so its caret shows within a content box `width` by `height`.
  fn reveal_caret(&mut self, width: f32, height: f32) {
    let caret = self.editor.caret_rect();

    if self.multiline {
      let mut scroll = self.vscroll;

      if caret.y - scroll < 0. {
        scroll = caret.y;
      } else if caret.y + caret.height - scroll > height {
        scroll = caret.y + caret.height - height;
      }

      self.scroll = 0.;
      self.vscroll = scroll.clamp(0., self.vscroll_limit(height));

      return;
    }

    let extent = self.editor.laid().size().width;
    let mut scroll = self.scroll;

    if caret.x - scroll < 0. {
      scroll = caret.x;
    } else if caret.x - scroll > width - 1. {
      scroll = caret.x - width + 1.;
    }

    self.scroll = scroll.clamp(0., (extent - width + 1.).max(0.));
  }
}

/// The colors an entry draws its caret, its selection and its placeholder in, as it inherits
/// them; the platform's where unset.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct EntryColors {
  /// The caret's, the text's where unset.
  pub caret: Option<Color>,
  /// The selection's fill.
  pub selection: Option<Color>,
  /// The text's.
  pub ink: Option<Color>,
}

impl Drawn {
  /// Runs `op` on its entry, made where it has none yet, then answers what changed;
  /// `None` where it is no entry.
  pub fn with_entry<T>(self: &Rc<Self>, op: impl FnOnce(&mut Entry) -> T) -> Option<T> {
    let skene_foundation::seam::node::NodeKind::Editable { multiline } = self.spec.kind else {
      return None;
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

    let frame = self.frame.get();
    let content = self.entry_content(Rect {
      x: 0.,
      y: 0.,
      ..frame
    });

    entry.reveal_caret(content.width.max(1.), content.height.max(1.));
    entry.caret_on = true;
    entry.blink_started = std::time::Instant::now();
    drop(borrowed);

    self.composing.set(composing);
    shell::invalidate_paint(self);
    ime_dirty(input);
    ACCESS.with(|access| access.set(true));

    if text_changed
      && self.spec.kind == (skene_foundation::seam::node::NodeKind::Editable { multiline: true })
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
    let style = props.run_style(palette.ink, DEFAULT_SIZE);
    let colors = EntryColors {
      caret: props.caret_color,
      selection: props.selection_color,
      ink: props.color,
    };
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

        if entry.colors != colors {
          entry.colors = colors;
          shell::invalidate_paint(self);
        }

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

  /// The space between its box's edges and its text: its padding and its border, leading,
  /// top, trailing and bottom.
  pub fn entry_insets(&self) -> (f32, f32, f32, f32) {
    let padding = self.layout.borrow().padding;
    let border = self.look.borrow().border_width();

    padding.map_or((border, border, border, border), |padding| {
      (
        padding.leading + border,
        padding.top + border,
        padding.trailing + border,
        padding.bottom + border,
      )
    })
  }

  /// Its content box for its box at `rect`: inside its padding and border.
  pub fn entry_content(&self, rect: Rect) -> Rect {
    let (leading, top, trailing, bottom) = self.entry_insets();

    Rect {
      x: rect.x + leading,
      y: rect.y + top,
      width: (rect.width - leading - trailing).max(0.),
      height: (rect.height - top - bottom).max(0.),
    }
  }

  /// The height its editor's text takes within `width` (its content box's).
  pub fn entry_height_at(self: &Rc<Self>, width: Option<f32>) -> f32 {
    self
      .with_entry(|entry| {
        entry.editor.set_width(width.map(|width| width.max(1.)));

        entry.editor.laid().size().height
      })
      .unwrap_or(17.)
  }

  /// Its box's size along `axis`, given the other axis's size where known: its text's, its
  /// padding and border around it.
  pub fn measure_entry(
    self: &Rc<Self>,
    multiline: bool,
    axis: Axis,
    for_size: Option<f32>,
  ) -> Measurement {
    let (leading, top, trailing, bottom) = self.entry_insets();

    match axis {
      Axis::Horizontal => Measurement {
        minimum: TEXT_WIDTH.1 + leading + trailing,
        natural: TEXT_WIDTH.0 + leading + trailing,
        baseline: None,
      },
      Axis::Vertical => {
        let (line, baseline) = self.entry_line_metrics();
        let text = if multiline {
          self.entry_height_at(for_size.map(|width| width - leading - trailing))
        } else {
          line
        };
        let height = text + top + bottom;

        Measurement {
          minimum: height,
          natural: height,
          baseline: Some(top + baseline),
        }
      }
    }
  }

  /// Lays its editor out at the width its box was given.
  pub fn fit_entry(self: &Rc<Self>) {
    let content = self.entry_content(Rect {
      x: 0.,
      y: 0.,
      width: self.laid_width.get(),
      height: self.frame.get().height,
    });

    self.with_entry(|entry| {
      if entry.multiline {
        entry.editor.set_width(Some(content.width.max(1.)));
        entry.vscroll = entry.vscroll.clamp(0., entry.vscroll_limit(content.height));
      }
    });
  }

  /// Scrolls its editor by `dy` pixels within its text: whether it moved.
  pub fn scroll_entry(self: &Rc<Self>, dy: f32) -> bool {
    let height = self
      .entry_content(Rect {
        x: 0.,
        y: 0.,
        ..self.frame.get()
      })
      .height;
    let moved = self
      .with_entry(|entry| {
        if !entry.multiline {
          return false;
        }

        let next = (entry.vscroll + dy).clamp(0., entry.vscroll_limit(height));
        let moved = (next - entry.vscroll).abs() > f32::EPSILON;

        entry.vscroll = next;

        moved
      })
      .unwrap_or(false);

    if moved {
      shell::invalidate_paint(self);
    }

    moved
  }

  /// How many lines a page of its editor holds: what the page keys move its caret by.
  pub fn entry_page(self: &Rc<Self>) -> isize {
    let height = self
      .entry_content(Rect {
        x: 0.,
        y: 0.,
        ..self.frame.get()
      })
      .height;
    let (line, _) = self.entry_line_metrics();

    #[expect(clippy::cast_possible_truncation, reason = "a count of lines")]
    let lines = (height / line.max(1.)).floor() as isize;

    lines.max(1)
  }

  /// Draws its entry's text in its box at `rect`, inside its padding and border: the
  /// selection, the text (or the placeholder while it is empty), the caret where it has focus,
  /// and an editor's scroll bar where its text overflows it.
  pub fn paint_entry(self: &Rc<Self>, canvas: &mut Canvas<'_>, rect: Rect, focused: bool) {
    let palette = canvas.palette;
    let placeholder = self.text.borrow().clone();
    let read_only = self.contents.borrow().read_only;
    let Ok(borrowed) = self.entry.try_borrow() else {
      return;
    };
    let Some(entry) = borrowed.as_ref() else {
      return;
    };
    let laid = entry.editor.laid();
    let content = self.entry_content(rect);
    let border = self.look.borrow().border_width();
    let padding_box = Rect {
      x: rect.x + border,
      y: rect.y + border,
      width: (rect.width - border * 2.).max(0.),
      height: (rect.height - border * 2.).max(0.),
    };
    let origin = entry.origin(content);
    let selection = entry.colors.selection.unwrap_or(if focused {
      palette.accent.with_alpha(0x55)
    } else {
      palette.dim.with_alpha(0x40)
    });
    let caret_ink = entry
      .colors
      .caret
      .or(entry.colors.ink)
      .unwrap_or(palette.ink);
    let empty = entry.editor.text().is_empty();
    let caret = entry.editor.caret_rect();
    let show_caret = focused
      && entry.caret_on
      && (entry.editor.selected().is_empty() || entry.editor.is_composing())
      && !read_only;
    let clip = if entry.multiline {
      Rect {
        y: content.y,
        height: content.height,
        ..padding_box
      }
    } else {
      padding_box
    };

    canvas.clipped(clip, |canvas| {
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
          caret_ink,
        );
      }
    });

    let extent = laid.size().height;

    if entry.multiline && extent > content.height + 0.5 {
      let length = (content.height * content.height / extent).max(24.);
      let travel = content.height - length;
      let at = travel * (entry.vscroll / (extent - content.height)).clamp(0., 1.);

      canvas.fill(
        Rect {
          x: padding_box.x + padding_box.width - 6.,
          y: content.y + at,
          width: 4.,
          height: length,
        },
        Corners::all(2.),
        caret_ink.with_alpha(70),
      );
    }
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
    let content = self.entry_content(rect);

    self.with_entry(|entry| {
      let origin = entry.origin(content);

      entry
        .editor
        .press(at.0 - origin.0, at.1 - origin.1, clicks, extend);
    });
  }

  /// A drag of the pointer to `x`, `y` (window pixels) after a press in its entry.
  pub fn drag_entry(self: &Rc<Self>, rect: Rect, at: (f32, f32)) {
    let content = self.entry_content(rect);

    self.with_entry(|entry| {
      let origin = entry.origin(content);

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
    if self.contents.borrow().read_only || self.access.borrow().disabled {
      return None;
    }

    let purpose = crate::ime::Purpose::of(self.contents.borrow().keyboard.as_deref());
    let content = self.entry_content(rect);

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
      let origin = entry.origin(content);
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
