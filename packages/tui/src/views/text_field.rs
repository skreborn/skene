//! A text field and a text editor, drawn with the platform's colors.

use skene_foundation::{
  AnyKind,
  Condition,
  ControlBase,
  DeclaredBoxStyle,
  DecorationBase,
  HasBoxStyle,
  HasControl,
  HasLayoutBox,
  Inset,
  LayoutBox,
  MotionBase,
  MouseCursor,
  NodeId,
  PlainChild,
  SelectionColors,
  State,
  SystemColors,
  TextEntry,
  TextEntryBase,
  View,
  When,
};

use crate::Active;

/// The platform's colors in cells.
type Colors = SystemColors<i32>;

/// A single-line text field: Skene's, drawn in cells, a [`Control`](crate::Control) over
/// [`TextEntry`](crate::behavior::TextEntry), its window over the editing state the scene
/// keeps for it.
///
/// Editing is the core's, through the terminal's keys and its clipboard. Controlled: its text is
/// what [`derive_value`](TextEntryBase::derive_value) reads, and an edit only calls
/// [`handle_change`](TextEntryBase::handle_change) with the text it asks for;
/// [`bind_value`](TextEntryBase::bind_value) wires both to one signal. Its selection is
/// controlled beside it, in grapheme clusters
/// ([`derive_selection`](TextEntryBase::derive_selection),
/// [`handle_selection_change`](TextEntryBase::handle_selection_change),
/// [`bind_selection`](TextEntryBase::bind_selection)). A
/// [secure](Self::set_secure) field masks its text and keeps it from the clipboard.
///
/// Its caption ([`set_label`](ControlBase::set_label)) sits before it among its
/// siblings. Its defaults, each a value its setters replace: a cell of padding at either
/// side inside a border of one cell in [`SystemColors::BORDER`], the accent while focus
/// shows, filled with [`SystemColors::FIELD`] and
/// [`SystemColors::CONTROL`] while disabled; its value selected on
/// [`SystemColors::SELECTION`] in [`SystemColors::INK`]
/// ([`set_selection_background`](TextEntryBase::set_selection_background),
/// [`set_selection_color`](TextEntryBase::set_selection_color)); the text cursor, the
/// default one while disabled.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let name = Signal::new(String::new());
/// let field = TextField::new()
///   .set_label("Name")
///   .set_placeholder("Ada")
///   .bind_value(name);
/// ```
#[derive(Debug)]
#[must_use]
pub struct TextField {
  control: skene_foundation::Control<Active, TextEntry>,
  selection: SelectionColors<i32>,
}

impl Default for TextField {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl TextField {
  /// An empty field.
  pub fn new() -> Self {
    Self {
      control: skene_foundation::Control::new(TextEntry::new()),
      selection: SelectionColors::new(),
    }
  }

  /// Makes the field secure where `secure`: its text masked, and kept from the
  /// clipboard.
  pub fn set_secure(self, secure: bool) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.set_secure(secure)))
  }

  /// Derives whether the field is secure from the signals `secure` reads.
  pub fn derive_secure(self, secure: impl Fn() -> bool + 'static) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.derive_secure(secure)))
  }
}

/// A multi-line text editor: Skene's, drawn in cells, a [`Control`](crate::Control) over
/// [`TextEntry`](crate::behavior::TextEntry) showing several lines at once, three unless [told
/// otherwise](Self::set_lines).
///
/// It is controlled as a [`TextField`] is, and takes the same defaults.
///
/// ```
/// use skene_tui::prelude::*;
///
/// let notes = Signal::new(String::new());
/// let editor = TextEditor::new()
///   .set_label("Notes")
///   .set_lines(4)
///   .bind_value(notes);
/// ```
#[derive(Debug)]
#[must_use]
pub struct TextEditor {
  control: skene_foundation::Control<Active, TextEntry>,
  selection: SelectionColors<i32>,
}

impl Default for TextEditor {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl TextEditor {
  /// The lines an editor shows unless told otherwise.
  pub const LINES: u16 = 3;

  /// An empty editor showing [`LINES`](Self::LINES) lines.
  pub fn new() -> Self {
    Self {
      control: skene_foundation::Control::new(TextEntry::new().set_lines(Self::LINES)),
      selection: SelectionColors::new(),
    }
  }

  /// Sets the lines the editor shows at once, two at least.
  pub fn set_lines(self, lines: u16) -> Self {
    self.map_control(|control| control.map_behavior(|entry| entry.set_lines(lines.max(2))))
  }
}

/// The defaults both entries take: the field's chrome and its selection colors.
fn defaults() -> (
  skene_foundation::Control<Active, TextEntry>,
  SelectionColors<i32>,
) {
  let control = skene_foundation::Control::new(TextEntry::new())
    .set_padding(Inset::symmetric(1, 0))
    .set_border_width(Inset::all(1))
    .set_border_color(Colors::BORDER.when(Condition::from(State::FocusVisible), Colors::ACCENT))
    .set_background(Colors::FIELD.when(State::Disabled, Colors::CONTROL))
    .set_cursor(MouseCursor::Text.when(State::Disabled, MouseCursor::Default));
  let selection = SelectionColors {
    color: Some(Colors::INK.into()),
    background: Some(Colors::SELECTION.into()),
  };

  (control, selection)
}

/// Builds an entry over the defaults, its selection colors defined at its node.
fn build(
  control: skene_foundation::Control<Active, TextEntry>,
  selection: SelectionColors<i32>,
) -> NodeId {
  let (chrome, colors) = defaults();
  let node = control.over(chrome).build();

  selection.over(colors).define::<Active>(node);

  node
}

macro_rules! entry_view {
  ($view:ident) => {
    impl HasControl<Active, TextEntry> for $view {
      #[inline(always)]
      fn control(&self) -> &skene_foundation::Control<Active, TextEntry> {
        &self.control
      }

      #[inline(always)]
      fn map_control(
        mut self,
        map: impl FnOnce(
          skene_foundation::Control<Active, TextEntry>,
        ) -> skene_foundation::Control<Active, TextEntry>,
      ) -> Self {
        self.control = map(self.control);

        self
      }
    }

    impl HasBoxStyle<Active> for $view {
      #[inline(always)]
      fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<Active> {
        self.control.box_style_mut()
      }
    }

    impl HasLayoutBox<Active> for $view {
      type Kind = PlainChild;

      #[inline(always)]
      fn map_layout_box(
        self,
        map: impl FnOnce(LayoutBox<Active, PlainChild>) -> LayoutBox<Active, PlainChild>,
      ) -> Self {
        self.map_control(|control| control.map_layout_box(map))
      }
    }

    impl TextEntryBase<Active> for $view {
      #[inline(always)]
      fn selection_colors_mut(&mut self) -> &mut SelectionColors<i32> {
        &mut self.selection
      }
    }

    impl ControlBase<Active, TextEntry> for $view {}

    impl DecorationBase<Active> for $view {}

    impl MotionBase<Active> for $view {}

    impl AnyKind for $view {}

    impl $view {
      crate::views::terminal_decoration!(box_style_mut);
    }

    impl View<Active> for $view {
      fn build(self) -> NodeId {
        build(self.control, self.selection)
      }
    }
  };
}

entry_view!(TextField);
entry_view!(TextEditor);
