//! The cross-platform controls.
//!
//! Each keeps its own fields and, in `build`, constructs its control node (`crate::nodes`), in
//! one `cfg`'d block per platform; a build for a platform a control has no block for fails to
//! compile. Only Linux has a block in this prototype, where Skene draws the control.
//!
//! What a control composes around its node (a caption beside it, where it sits, the
//! space between them) is a default its user overrides (`set_caption_placement`,
//! `set_spacing`), never fixed. The caption is a [`crate::Text`] inheriting the run properties
//! of the boxes around the control, so a [`crate::Container`] around it styles it.

use std::rc::Rc;

use crate::{
  Alignment, AxisAlignment, Dimension, GridPlacement, GridTrack, Shortcut, Signal, Spacing, Tracks,
  Value,
  access::{Access, access},
  editing::{Editing, editing, enabled},
  view::{Cx, Node, NodeHandle, View, handle},
};
#[cfg(skene_platform = "linux")]
use crate::{Grid, GridCell, Text, access::give_access, editing::give_editing, view::Apply as _};

/// Where a control's caption sits beside the control it names.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum CaptionPlacement {
  /// Above the control, as wide as it.
  #[default]
  Above,
  /// Below the control, as wide as it.
  Below,
  /// Before the control in its row, taking the room the control leaves.
  Leading,
  /// After the control in its row, taking the room the control leaves.
  Trailing,
}

impl skene_foundation::Property for CaptionPlacement {}

impl CaptionPlacement {
  /// The grid's columns.
  fn columns(self) -> Tracks {
    match self {
      Self::Above | Self::Below => Tracks::from([GridTrack::Flex(1.)]),
      Self::Leading => Tracks::from([GridTrack::Flex(1.), GridTrack::Auto]),
      Self::Trailing => Tracks::from([GridTrack::Auto, GridTrack::Flex(1.)]),
    }
  }

  /// The caption's cell and the control's.
  const fn cells(self) -> (GridPlacement, GridPlacement) {
    match self {
      Self::Above => (GridPlacement::at(1, 1), GridPlacement::at(2, 1)),
      Self::Below => (GridPlacement::at(2, 1), GridPlacement::at(1, 1)),
      Self::Leading => (GridPlacement::at(1, 1), GridPlacement::at(1, 2)),
      Self::Trailing => (GridPlacement::at(1, 2), GridPlacement::at(1, 1)),
    }
  }

  /// Where the caption sits in its cell: beside the control, centered across the row.
  const fn caption_alignment(self) -> Alignment {
    match self {
      Self::Above | Self::Below => Alignment::STRETCH,
      Self::Leading | Self::Trailing => {
        Alignment::new(AxisAlignment::Stretch, AxisAlignment::Center)
      }
    }
  }
}

/// A caption and a control, composed in a grid the placement and the spacing lay out: every
/// piece a public view, each of its properties as conditional and derived as any.
struct Captioned {
  placement: Value<CaptionPlacement>,
  spacing: Value<Spacing>,
}

#[cfg(skene_platform = "linux")]
impl Captioned {
  /// The grid holding `caption` (written into `handle`) and `control`.
  fn compose(self, caption: String, handle: &NodeHandle, control: impl View) -> Grid {
    let placement = self.placement;
    let columns = placement.clone().map(CaptionPlacement::columns);
    let (caption_cell, control_cell) = (
      placement.clone().map(|placement| placement.cells().0),
      placement.clone().map(|placement| placement.cells().1),
    );

    Grid::new()
      .set_columns(columns)
      .set_spacing(self.spacing)
      .add_child(
        GridCell::new()
          .set_cell(caption_cell)
          .set_alignment(placement.map(CaptionPlacement::caption_alignment))
          .set_child(Text::new(caption).set_handle(handle)),
      )
      .add_child(GridCell::new().set_cell(control_cell).set_child(control))
  }
}

