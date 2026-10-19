//! The built-in controls: each a `Control` over a shipped behavior, composed of Skene's views
//! and styled by ordinary values, so every default is a value its user overrides and every
//! part a view its user replaces (`set_<part>`), starting from the default the control hands
//! back (`default_<part>()`).
//!
//! Colors are `SystemColors` tokens, which the platform defines at the root and any `Theme`
//! beneath redefines. No default changes a layout property with a state or a value: a press, a
//! check, a flip or a drag changes paint alone, so a change costs no layout.

pub use skene_foundation::view::{Keyboard, MenuEntry, controls::marks};
use skene_foundation::{
  Color, Fill, Value,
  view::{
    self as base, ButtonBase, CheckboxBase, ContextMenuBase, ControlMembersBase, DecorationBase,
    EditingBase, MenuItemBase, MotionBase, PickerBase, PickerItemBase, RadioBase, RadioGroupBase,
    RunBase, RunWithEntries, SliderBase, TextEditorBase, TextFieldBase, ToggleBase, Wraps,
  },
  wraps,
};

use crate::{Active, Container, Painted};

/// A command: Skene's button, drawn on every platform. `Enter` activates it at once, `Space`
/// at its release, a click on its release over it.
///
/// Its label is its content and its name (`Button::new`, `Button::derive`); a derived label
/// changes in place. Its defaults: a row 34 pixels tall at least and as wide, padded 7 above
/// and below and 14 at the sides, its label centered, its corners rounded by 8, filled with
/// [`Button::default_background`] in [`Button::default_color`], and a ring of 2 in the accent
/// while focus shows.
///
/// ```
/// use skene_gui::prelude::*;
///
/// Button::new("Save").set_prominent(true).handle_activate(|| {});
/// ```
#[must_use]
pub struct Button(base::Button<Active>);

wraps!([] Button => base::Button<Active>, Active);

impl ButtonBase<Active> for Button {}

impl ControlMembersBase<Active> for Button {}

impl DecorationBase for Button {}

impl RunBase for Button {}

impl RunWithEntries for Button {}

impl MotionBase for Button {}

impl Button {
  /// The default text color: the ink, the color on the accent where `prominent`.
  pub fn default_color(prominent: bool) -> Value<Color> {
    base::Button::<Active>::default_color(prominent)
  }

  /// The default fill: the control color and its hover and pressed variants, the accent's
  /// where `prominent`; the resting one while disabled.
  pub fn default_background(prominent: bool) -> Value<Fill> {
    base::Button::<Active>::default_background(prominent)
  }
}

/// A checkbox with its caption: Skene's checkbox, drawn on every platform. `Space` flips it at
/// its release, a click anywhere on it, its caption included; a mixed checkbox turns on when
/// flipped.
///
/// It is controlled: it shows what its `derive_checked` returns, asks its `handle_change` for
/// each flip, and shows the truth. Its parts: its face ([`Checkbox::default_face`]) holding
/// its mark ([`Checkbox::default_mark`]), then its caption, 8 pixels after. The face follows
/// the control's states, its hover included, so hovering the caption highlights it.
#[must_use]
pub struct Checkbox(base::Checkbox<Active>);

wraps!([] Checkbox => base::Checkbox<Active>, Active);

impl CheckboxBase<Active> for Checkbox {}

impl ControlMembersBase<Active> for Checkbox {}

impl DecorationBase for Checkbox {}

impl RunBase for Checkbox {}

impl RunWithEntries for Checkbox {}

impl MotionBase for Checkbox {}

impl Checkbox {
  /// The default face: 18 pixels square, its corners rounded by 4, a border of 1.5 around the
  /// field color and its hover and pressed variants, the accent while checked or mixed, its
  /// mark centered and inked in the color on the accent; a ring of 2 in the accent while focus
  /// shows.
  pub fn default_face() -> Container {
    Container::wrap(base::Checkbox::<Active>::default_face())
  }

  /// The default mark: [`marks::check`], filling the face.
  pub fn default_mark() -> Painted {
    Painted::wrap(base::Checkbox::<Active>::default_mark())
  }
}

