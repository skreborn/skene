//! A command.

use std::{cell::RefCell, rc::Rc};

use crate::{
  Arrangement, AxisAlignment, Color, Fill, Inset, State, SystemColors, Value,
  behavior::Activate,
  seam::Realization,
  view::{
    AnyKind, ControlBase, ControlMembersBase, Cx, DecorationBase, Label, Node, RunBase, Text,
    TextBase, View, Wraps,
    controls::{control_surfaces, focus_ring},
    wraps_itself,
  },
};

/// Whether a button shows as its screen's primary action, as its setter left it.
type Prominent = Rc<RefCell<Rc<dyn Fn() -> bool>>>;

/// A command: Skene's button, drawn on every platform. It is a [`crate::view::Control`] over
/// [`Activate`]: `Enter` activates it at once, `Space` at its release, a click on its release
/// over it.
///
/// Its label is its content and its name ([`ButtonBase::new`], [`ButtonBase::derive`]); a
/// derived label changes in place. Its defaults: a row 34 pixels tall at least and as wide,
/// padded 7 above and below and 14 at the sides, its label centered, its corners rounded by
/// 8, filled with [`SystemColors::CONTROL`] (the accent where prominent) and its hover and
/// pressed variants, its text in [`SystemColors::INK`] ([`SystemColors::ON_ACCENT`] where
/// prominent), and a ring of 2 in the accent while focus shows.
#[must_use]
pub struct Button<R: Realization> {
  /// The control it composes, its defaults set.
  control: crate::view::Control<Activate, R>,
  /// Whether it shows as its screen's primary action.
  prominent: Prominent,
}

wraps_itself!(Button<R> where R: Realization);

control_surfaces!([R: Realization] Button<R>, R => control);

impl<R: Realization> Button<R> {
  /// The default text color: [`SystemColors::INK`], [`SystemColors::ON_ACCENT`] where
  /// `prominent`.
  pub fn default_color(prominent: bool) -> Value<Color> {
    Value::from(if prominent {
      SystemColors::ON_ACCENT
    } else {
      SystemColors::INK
    })
  }

  /// The default fill: [`SystemColors::CONTROL`] and its hover and pressed variants, the
  /// accent's where `prominent`; the resting one while disabled.
  pub fn default_background(prominent: bool) -> Value<Fill> {
    let (rest, hover, pressed) = if prominent {
      (
        SystemColors::ACCENT,
        SystemColors::ACCENT_HOVER,
        SystemColors::ACCENT_PRESSED,
      )
    } else {
      (
        SystemColors::CONTROL,
        SystemColors::CONTROL_HOVER,
        SystemColors::CONTROL_PRESSED,
      )
    };

    Value::from(rest)
      .when(State::Hovered, hover)
      .when(State::Pressed, pressed)
      .when(State::Disabled, rest)
  }
}

/// What a [`Button`] carries beside every control's members: its label, its activation and
/// whether it shows as its screen's primary action.
pub trait ButtonBase<R: Realization>: Wraps<Inner = Button<R>> {
  /// A button labelled `label`.
  fn new(label: impl Into<String>) -> Self {
    let label = label.into();

    Self::derive(move || label.clone())
  }

  /// A button labelled what `label` returns, kept current.
  fn derive<S: Into<String>>(label: impl Fn() -> S + 'static) -> Self {
    let label: Rc<dyn Fn() -> String> = Rc::new(move || label().into());
    let prominent: Prominent = Rc::new(RefCell::new(Rc::new(|| false)));
    let is_prominent = {
      let prominent = Rc::clone(&prominent);

      move || {
        let prominent = prominent.borrow().clone();

        prominent()
      }
    };
    let background = {
      let is_prominent = is_prominent.clone();

      Value::derived(move || Button::<R>::default_background(is_prominent()))
    };
    let color = Value::derived(move || Button::<R>::default_color(is_prominent()));
    let text = {
      let label = Rc::clone(&label);

      Text::<R>::derive(move || label()).set_max_lines(1)
    };
    let control = crate::view::Control::new(Activate::new())
      .set_min_height(34)
      .set_min_width(34)
      .set_padding(Inset::new(7, 14, 7, 14))
      .set_corner_radius(8)
      .set_arrangement(Arrangement::Center)
      .set_item_alignment(AxisAlignment::Center)
      .set_background(background)
      .set_color(color)
      .set_ring(focus_ring())
      .set_ring_color(SystemColors::ACCENT)
      .set_label(Label::new(text).derive_name(move || label()));

    Self::wrap(Button { control, prominent })
  }

  /// Runs `activate` when the button is activated: clicked, or pressed from the keyboard.
  fn handle_activate(self, activate: impl Fn() + 'static) -> Self {
    self.map_inner(|mut button| {
      button.control = button
        .control
        .map_behavior(|behavior| behavior.handle_activate(activate));

      button
    })
  }

  /// Shows the button as the screen's primary action: filled with the accent, its text in
  /// the color that sits on it, unless its style says otherwise.
  fn set_prominent(self, prominent: bool) -> Self {
    self.derive_prominent(move || prominent)
  }

  /// Shows the button as the screen's primary action, kept current.
  fn derive_prominent(self, prominent: impl Fn() -> bool + 'static) -> Self {
    *self.inner().prominent.borrow_mut() = Rc::new(prominent);

    self
  }
}

impl<R: Realization> ButtonBase<R> for Button<R> {}

impl<R: Realization> AnyKind for Button<R> {}

impl<R: Realization> View<R> for Button<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    self.control.build(cx)
  }
}