/// A control's caption setters, writing its `captioned` field.
macro_rules! captioned {
  ($placement:literal, $spacing:literal) => {
    #[doc = concat!("Where the caption sits beside the control; ", $placement, " where unset.")]
    pub fn set_caption_placement(mut self, placement: impl Into<Value<CaptionPlacement>>) -> Self {
      self.captioned.placement = placement.into();

      self
    }

    #[doc = concat!("Where the caption sits beside the control, kept current; ", $placement, " where unset.")]
    pub fn derive_caption_placement<V: Into<Value<CaptionPlacement>>>(
      self,
      placement: impl Fn() -> V + 'static,
    ) -> Self {
      self.set_caption_placement(Value::derived(placement))
    }

    #[doc = concat!("The space between the caption and the control, in pixels; ", $spacing, " where unset.")]
    pub fn set_spacing(mut self, spacing: impl Into<Value<Spacing>>) -> Self {
      self.captioned.spacing = spacing.into();

      self
    }

    #[doc = concat!("The space between the caption and the control, in pixels, kept current; ", $spacing, " where unset.")]
    pub fn derive_spacing<V: Into<Value<Spacing>>>(self, spacing: impl Fn() -> V + 'static) -> Self {
      self.set_spacing(Value::derived(spacing))
    }
  };
}

/// A labelled on/off control: Skene's switch, drawn on every platform.
///
/// It is controlled: it shows what [`Toggle::derive_on`] returns, reports a flip to
/// [`Toggle::handle_change`], and is brought back to the truth after every report.
#[must_use]
pub struct Toggle {
  label: String,
  on: Box<dyn Fn() -> bool>,
  change: Option<Box<dyn Fn(bool)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  captioned: Captioned,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Toggle {
  /// A toggle labelled `label`, off.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      on: Box::new(|| false),
      change: None,
      enabled: None,
      captioned: Captioned {
        placement: Value::from(CaptionPlacement::Leading),
        spacing: Value::from(12),
      },
      handle: None,
      access: Access::default(),
    }
  }

  /// Whether the toggle is on.
  pub fn set_on(self, on: bool) -> Self {
    self.derive_on(move || on)
  }

  /// Whether the toggle is on, recomputed whenever a signal the closure reads changes.
  pub fn derive_on(mut self, on: impl Fn() -> bool + 'static) -> Self {
    self.on = Box::new(on);

    self
  }

  /// Runs `change` with the state the reader flipped the toggle to.
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.change = Some(Box::new(change));

    self
  }

  /// Shows `on` and writes each flip into it.
  pub fn bind_on(self, on: Signal<bool>) -> Self {
    self
      .derive_on(move || on.get())
      .handle_change(move |value| on.set(value))
  }

  enabled!();
  handle!();
  captioned!("leading", "12");
  access!();
}

impl View for Toggle {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let toggle = {
      use crate::nodes::Switch;

      let caption = NodeHandle::new();
      let switch = Switch::new()
        .derive_on(self.on)
        .apply(self.change, Switch::handle_change)
        .apply(self.enabled, |switch, enabled| {
          switch.derive_enabled(move || enabled())
        })
        .apply(self.handle, |switch, handle| switch.set_handle(&handle));
      let switch = give_access!(self.access.captioned(&caption), switch);

      self.captioned.compose(self.label, &caption, switch)
    };

    toggle.build(cx)
  }
}

/// A labelled checkbox: Skene's checkbox, drawn on every platform, its caption after the
/// mark where unset. A click on the caption flips it, as on the mark.
///
/// It is controlled: it shows what [`Checkbox::derive_checked`] returns, reports a flip to
/// [`Checkbox::handle_change`], and is brought back to the truth after every report. A mixed
/// checkbox ([`Checkbox::derive_mixed`]) turns on when flipped.
#[must_use]
pub struct Checkbox {
  label: String,
  checked: Box<dyn Fn() -> bool>,
  mixed: Option<Box<dyn Fn() -> bool>>,
  change: Option<Box<dyn Fn(bool)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  captioned: Captioned,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Checkbox {
  /// A checkbox labelled `label`, unchecked.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      checked: Box::new(|| false),
      mixed: None,
      change: None,
      enabled: None,
      captioned: Captioned {
        placement: Value::from(CaptionPlacement::Trailing),
        spacing: Value::from(8),
      },
      handle: None,
      access: Access::default(),
    }
  }

  /// Whether the checkbox is checked.
  pub fn set_checked(self, checked: bool) -> Self {
    self.derive_checked(move || checked)
  }

  /// Whether the checkbox is checked, recomputed whenever a signal the closure reads changes.
  pub fn derive_checked(mut self, checked: impl Fn() -> bool + 'static) -> Self {
    self.checked = Box::new(checked);

    self
  }

  /// Whether the checkbox is mixed: neither checked nor unchecked, drawn with a dash.
  pub fn set_mixed(self, mixed: bool) -> Self {
    self.derive_mixed(move || mixed)
  }

  /// Whether the checkbox is mixed, recomputed whenever a signal the closure reads changes.
  pub fn derive_mixed(mut self, mixed: impl Fn() -> bool + 'static) -> Self {
    self.mixed = Some(Box::new(mixed));

    self
  }

  /// Runs `change` with the state the reader flipped the checkbox to.
  pub fn handle_change(mut self, change: impl Fn(bool) + 'static) -> Self {
    self.change = Some(Box::new(change));

    self
  }

  /// Shows `checked` and writes each flip into it.
  pub fn bind_checked(self, checked: Signal<bool>) -> Self {
    self
      .derive_checked(move || checked.get())
      .handle_change(move |value| checked.set(value))
  }

  enabled!();
  handle!();
  captioned!("trailing", "8");
  access!();
}