/// An on/off control with its caption: Skene's switch, drawn on every platform. `Space` flips
/// it at its release, a click anywhere on it, its caption included.
///
/// It is controlled: it shows what its `derive_on` returns, asks its `handle_change` for each
/// flip, and shows the truth. Its parts: the caption first, taking the room the switch leaves,
/// 12 pixels before its track ([`Toggle::default_track`]), which holds its thumb
/// ([`Toggle::default_thumb`]). The thumb moves by its own width while on, by paint alone.
///
/// ```
/// use skene_gui::prelude::*;
///
/// let sync = Signal::new(true);
///
/// Toggle::new("Sync").bind_on(sync);
/// ```
#[must_use]
pub struct Toggle(base::Toggle<Active>);

wraps!([] Toggle => base::Toggle<Active>, Active);

impl ToggleBase<Active> for Toggle {}

impl ControlMembersBase<Active> for Toggle {}

impl DecorationBase for Toggle {}

impl RunBase for Toggle {}

impl RunWithEntries for Toggle {}

impl MotionBase for Toggle {}

impl Toggle {
  /// The default track: 44 by 24 pixels, its corners rounded by 12, padded 2, its thumb at its
  /// leading end; filled with the track color, its hover and pressed variants, and the accent
  /// while on; a ring of 2 in the accent while focus shows.
  pub fn default_track() -> Container {
    Container::wrap(base::Toggle::<Active>::default_track())
  }

  /// The default thumb: a circle 20 pixels across in the knob color, moved by its own width
  /// while on.
  pub fn default_thumb() -> Container {
    Container::wrap(base::Toggle::<Active>::default_thumb())
  }
}

/// A value along a range, with its caption: Skene's slider, drawn on every platform. The
/// arrows step it, the page keys move it by a tenth, `Home` and `End` take it to its ends, a
/// press jumps to where it lands and a drag follows the pointer.
///
/// It is controlled: it shows what its `derive_value` returns, asks its `handle_change` for
/// each move, and shows the truth. Its caption sits before it among its siblings, a press on it
/// focusing the slider. Its parts: a track ([`Slider::default_track`]) clipping its fill
/// ([`Slider::default_fill`]), and a thumb ([`Slider::default_thumb`]) centered on the value.
/// It is 200 pixels wide unless its layout stretches or shrinks it, 80 at least, and 24 tall,
/// padded 9 at the sides; it takes a press on what it draws: its box, and its thumb where the
/// thumb overflows it.
#[must_use]
pub struct Slider(base::Slider<Active>);

wraps!([] Slider => base::Slider<Active>, Active);

impl SliderBase<Active> for Slider {}

impl ControlMembersBase<Active> for Slider {}

impl DecorationBase for Slider {}

impl RunBase for Slider {}

impl RunWithEntries for Slider {}

impl MotionBase for Slider {}

impl Slider {
  /// The default track: the slider's width, 4 pixels tall, its corners rounded by 2, in the
  /// track color, clipping the fill.
  pub fn default_track() -> Container {
    Container::wrap(base::Slider::<Active>::default_track())
  }

  /// The default fill: as large as the track, its corners rounded by 2, in the accent.
  pub fn default_fill() -> Container {
    Container::wrap(base::Slider::<Active>::default_fill())
  }

  /// The default thumb: a circle 18 pixels across in the knob color, its center on the value,
  /// edged by half a pixel of the border color, a ring of 2 in the accent while focus shows.
  pub fn default_thumb() -> Container {
    Container::wrap(base::Slider::<Active>::default_thumb())
  }
}

/// One option of a [`RadioGroup`], as its option builder makes it: a radio with its caption.
///
/// Its parts: its face ([`Radio::default_face`]) holding its mark ([`Radio::default_mark`]),
/// then its caption, 8 pixels after. Every member of a control applies to it, so one option
/// can be disabled.
#[must_use]
pub struct Radio(base::Radio<Active>);

wraps!(wrapper [] Radio => base::Radio<Active>);

impl RadioBase<Active> for Radio {}

impl ControlMembersBase<Active> for Radio {}

impl DecorationBase for Radio {}

impl RunBase for Radio {}

impl RunWithEntries for Radio {}

impl MotionBase for Radio {}

