//! The text entries: a field and an editor.

use std::rc::Rc;

use skene_foundation::{
  Border, Inset, Ring, State, SystemColors, Value, When as _, behavior::TextEntry,
};

use crate::{
  Control, Signal, TextSelection,
  control::control_members,
  view::{Cx, Node, View},
};

/// The keyboard a text field asks the platform's input for: the on-screen keyboard's layout on
/// iOS and Android, the input method's content purpose on Linux (text-input-v3, IBus, Fcitx5),
/// the `<input>`'s type on the web.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Keyboard {
  /// Any text.
  #[default]
  Text,
  /// An email address.
  Email,
  /// A number.
  Number,
  /// A phone number.
  Phone,
  /// A URL.
  Url,
}

impl Keyboard {
  /// Its name, as the platform's input is told it.
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::Text => "text",
      Self::Email => "email",
      Self::Number => "number",
      Self::Phone => "phone",
      Self::Url => "url",
    }
  }
}

/// The controlled text and selection setters of a text entry with a `control:
/// Control<TextEntry>` field.
macro_rules! editing {
  () => {
    /// The text.
    pub fn set_text(self, text: impl Into<String>) -> Self {
      let text = text.into();

      self.derive_text(move || text.clone())
    }

    /// The text, kept current.
    pub fn derive_text<S: Into<String>>(mut self, text: impl Fn() -> S + 'static) -> Self {
      self.control = self
        .control
        .map_behavior(|entry| entry.derive_text(move || text().into()));

      self
    }

    /// Runs `change` with the whole text each edit proposes, before it shows.
    ///
    /// Where the truth then is that text the platform applies the edit itself (caret, undo
    /// and the input method intact); otherwise the truth shows, as the smallest change, the
    /// caret mapped through it.
    pub fn handle_change(mut self, change: impl Fn(String) + 'static) -> Self {
      self.control = self
        .control
        .map_behavior(|entry| entry.handle_change(change));

      self
    }

    /// Shows `text` and writes each edit into it.
    pub fn bind_text(self, text: Signal<String>) -> Self {
      self
        .derive_text(move || text.get())
        .handle_change(move |value| text.set(value))
    }

    /// The selection, in grapheme clusters of the text, kept current: the selection is then
    /// controlled, each move of it reported to `handle_selection_change`.
    pub fn derive_selection(mut self, selection: impl Fn() -> TextSelection + 'static) -> Self {
      self.control = self
        .control
        .map_behavior(|entry| entry.derive_selection(selection));

      self
    }

    /// Runs `select` with each move of the caret or the selection, in grapheme clusters,
    /// where the selection is derived.
    pub fn handle_selection_change(mut self, select: impl Fn(TextSelection) + 'static) -> Self {
      self.control = self
        .control
        .map_behavior(|entry| entry.handle_selection_change(select));

      self
    }

    /// Places the selection at `selection` and writes each move into it.
    pub fn bind_selection(self, selection: Signal<TextSelection>) -> Self {
      self
        .derive_selection(move || selection.get())
        .handle_selection_change(move |moved| selection.set(moved))
    }

    /// Whether the text can be selected and copied but not changed.
    pub fn set_read_only(self, read_only: bool) -> Self {
      self.derive_read_only(move || read_only)
    }

    /// Whether the text can be selected and copied but not changed, kept current.
    pub fn derive_read_only(mut self, read_only: impl Fn() -> bool + 'static) -> Self {
      self.control = self
        .control
        .map_behavior(|entry| entry.derive_read_only(read_only));

      self
    }
  };
}