impl View for Checkbox {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let checkbox = {
      use crate::nodes::Checkbox;

      let caption = NodeHandle::new();
      let mark = Checkbox::new()
        .derive_on(self.checked)
        .apply(self.mixed, |mark, mixed| mark.derive_mixed(mixed))
        .apply(self.change, Checkbox::handle_change)
        .apply(self.enabled, |mark, enabled| {
          mark.derive_enabled(move || enabled())
        })
        .apply(self.handle, |mark, handle| mark.set_handle(&handle));
      let mark = give_access!(self.access.captioned(&caption), mark);

      self.captioned.compose(self.label, &caption, mark)
    };

    checkbox.build(cx)
  }
}

/// One choice of a few, all shown, with its caption: Skene's radio group, drawn on every
/// platform. Each option is a radio with its own caption after it; a click on an option's
/// caption chooses it, as on its mark.
///
/// Focus rests on the chosen radio (the first where none is chosen); the arrows move it to
/// the next or the previous option, wrapping, and choose it.
///
/// It is controlled: it shows what [`RadioGroup::derive_chosen`] returns, reports each choice
/// to [`RadioGroup::handle_choose`], and is brought back to the truth after every report.
#[must_use]
pub struct RadioGroup {
  label: String,
  options: Rc<dyn Fn() -> Vec<String>>,
  chosen: Rc<dyn Fn() -> Option<usize>>,
  choose: Option<Rc<dyn Fn(Option<usize>)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  option_spacing: Value<Spacing>,
  captioned: Captioned,
  handle: Option<NodeHandle>,
  access: Access,
}

impl RadioGroup {
  /// A radio group labelled `label`, with no options.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      options: Rc::new(Vec::new),
      chosen: Rc::new(|| None),
      choose: None,
      enabled: None,
      option_spacing: Value::from(6),
      captioned: Captioned {
        placement: Value::from(CaptionPlacement::Above),
        spacing: Value::from(6),
      },
      handle: None,
      access: Access::default(),
    }
  }

  /// The options' texts.
  pub fn set_options<S: Into<String>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Vec<String> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.clone())
  }

  /// The options' texts, recomputed whenever a signal the closure reads changes: the radios
  /// are built again.
  pub fn derive_options<S: Into<String>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.options = Rc::new(move || options().into_iter().map(Into::into).collect());

    self
  }

  /// The chosen option's index, or none.
  pub fn set_chosen(self, chosen: Option<usize>) -> Self {
    self.derive_chosen(move || chosen)
  }

  /// The chosen option's index, or none, recomputed whenever a signal the closure reads
  /// changes.
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<usize> + 'static) -> Self {
    self.chosen = Rc::new(chosen);

    self
  }

  /// Runs `choose` with the option the reader chose.
  pub fn handle_choose(mut self, choose: impl Fn(Option<usize>) + 'static) -> Self {
    self.choose = Some(Rc::new(choose));

    self
  }

  /// Shows `chosen` and writes each choice into it.
  pub fn bind_chosen(self, chosen: Signal<Option<usize>>) -> Self {
    self
      .derive_chosen(move || chosen.get())
      .handle_choose(move |choice| chosen.set(choice))
  }

  /// The space between the options, in pixels; 6 where unset.
  pub fn set_option_spacing(mut self, spacing: impl Into<Value<Spacing>>) -> Self {
    self.option_spacing = spacing.into();

    self
  }

  /// The space between the options, in pixels, kept current; 6 where unset.
  pub fn derive_option_spacing<V: Into<Value<Spacing>>>(
    self,
    spacing: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_option_spacing(Value::derived(spacing))
  }

  enabled!();
  handle!();
  captioned!("above", "6");
  access!();
}