impl Radio {
  /// The default face: a circle 18 pixels across, a border of 1.5 around the field color and
  /// its hover and pressed variants, the accent while chosen, its mark centered and inked in
  /// the color on the accent; a ring of 2 in the accent while focus shows.
  pub fn default_face() -> Container {
    Container::wrap(base::Radio::<Active>::default_face())
  }

  /// The default mark: [`marks::dot`], filling the face.
  pub fn default_mark() -> Painted {
    Painted::wrap(base::Radio::<Active>::default_mark())
  }
}

/// One choice of a few, all shown, with its caption: Skene's radio group, drawn on every
/// platform: its caption above a column of radios 6 pixels apart, each from its option
/// builder.
///
/// The group is one tab stop, on its chosen radio (its first enabled where none is); the
/// arrows move to the next or the previous enabled radio, wrapping, and choose it; `Space` and
/// a click choose. A disabled group disables its radios. It is controlled: it shows what its
/// `derive_chosen` returns, asks its `handle_choose` for each choice, and shows the truth.
#[must_use]
pub struct RadioGroup(base::RadioGroup<Active>);

wraps!([] RadioGroup => base::RadioGroup<Active>, Active);

impl RadioGroupBase<Active> for RadioGroup {}

impl ControlMembersBase<Active> for RadioGroup {}

impl DecorationBase for RadioGroup {}

impl RunBase for RadioGroup {}

impl RunWithEntries for RadioGroup {}

impl MotionBase for RadioGroup {}

/// A one-line text entry: Skene's text field, drawn on every platform, the platform's own text
/// view whose box the style decorates: its input methods, its clipboard and its selection
/// gestures are the platform's.
///
/// Its caption sits before it among its siblings, a press on it focusing the field. It is
/// controlled: it shows what its `derive_text` returns, reports each edit to its
/// `handle_change` as the whole text it proposes, and shows the truth; its selection is
/// controlled the same way where it is derived, in grapheme clusters. It offers the standard
/// context menu, GTK's entry's editing entries, unless set otherwise (`set_context_menu`).
///
/// ```
/// use skene_gui::prelude::*;
///
/// let name = Signal::new(String::new());
///
/// TextField::new("Name").set_placeholder("Ada").bind_text(name);
/// ```
#[must_use]
pub struct TextField(base::TextField<Active>);

wraps!([] TextField => base::TextField<Active>, Active);

impl TextFieldBase<Active> for TextField {}

impl EditingBase<Active> for TextField {}

impl ControlMembersBase<Active> for TextField {}

impl DecorationBase for TextField {}

impl RunBase for TextField {}

impl RunWithEntries for TextField {}

impl MotionBase for TextField {}

/// A text entry of several lines: Skene's text editor, drawn on every platform, the platform's
/// own text view, 96 pixels tall at least; past its greatest height it scrolls itself, as a
/// `<textarea>` does, keeping its caret in view.
///
/// It is controlled as a [`TextField`] is, and offers the same context menu.
#[must_use]
pub struct TextEditor(base::TextEditor<Active>);

wraps!([] TextEditor => base::TextEditor<Active>, Active);

impl TextEditorBase<Active> for TextEditor {}

impl EditingBase<Active> for TextEditor {}

impl ControlMembersBase<Active> for TextEditor {}

impl DecorationBase for TextEditor {}

impl RunBase for TextEditor {}

impl RunWithEntries for TextEditor {}

impl MotionBase for TextEditor {}

/// One option of a [`Picker`]'s list, as its item builder makes it: its text, taking the room
/// its row leaves, then its mark ([`PickerItem::default_mark`]). Its text is its name.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 before its text and 6 after its
/// mark, its corners rounded by 6, filled with the control's hover color while focus is on it
/// (where the keys and the pointer move it).
#[must_use]
pub struct PickerItem(base::PickerItem<Active>);

wraps!(wrapper [] PickerItem => base::PickerItem<Active>);

impl PickerItemBase<Active> for PickerItem {}

impl ControlMembersBase<Active> for PickerItem {}

impl DecorationBase for PickerItem {}

impl RunBase for PickerItem {}

impl RunWithEntries for PickerItem {}

impl MotionBase for PickerItem {}

