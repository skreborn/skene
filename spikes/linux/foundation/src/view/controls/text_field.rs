//! The text entries: a field and an editor.

use std::rc::Rc;

use crate::{
  Border, Inset, Ring, Signal, State, SystemColors, Value, When as _,
  behavior::TextEntry,
  seam::{Realization, content::TextSelection},
  view::{
    AnyKind, ContextMenu, ContextMenuBase, Control, ControlBase, ControlMembersBase, Cx,
    DecorationBase, Node, RunBase, RunWithEntries, View, Wraps, controls::control_surfaces,
    wraps_itself,
  },
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

/// A view over a text entry and its context menu: what [`EditingBase`] writes.
pub trait HasEntry<R: Realization>: Sized {
  /// This view with its text entry's control changed by `map`.
  #[must_use]
  fn map_entry(self, map: impl FnOnce(Control<TextEntry, R>) -> Control<TextEntry, R>) -> Self;

  /// Its context menu, to replace.
  fn menu_mut(&mut self) -> &mut ContextMenu<R>;
}

/// What every text entry carries: its context menu, its text, its selection and whether it is
/// read only, each controlled.
pub trait EditingBase<R: Realization>: Wraps<Inner: HasEntry<R>> {
  /// The context menu a secondary press, `Shift-F10` or the menu key opens on it, in place of
  /// the standard one ([`ContextMenuBase::standard`]); an empty one ([`ContextMenuBase::new`])
  /// opens none. Its child is the entry.
  fn set_context_menu(mut self, menu: impl Wraps<Inner = ContextMenu<R>>) -> Self {
    *self.inner_mut().menu_mut() = menu.into_inner();

    self
  }

  /// The text.
  fn set_text(self, text: impl Into<String>) -> Self {
    let text = text.into();

    self.derive_text(move || text.clone())
  }

  /// The text, kept current.
  fn derive_text<S: Into<String>>(self, text: impl Fn() -> S + 'static) -> Self {
    self.map_inner(|inner| {
      inner
        .map_entry(|control| control.map_behavior(|entry| entry.derive_text(move || text().into())))
    })
  }

  /// Runs `change` with the whole text each edit proposes, before it shows.
  ///
  /// Where the truth then is that text the platform applies the edit itself (caret, undo and
  /// the input method intact); otherwise the truth shows, as the smallest change, the caret
  /// mapped through it.
  fn handle_change(self, change: impl Fn(String) + 'static) -> Self {
    self.map_inner(|inner| {
      inner.map_entry(|control| control.map_behavior(|entry| entry.handle_change(change)))
    })
  }

  /// Shows `text` and writes each edit into it.
  fn bind_text(self, text: Signal<String>) -> Self {
    self
      .derive_text(move || text.get())
      .handle_change(move |value| text.set(value))
  }

  /// The selection, in grapheme clusters of the text, kept current: the selection is then
  /// controlled, each move of it reported to `handle_selection_change`.
  fn derive_selection(self, selection: impl Fn() -> TextSelection + 'static) -> Self {
    self.map_inner(|inner| {
      inner.map_entry(|control| control.map_behavior(|entry| entry.derive_selection(selection)))
    })
  }

  /// Runs `select` with each move of the caret or the selection, in grapheme clusters, where
  /// the selection is derived.
  fn handle_selection_change(self, select: impl Fn(TextSelection) + 'static) -> Self {
    self.map_inner(|inner| {
      inner.map_entry(|control| control.map_behavior(|entry| entry.handle_selection_change(select)))
    })
  }

  /// Places the selection at `selection` and writes each move into it.
  fn bind_selection(self, selection: Signal<TextSelection>) -> Self {
    self
      .derive_selection(move || selection.get())
      .handle_selection_change(move |moved| selection.set(moved))
  }

  /// Whether the text can be selected and copied but not changed.
  fn set_read_only(self, read_only: bool) -> Self {
    self.derive_read_only(move || read_only)
  }

  /// Whether the text can be selected and copied but not changed, kept current.
  fn derive_read_only(self, read_only: impl Fn() -> bool + 'static) -> Self {
    self.map_inner(|inner| {
      inner.map_entry(|control| control.map_behavior(|entry| entry.derive_read_only(read_only)))
    })
  }
}

/// The defaults every text entry's box starts from: 34 pixels tall at least and 60 wide, its
/// corners rounded by 8, padded 6 above and below and 9 at the sides inside a border of 1 in
/// [`SystemColors::BORDER`] (the accent while focused, with a line of 1 in the accent just
/// inside it), filled with [`SystemColors::FIELD`], its text in [`SystemColors::INK`], its
/// placeholder in [`SystemColors::MUTED_INK`], its selection in [`SystemColors::SELECTION`]
/// (and [`SystemColors::SELECTION_INACTIVE`] without focus).
fn entry<R: Realization>(lines: u16) -> Control<TextEntry, R> {
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

/// Builds `control` in `cx` inside `menu`; alone where the menu has no entry.
fn with_menu<R: Realization>(
  control: Control<TextEntry, R>,
  menu: ContextMenu<R>,
  cx: &mut Cx<R>,
) -> Node {
  if menu.is_empty() {
    return control.build(cx);
  }

  menu.set_child(control).build(cx)
}

/// A one-line text entry: Skene's text field, drawn on every platform. It is a [`Control`] over
/// a one-line [`TextEntry`], the platform's own text view whose box the style decorates: its
/// input methods, its clipboard and its selection gestures are the platform's.
///
/// Its caption sits before it among its siblings, a press on it focusing the field. It is
/// controlled: it shows what [`EditingBase::derive_text`] returns, reports each edit to
/// [`EditingBase::handle_change`] as the whole text it proposes, and shows the truth; its
/// selection is controlled the same way where it is derived ([`EditingBase::bind_selection`]),
/// in grapheme clusters. It offers the standard context menu ([`ContextMenuBase::standard`]),
/// GTK's entry's editing entries, unless set otherwise ([`EditingBase::set_context_menu`]).
#[must_use]
pub struct TextField<R: Realization> {
  /// The control it composes, its defaults set.
  control: Control<TextEntry, R>,
  /// Its context menu.
  menu: ContextMenu<R>,
}

wraps_itself!(TextField<R> where R: Realization);

control_surfaces!([R: Realization] TextField<R>, R => control);

impl<R: Realization> HasEntry<R> for TextField<R> {
  fn map_entry(mut self, map: impl FnOnce(Control<TextEntry, R>) -> Control<TextEntry, R>) -> Self {
    self.control = map(self.control);

    self
  }

  fn menu_mut(&mut self) -> &mut ContextMenu<R> {
    &mut self.menu
  }
}

impl<R: Realization> EditingBase<R> for TextField<R> {}

/// What a [`TextField`] carries beside every text entry's members: its placeholder, whether it
/// hides what is typed, its keyboard and its submission.
pub trait TextFieldBase<R: Realization>: Wraps<Inner = TextField<R>> {
  /// An empty text field labelled `label`.
  fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self::wrap(TextField {
      control: entry(1).set_label(label),
      menu: ContextMenu::standard(),
    })
  }

  /// The text shown while the field is empty.
  fn set_placeholder(self, placeholder: impl Into<Rc<str>>) -> Self {
    let placeholder = placeholder.into();

    self.derive_placeholder(move || Rc::clone(&placeholder))
  }

  /// The text shown while the field is empty, kept current.
  fn derive_placeholder<S: Into<Rc<str>>>(self, placeholder: impl Fn() -> S + 'static) -> Self {
    self.map_inner(|field| {
      field.map_entry(|control| {
        control.map_behavior(|entry| entry.derive_placeholder(move || placeholder().into()))
      })
    })
  }

  /// Whether what is typed is hidden: a password.
  fn set_secure(self, secure: bool) -> Self {
    self.derive_secure(move || secure)
  }

  /// Whether what is typed is hidden, kept current.
  fn derive_secure(self, secure: impl Fn() -> bool + 'static) -> Self {
    self.map_inner(|field| {
      field.map_entry(|control| control.map_behavior(|entry| entry.derive_secure(secure)))
    })
  }

  /// The keyboard the field asks the platform's input for.
  fn set_keyboard(self, keyboard: Keyboard) -> Self {
    self.derive_keyboard(move || keyboard)
  }

  /// The keyboard the field asks the platform's input for, kept current.
  fn derive_keyboard(self, keyboard: impl Fn() -> Keyboard + 'static) -> Self {
    self.map_inner(|field| {
      field.map_entry(|control| {
        control.map_behavior(|entry| entry.derive_keyboard(move || Rc::from(keyboard().name())))
      })
    })
  }

  /// Runs `activate` when the reader submits the field (`Enter`).
  fn handle_activate(self, activate: impl Fn() + 'static) -> Self {
    self.map_inner(|field| {
      field.map_entry(|control| control.map_behavior(|entry| entry.handle_activate(activate)))
    })
  }
}

impl<R: Realization> TextFieldBase<R> for TextField<R> {}

impl<R: Realization> AnyKind for TextField<R> {}

impl<R: Realization> View<R> for TextField<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    with_menu(self.control, self.menu, cx)
  }
}