impl View for RadioGroup {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let group = {
      use crate::{
        Derived,
        nodes::{Group, Radio},
      };

      let caption = NodeHandle::new();
      let access = self.access.captioned(&caption);
      let Self {
        options,
        chosen,
        choose,
        enabled,
        option_spacing,
        handle,
        ..
      } = self;
      let group = Derived::new(move || {
        let group = Group::new(skene_foundation::seam::access::Role::RadioGroup)
          .set_spacing(option_spacing.clone());
        let mut group = give_access!(access.clone(), group);

        for (index, option) in options().into_iter().enumerate() {
          let option_caption = NodeHandle::new();
          let chosen = Rc::clone(&chosen);
          let mut radio = Radio::new()
            .derive_on(move || chosen() == Some(index))
            .set_labelled_by(&option_caption);

          if let Some(choose) = &choose {
            let choose = Rc::clone(choose);

            radio = radio.handle_change(move |on| {
              if on {
                choose(Some(index));
              }
            });
          }

          if let Some(enabled) = &enabled {
            let enabled = Rc::clone(enabled);

            radio = radio.derive_enabled(move || enabled());
          }

          if index == 0
            && let Some(handle) = &handle
          {
            radio = radio.set_handle(handle);
          }

          let row = Captioned {
            placement: Value::from(CaptionPlacement::Trailing),
            spacing: Value::from(8),
          }
          .compose(option, &option_caption, radio);

          group = group.add_child(row);
        }

        group
      });

      self.captioned.compose(self.label, &caption, group)
    };

    group.build(cx)
  }
}

/// A command: Skene's button, drawn on every platform.
///
/// Its label is what [`Button::new`] or [`Button::derive`] gives it; a derived label changes in
/// place, so the platform's button lives on.
#[must_use]
pub struct Button {
  label: Box<dyn Fn() -> String>,
  activate: Option<Box<dyn Fn()>>,
  prominent: Option<Rc<dyn Fn() -> bool>>,
  shortcut: Option<Shortcut>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Button {
  /// A button labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    let label = label.into();

    Self::derive(move || label.clone())
  }

  /// A button labelled what `label` returns, recomputed whenever a signal it reads changes.
  pub fn derive<S: Into<String>>(label: impl Fn() -> S + 'static) -> Self {
    Self {
      label: Box::new(move || label().into()),
      activate: None,
      prominent: None,
      shortcut: None,
      enabled: None,
      handle: None,
      access: Access::default(),
    }
  }

  /// Runs `activate` when the button is activated: clicked, tapped, or pressed from the
  /// keyboard.
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.activate = Some(Box::new(activate));

    self
  }

  /// Shows the button as the screen's primary action, each platform's way: on Linux, filled
  /// with the accent.
  pub fn set_prominent(self, prominent: bool) -> Self {
    self.derive_prominent(move || prominent)
  }

  /// Activates the button when `shortcut` (`Shortcut::primary('s')`: Control+S on Linux and
  /// Windows, Command+S on macOS) is pressed wherever focus is in its window, unless the
  /// focused control uses the key itself; a reader reads it as the button's key binding.
  pub const fn set_shortcut(mut self, shortcut: Shortcut) -> Self {
    self.shortcut = Some(shortcut);

    self
  }

  /// Shows the button as the screen's primary action, each platform's way (on Linux, filled
  /// with the accent), kept current.
  pub fn derive_prominent(mut self, prominent: impl Fn() -> bool + 'static) -> Self {
    self.prominent = Some(Rc::new(prominent));

    self
  }

  enabled!();
  handle!();
  access!();
}

impl View for Button {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let button = {
      use crate::nodes::Button;