impl PickerItem {
  /// The default mark: [`marks::check_mark`], 14 by 12 pixels, in the accent.
  pub fn default_mark() -> Painted {
    Painted::wrap(base::PickerItem::<Active>::default_mark())
  }
}

/// One choice of a list, shown closed, with its caption: Skene's picker, drawn on every
/// platform. Its options are real controls in a list box in an `Overlay` below it, each from
/// its item builder.
///
/// Closed, `Space`, `Alt` with an arrow down and `F4` open it, the arrows choose without
/// opening, and typing finds an option by its name. Open, focus is on its options: the arrows
/// move it, `Enter`, `Space` and a click choose and close, `Escape` closes, `Tab` chooses and
/// moves on. It is controlled: it shows what its `derive_chosen` returns, asks its
/// `handle_choose` for each choice, and shows the truth.
///
/// Its caption sits before it among its siblings, a press on it focusing the picker. Its face:
/// 34 pixels tall at least, padded 7 above and below and 14 at the sides, its corners rounded
/// by 8, filled with the control color and its hover and pressed variants (pressed while
/// open), the chosen option's text taking the room the face leaves, as wide as the widest
/// option's, then its mark ([`Picker::default_mark`]); a ring of 2 in the accent while focus
/// shows. Its list: [`Picker::default_list`].
#[must_use]
pub struct Picker(base::Picker<Active>);

wraps!([] Picker => base::Picker<Active>, Active);

impl PickerBase<Active> for Picker {}

impl ControlMembersBase<Active> for Picker {}

impl DecorationBase for Picker {}

impl RunBase for Picker {}

impl RunWithEntries for Picker {}

impl MotionBase for Picker {}

impl Picker {
  /// The default mark: [`marks::chevron`], 12 by 8 pixels.
  pub fn default_mark() -> Painted {
    Painted::wrap(base::Picker::<Active>::default_mark())
  }

  /// The default list: the field color inside a border of 1, its corners rounded by 8, padded
  /// 4.
  pub fn default_list() -> Container {
    Container::wrap(base::Picker::<Active>::default_list())
  }
}

/// One item of a [`ContextMenu`]'s list, as its item builder makes it from its entry.
///
/// Its defaults: a row 30 pixels tall at least, padded 12 at its sides, its corners rounded by
/// 6, filled with the control's hover color while focus is on it (where the keys and the
/// pointer move it).
#[must_use]
pub struct MenuItem(base::MenuItem<Active>);

wraps!(wrapper [] MenuItem => base::MenuItem<Active>);

impl MenuItemBase<Active> for MenuItem {}

impl ControlMembersBase<Active> for MenuItem {}

impl DecorationBase for MenuItem {}

impl RunBase for MenuItem {}

impl RunWithEntries for MenuItem {}

impl MotionBase for MenuItem {}

/// A context menu around one view: a secondary press, `Shift-F10` or the menu key on the view
/// opens its entries in a menu hung from the pointer (from the focus's middle by keyboard),
/// the deepest menu along the path taking it, and keeps the platform's own from showing. A
/// menu whose every entry is absent there opens nothing and lets the request climb.
///
/// Open, focus is on its first operable item: the arrows move it, wrapping, `Enter`, `Space`
/// and a click run the item and close the menu, `Escape`, `Tab` and a press outside it close
/// it; focus then returns to the view, whose selection the menu kept. Its entries and the
/// view it wraps are its content, so they end its chain.
///
/// ```
/// use skene_gui::prelude::*;
///
/// ContextMenu::new()
///   .add_entry(MenuEntry::new("Refresh").handle_activate(|| {}))
///   .add_entry(MenuEntry::copy())
///   .set_child(Text::new("Inbox"));
/// ```
#[must_use]
pub struct ContextMenu(base::ContextMenu<Active>);

wraps!([] ContextMenu => base::ContextMenu<Active>, Active);

impl ContextMenuBase<Active> for ContextMenu {}

impl Default for ContextMenu {
  fn default() -> Self {
    Self::new()
  }
}

impl ContextMenu {
  /// The default list: the field color inside a border of 1, its corners rounded by 8, padded
  /// 4.
  pub fn default_list() -> Container {
    Container::wrap(base::ContextMenu::<Active>::default_list())
  }
}

#[cfg(test)]
mod tests;