/// A text entry of several lines: Skene's text editor, drawn on every platform. It is a
/// [`Control`] over a [`TextEntry`] of several lines, the platform's own text view, 96 pixels
/// tall at least; past its greatest height (`set_max_height`) it scrolls itself, as a
/// `<textarea>` does, keeping its caret in view.
///
/// It is controlled as a [`TextField`] is, and offers the same context menu.
#[must_use]
pub struct TextEditor<R: Realization> {
  /// The control it composes, its defaults set.
  control: Control<TextEntry, R>,
  /// Its context menu.
  menu: ContextMenu<R>,
}

wraps_itself!(TextEditor<R> where R: Realization);

control_surfaces!([R: Realization] TextEditor<R>, R => control);

impl<R: Realization> HasEntry<R> for TextEditor<R> {
  fn map_entry(mut self, map: impl FnOnce(Control<TextEntry, R>) -> Control<TextEntry, R>) -> Self {
    self.control = map(self.control);

    self
  }

  fn menu_mut(&mut self) -> &mut ContextMenu<R> {
    &mut self.menu
  }
}

impl<R: Realization> EditingBase<R> for TextEditor<R> {}

/// What a [`TextEditor`] carries beside every text entry's members: its constructor.
pub trait TextEditorBase<R: Realization>: Wraps<Inner = TextEditor<R>> {
  /// An empty text editor labelled `label`.
  fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self::wrap(TextEditor {
      control: entry(4).set_label(label),
      menu: ContextMenu::standard(),
    })
  }
}

impl<R: Realization> TextEditorBase<R> for TextEditor<R> {}

impl<R: Realization> AnyKind for TextEditor<R> {}

impl<R: Realization> View<R> for TextEditor<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    with_menu(self.control, self.menu, cx)
  }
}