      let button = Button::derive(self.label)
        .apply(self.prominent, |button, prominent| {
          button.derive_prominent(move || prominent())
        })
        .apply(self.activate, Button::handle_activate)
        .apply(self.shortcut, Button::set_shortcut)
        .apply(self.enabled, |button, enabled| {
          button.derive_enabled(move || enabled())
        })
        .apply(self.handle, |button, handle| button.set_handle(&handle));

      give_access!(self.access, button)
    };

    button.build(cx)
  }
}

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

/// A one-line text entry with its caption: Skene's text field, drawn on every platform.
///
/// It is controlled: it shows what [`TextField::derive_text`] returns, reports each edit to
/// [`TextField::handle_change`] as the whole text it proposes, and shows the truth. Its
/// selection is controlled the same way where it is derived ([`TextField::bind_selection`]),
/// in grapheme clusters.
#[must_use]
pub struct TextField {
  label: String,
  editing: Editing,
  placeholder: Option<Rc<dyn Fn() -> Rc<str>>>,
  secure: Option<Rc<dyn Fn() -> bool>>,
  keyboard: Option<Rc<dyn Fn() -> Keyboard>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  captioned: Captioned,
  handle: Option<NodeHandle>,
  access: Access,
}

impl TextField {
  /// An empty text field labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      editing: Editing::default(),
      placeholder: None,
      secure: None,
      keyboard: None,
      enabled: None,
      captioned: Captioned {
        placement: Value::from(CaptionPlacement::Above),
        spacing: Value::from(4),
      },
      handle: None,
      access: Access::default(),
    }
  }

  editing!(text: "", change: "", selection: "", read_only: "");

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
    self.placeholder = Some(Rc::new(move || placeholder().into()));

    self
  }

  /// Whether what is typed is hidden.
  pub fn set_secure(self, secure: bool) -> Self {
    self.derive_secure(move || secure)
  }

  /// Whether what is typed is hidden, kept current.
  pub fn derive_secure(mut self, secure: impl Fn() -> bool + 'static) -> Self {
    self.secure = Some(Rc::new(secure));

    self
  }

  /// The keyboard the field asks the platform's input for.
  pub fn set_keyboard(self, keyboard: Keyboard) -> Self {
    self.derive_keyboard(move || keyboard)
  }

  /// The keyboard the field asks the platform's input for, kept current.
  pub fn derive_keyboard(mut self, keyboard: impl Fn() -> Keyboard + 'static) -> Self {
    self.keyboard = Some(Rc::new(keyboard));

    self
  }

  /// Runs `activate` when the reader submits the field (`Enter`).
  pub fn handle_activate(mut self, activate: impl Fn() + 'static) -> Self {
    self.editing.activate = Some(Rc::new(activate));

    self
  }

  enabled!();
  handle!();
  captioned!("above", "4");
  access!();
}

impl View for TextField {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let field = {
      use crate::nodes::Entry;

      let caption = NodeHandle::new();
      let activate = self.editing.activate.clone();
      let entry = Entry::new()
        .apply(self.placeholder, |entry, placeholder| {
          entry.derive_placeholder(move || placeholder())
        })
        .apply(self.secure, |entry, secure| {
          entry.derive_secure(move || secure())
        })
        .apply(self.keyboard, |entry, keyboard| {
          entry.derive_keyboard(move || keyboard().name())
        })
        .apply(activate, |entry, activate| {
          entry.handle_activate(move || activate())
        })
        .apply(self.enabled, |entry, enabled| {
          entry.derive_enabled(move || enabled())
        })
        .apply(self.handle, |entry, handle| entry.set_handle(&handle));
      let entry = give_editing!(self.editing, entry);
      let entry = give_access!(self.access.captioned(&caption), entry);

      self.captioned.compose(self.label, &caption, entry)
    };

    field.build(cx)
  }
}

/// A multi-line text entry with its caption: Skene's text editor, drawn on every platform,
/// inside a scroll.
///
/// It is controlled as a [`TextField`] is.
#[must_use]
pub struct TextEditor {
  label: String,
  editing: Editing,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  min_height: Value<Dimension>,
  max_height: Option<Value<Dimension>>,
  captioned: Captioned,
  handle: Option<NodeHandle>,
  access: Access,
}