/// The defaults every text entry's box starts from: 34 pixels tall at least and 60 wide, its
/// corners rounded by 8, padded 6 above and below and 9 at the sides inside a border of 1 in
/// [`SystemColors::BORDER`] (the accent while focused, with a line of 1 in the accent just
/// inside it), filled with [`SystemColors::FIELD`], its text in [`SystemColors::INK`], its
/// placeholder in [`SystemColors::MUTED_INK`], its selection in [`SystemColors::SELECTION`]
/// (and [`SystemColors::SELECTION_INACTIVE`] without focus).
fn entry(lines: u16) -> Control<TextEntry> {
  Control::new(TextEntry::new(lines))
    .set_min_height(if lines > 1 { 96 } else { 34 })
    .set_min_width(60)
    .set_corner_radius(8)
    .set_padding(Inset::new(6, 9, 6, 9))
    .set_border(Border::new(1))
    .set_background(SystemColors::FIELD)
    .set_color(SystemColors::INK)
    .set_border_color(Value::from(SystemColors::BORDER).when(State::Focused, SystemColors::ACCENT))
    .set_ring(Ring::NONE.when(State::Focused, Ring::new(1).set_offset(-2)))
    .set_ring_color(SystemColors::ACCENT)
    .set_selection_color(
      Value::from(SystemColors::SELECTION_INACTIVE).when(State::Focused, SystemColors::SELECTION),
    )
    .set_placeholder_color(SystemColors::MUTED_INK)
}

/// A one-line text entry: Skene's text field, drawn on every platform. It is a [`Control`] over
/// a one-line [`TextEntry`], the platform's own text view whose box the style decorates: its
/// input methods, its clipboard and its selection gestures are the platform's.
///
/// Its caption sits before it among its siblings, a press on it focusing the field. It is
/// controlled: it shows what [`TextField::derive_text`] returns, reports each edit to
/// [`TextField::handle_change`] as the whole text it proposes, and shows the truth; its
/// selection is controlled the same way where it is derived ([`TextField::bind_selection`]),
/// in grapheme clusters.
#[must_use]
pub struct TextField {
  pub(super) control: Control<TextEntry>,
}

impl TextField {
  /// An empty text field labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self {
      control: entry(1).set_label(label),
    }
  }

  editing!();

  /// The text shown while the field is empty.
  pub fn set_placeholder(self, placeholder: impl Into<Rc<str>>) -> Self {
    let placeholder = placeholder.into();

    self.derive_placeholder(move || Rc::clone(&placeholder))
  }

  /// The text shown while the field is empty, kept current.
  pub fn derive_placeholder<S: Into<Rc<str>>>(
    mut self,
    placeholder: impl Fn() -> S + 'static,
  ) -> Self {
    self.control = self
      .control
      .map_behavior(|entry| entry.derive_placeholder(move || placeholder().into()));

    self
  }

  /// Whether what is typed is hidden: a password.
  pub fn set_secure(self, secure: bool) -> Self {
    self.derive_secure(move || secure)
  }

  /// Whether what is typed is hidden, kept current.
  pub fn derive_secure(mut self, secure: impl Fn() -> bool + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|entry| entry.derive_secure(secure));

    self
  }

  /// The keyboard the field asks the platform's input for.
  pub fn set_keyboard(self, keyboard: Keyboard) -> Self {
    self.derive_keyboard(move || keyboard)
  }

  /// The keyboard the field asks the platform's input for, kept current.
  pub fn derive_keyboard(mut self, keyboard: impl Fn() -> Keyboard + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|entry| entry.derive_keyboard(move || Rc::from(keyboard().name())));

    self
  }

  /// Runs `activate` when the reader submits the field (`Enter`).
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|entry| entry.handle_activate(activate));

    self
  }

  control_members!(control.members);
}

impl View for TextField {
  fn build(self, cx: &mut Cx) -> Node {
    self.control.build(cx)
  }
}

/// A text entry of several lines: Skene's text editor, drawn on every platform. It is a
/// [`Control`] over a [`TextEntry`] of several lines, the platform's own text view, 96 pixels
/// tall at least; past its greatest height (`set_max_height`) it scrolls itself, as a
/// `<textarea>` does, keeping its caret in view.
///
/// It is controlled as a [`TextField`] is.
#[must_use]
pub struct TextEditor {
  pub(super) control: Control<TextEntry>,
}

impl TextEditor {
  /// An empty text editor labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self {
      control: entry(4).set_label(label),
    }
  }

  editing!();

  control_members!(control.members);
}

impl View for TextEditor {
  fn build(self, cx: &mut Cx) -> Node {
    self.control.build(cx)
  }
}