impl TextEditor {
  /// An empty text editor labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      editing: Editing::default(),
      enabled: None,
      min_height: Value::from(96),
      max_height: None,
      captioned: Captioned {
        placement: Value::from(CaptionPlacement::Above),
        spacing: Value::from(4),
      },
      handle: None,
      access: Access::default(),
    }
  }

  editing!(text: "", change: "", selection: "", read_only: "");

  /// The least height of the text's box, in pixels; 96 where unset.
  pub fn set_min_height(mut self, height: impl Into<Value<Dimension>>) -> Self {
    self.min_height = height.into();

    self
  }

  /// The least height of the text's box, in pixels, kept current.
  pub fn derive_min_height<V: Into<Value<Dimension>>>(
    self,
    height: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_min_height(Value::derived(height))
  }

  /// The greatest height of the text's box, past which it scrolls; none where unset.
  pub fn set_max_height(mut self, height: impl Into<Value<Dimension>>) -> Self {
    self.max_height = Some(height.into());

    self
  }

  /// The greatest height of the text's box, past which it scrolls, kept current.
  pub fn derive_max_height<V: Into<Value<Dimension>>>(
    self,
    height: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_max_height(Value::derived(height))
  }

  enabled!();
  handle!();
  captioned!("above", "4");
  access!();
}

impl View for TextEditor {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let editor = {
      use crate::{Scroll, ScrollAxes, nodes::TextView};

      let caption = NodeHandle::new();
      let view = TextView::new()
        .apply(self.enabled, |view, enabled| {
          view.derive_enabled(move || enabled())
        })
        .apply(self.handle, |view, handle| view.set_handle(&handle));
      let view = give_editing!(self.editing, view);
      let view = give_access!(self.access.captioned(&caption), view);
      let scroll = Scroll::new(ScrollAxes::Vertical)
        .set_min_height(self.min_height)
        .apply(self.max_height, Scroll::set_max_height)
        .set_child(view);

      self.captioned.compose(self.label, &caption, scroll)
    };

    editor.build(cx)
  }
}

/// A value along a range with its caption: Skene's slider, drawn on every platform.
///
/// It is controlled: it shows what [`Slider::derive_value`] returns, reports each move to
/// [`Slider::handle_change`], and is brought back to the truth after every report.
#[must_use]
pub struct Slider {
  label: String,
  value: Box<dyn Fn() -> f32>,
  range: Box<dyn Fn() -> (f32, f32)>,
  step: Box<dyn Fn() -> f32>,
  change: Option<Box<dyn Fn(f32)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  captioned: Captioned,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Slider {
  /// A slider labelled `label`, at 0 of 0 to 1, stepping by a hundredth.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      value: Box::new(|| 0.),
      range: Box::new(|| (0., 1.)),
      step: Box::new(|| 0.01),
      change: None,
      enabled: None,
      captioned: Captioned {
        placement: Value::from(CaptionPlacement::Above),
        spacing: Value::from(4),
      },
      handle: None,
      access: Access::default(),
    }
  }

  /// The slider's value.
  pub fn set_value(self, value: f32) -> Self {
    self.derive_value(move || value)
  }

  /// The slider's value, recomputed whenever a signal the closure reads changes.
  pub fn derive_value(mut self, value: impl Fn() -> f32 + 'static) -> Self {
    self.value = Box::new(value);

    self
  }

  /// Shows `value` and writes each move into it.
  pub fn bind_value(self, value: Signal<f32>) -> Self {
    self
      .derive_value(move || value.get())
      .handle_change(move |moved| value.set(moved))
  }

  /// The least and greatest values; 0 to 1 where unset.
  pub fn set_range(self, minimum: f32, maximum: f32) -> Self {
    self.derive_range(move || (minimum, maximum))
  }

  /// The least and greatest values, recomputed whenever a signal the closure reads changes.
  pub fn derive_range(mut self, range: impl Fn() -> (f32, f32) + 'static) -> Self {
    self.range = Box::new(range);

    self
  }

  /// The step the value moves by; a hundredth where unset.
  pub fn set_step(self, step: f32) -> Self {
    self.derive_step(move || step)
  }

  /// The step the value moves by, recomputed whenever a signal the closure reads changes.
  pub fn derive_step(mut self, step: impl Fn() -> f32 + 'static) -> Self {
    self.step = Box::new(step);

    self
  }

  /// Runs `change` with the value the reader moved the slider to.
  pub fn handle_change(mut self, change: impl Fn(f32) + 'static) -> Self {
    self.change = Some(Box::new(change));

    self
  }

  enabled!();
  handle!();
  captioned!("above", "4");
  access!();
}

impl View for Slider {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let slider = {
      use crate::nodes::Scale;

      let caption = NodeHandle::new();
      let (value, range, step) = (self.value, self.range, self.step);
      let scale = Scale::new()
        .derive_range(move || {
          let (minimum, maximum) = range();

          (f64::from(minimum), f64::from(maximum))
        })
        .derive_step(move || f64::from(step()))
        .derive_value(move || f64::from(value()))
        .apply(self.change, |scale, change| {
          scale.handle_change(move |moved| change(moved as f32))
        })
        .apply(self.enabled, |scale, enabled| {
          scale.derive_enabled(move || enabled())
        })
        .apply(self.handle, |scale, handle| scale.set_handle(&handle));
      let scale = give_access!(self.access.captioned(&caption), scale);

      self.captioned.compose(self.label, &caption, scale)
    };

    slider.build(cx)
  }
}

/// One choice of a list, shown closed, with its caption: Skene's picker, drawn on every
/// platform.
///
/// It is controlled: it shows what [`Picker::derive_chosen`] returns, reports each choice to
/// [`Picker::handle_choose`], and is brought back to the truth after every report.
#[must_use]
pub struct Picker {
  label: String,
  options: Box<dyn Fn() -> Vec<String>>,
  chosen: Box<dyn Fn() -> Option<usize>>,
  choose: Option<Box<dyn Fn(Option<usize>)>>,
  enabled: Option<Rc<dyn Fn() -> bool>>,
  captioned: Captioned,
  handle: Option<NodeHandle>,
  access: Access,
}

impl Picker {
  /// A picker labelled `label`, with no options.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      label: label.into(),
      options: Box::new(Vec::new),
      chosen: Box::new(|| None),
      choose: None,
      enabled: None,
      captioned: Captioned {
        placement: Value::from(CaptionPlacement::Above),
        spacing: Value::from(4),
      },
      handle: None,
      access: Access::default(),
    }
  }

  /// The options' texts.
  pub fn set_options<S: Into<String>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Vec<String> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.clone())
  }

  /// The options' texts, recomputed whenever a signal the closure reads changes.
  pub fn derive_options<S: Into<String>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.options = Box::new(move || options().into_iter().map(Into::into).collect());

    self
  }

  /// The chosen option's index, or none.
  pub fn set_chosen(self, chosen: Option<usize>) -> Self {
    self.derive_chosen(move || chosen)
  }

  /// The chosen option's index, or none, recomputed whenever a signal the closure reads
  /// changes.
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<usize> + 'static) -> Self {
    self.chosen = Box::new(chosen);

    self
  }

  /// Runs `choose` with the option the reader chose.
  pub fn handle_choose(mut self, choose: impl Fn(Option<usize>) + 'static) -> Self {
    self.choose = Some(Box::new(choose));

    self
  }

  /// Shows `chosen` and writes each choice into it.
  pub fn bind_chosen(self, chosen: Signal<Option<usize>>) -> Self {
    self
      .derive_chosen(move || chosen.get())
      .handle_choose(move |choice| chosen.set(choice))
  }

  enabled!();
  handle!();
  captioned!("above", "4");
  access!();
}

impl View for Picker {
  fn build(self, cx: &mut Cx) -> Node {
    #[cfg(skene_platform = "linux")]
    let picker = {
      use crate::nodes::DropDown;

      let caption = NodeHandle::new();
      let (options, chosen) = (self.options, self.chosen);
      let dropdown = DropDown::new()
        .derive_options(options)
        .derive_chosen(move || chosen().map(|index| index as u32))
        .apply(self.choose, |dropdown, choose| {
          dropdown.handle_choose(move |chosen| choose(chosen.map(|index| index as usize)))
        })
        .apply(self.enabled, |dropdown, enabled| {
          dropdown.derive_enabled(move || enabled())
        })
        .apply(self.handle, |dropdown, handle| dropdown.set_handle(&handle));
      let dropdown = give_access!(self.access.captioned(&caption), dropdown);

      self.captioned.compose(self.label, &caption, dropdown)
    };

    picker.build(cx)
  }
}
