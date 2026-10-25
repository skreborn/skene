//! A command, drawn with the platform's colors.

use std::rc::Rc;

use skene_foundation::{
  Activate,
  AnyKind,
  Arrangement,
  Color,
  Condition,
  ControlBase,
  DeclaredBoxStyle,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  Expr,
  HasBoxStyle,
  HasControl,
  HasLayoutBox,
  Inset,
  Label,
  LayoutBox,
  Leaf,
  MotionBase,
  MouseCursor,
  NodeId,
  PlainChild,
  RunBase,
  State,
  StaticLeaf,
  SystemColors,
  Text,
  TokenRef,
  Value,
  View,
  When,
  realize::Source,
};

use crate::Active;

/// The platform's colors in logical pixels.
type Colors = SystemColors<f32>;

/// A command: Skene's button, drawn on every engine, a [`Control`](crate::Control) over
/// [`Activate`](crate::behavior::Activate): `Enter` activates it on every press, `Space`
/// at its release, a click on its release over it.
///
/// Its label is its content and its name ([`new`](Self::new), [`derive`](Self::derive)); a
/// derived label changes in place. [`set_prominent`](Self::set_prominent) shows it as its
/// screen's primary action. Its defaults, each a value its setters replace: at least 32
/// pixels tall, padded 4 above and below and 12 at the sides inside a border of 2, its
/// label centered, its corners rounded by 6; filled with [`SystemColors::CONTROL`] and
/// its hover and pressed variants, where prominent the accent's darkened by a tenth of
/// Oklch lightness, and the resting one while disabled; its label in
/// [`SystemColors::INK`] ([`SystemColors::ON_ACCENT`] where prominent,
/// [`SystemColors::MUTED_INK`] while disabled); its border invisible but while focus
/// shows, a ring in the accent ([`SystemColors::INK`] where prominent); the click cursor,
/// the default one while disabled. A prominence that follows a signal re-inks the button
/// in place. [`set_label`](ControlBase::set_label) replaces the label, and
/// [`set_child`](ControlBase::set_child) adds a body beside it.
///
/// ```
/// use skene_web::prelude::*;
///
/// let saved = Signal::new(0);
/// let save = Button::new("Save")
///   .set_prominent(true)
///   .handle_activate(move || saved.update(|count| *count += 1));
/// ```
#[must_use]
pub struct Button {
  control: skene_foundation::Control<Active, Activate>,
  label: Source<String>,
  prominent: Source<bool>,
  text_color: Option<Value<Color, Condition<f32>>>,
}

impl std::fmt::Debug for Button {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Button").finish_non_exhaustive()
  }
}

impl Button {
  /// A button labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    Self::from_label(Source::Fixed(label.into()))
  }

  /// A button labelled what `label` returns, kept current.
  pub fn derive<S: Into<String>>(label: impl Fn() -> S + 'static) -> Self {
    Self::from_label(Source::derive(move || label().into()))
  }

  /// A button labelled `label`, neither prominent nor otherwise styled.
  fn from_label(label: Source<String>) -> Self {
    Self {
      control: skene_foundation::Control::new(Activate::new()),
      label,
      prominent: Source::Fixed(false),
      text_color: None,
    }
  }

  /// Sets whether the button shows as its screen's primary action: filled with the
  /// accent, its label in the color that sits on it, unless its style says otherwise.
  #[inline(always)]
  pub fn set_prominent(mut self, prominent: bool) -> Self {
    self.prominent = Source::Fixed(prominent);

    self
  }

  /// Derives whether the button shows as its screen's primary action from the signals
  /// `prominent` reads, re-inking it in place as they change.
  pub fn derive_prominent(mut self, prominent: impl Fn() -> bool + 'static) -> Self {
    self.prominent = Source::derive(prominent);

    self
  }

  /// Sets the color the button's own label is drawn in.
  pub fn set_text_color(mut self, color: impl Into<Value<Color, Condition<f32>>>) -> Self {
    self.text_color = Some(color.into());

    self
  }

  /// Derives the color the button's own label is drawn in from the signals `color`
  /// reads: a color or a token.
  pub fn derive_text_color<R: Into<StaticLeaf<Color>>>(
    mut self,
    color: impl Fn() -> R + 'static,
  ) -> Self {
    self.text_color = Some(Value::new(Leaf::derive(color)));

    self
  }
}

impl Button {
  /// How far a prominent button's fill darkens the accent, in Oklch lightness.
  const PROMINENT_DARKENING: f32 = 0.1;

  /// The leaf `pick` makes of a button's prominence: a static leaf where it is fixed,
  /// and a leaf derived from it where it follows a signal.
  fn by_prominence(
    prominent: &Source<bool>,
    pick: impl Fn(bool) -> StaticLeaf<Color> + 'static,
  ) -> Leaf<Color> {
    match prominent {
      Source::Fixed(prominent) => Leaf::Static(pick(*prominent)),
      Source::Derived(prominent) => {
        let prominent = Rc::clone(prominent);

        Leaf::derive(move || pick(prominent()))
      }
    }
  }

  /// The fill `neutral` names, or where prominent `accent` darkened by
  /// [`PROMINENT_DARKENING`](Self::PROMINENT_DARKENING).
  fn fill(
    prominent: &Source<bool>,
    neutral: TokenRef<Color>,
    accent: TokenRef<Color>,
  ) -> Leaf<Color> {
    Self::by_prominence(prominent, move |prominent| {
      if prominent {
        StaticLeaf::from(Expr::from(accent).darker(Self::PROMINENT_DARKENING))
      }
      else {
        StaticLeaf::from(neutral)
      }
    })
  }

  /// The color `neutral` names, or `accent` where prominent.
  fn ink(
    prominent: &Source<bool>,
    neutral: TokenRef<Color>,
    accent: TokenRef<Color>,
  ) -> Leaf<Color> {
    Self::by_prominence(prominent, move |prominent| {
      StaticLeaf::from(if prominent { accent } else { neutral })
    })
  }

  /// The button's defaults, its label in `text_color`.
  fn defaults(
    label: Source<String>,
    prominent: &Source<bool>,
    text_color: Value<Color, Condition<f32>>,
  ) -> skene_foundation::Control<Active, Activate> {
    let background =
      Value::<Color, Condition<f32>>::new(Self::fill(prominent, Colors::CONTROL, Colors::ACCENT))
        .when(
          State::Hovered.into(),
          Self::fill(prominent, Colors::CONTROL_HOVER, Colors::ACCENT_HOVER),
        )
        .when(
          State::Pressed.into(),
          Self::fill(prominent, Colors::CONTROL_PRESSED, Colors::ACCENT_PRESSED),
        )
        .when(State::Disabled.into(), Colors::CONTROL);
    let ring = Color::TRANSPARENT.when(
      Condition::from(State::FocusVisible),
      Self::ink(prominent, Colors::ACCENT, Colors::INK),
    );
    let label = match label {
      Source::Fixed(label) => {
        Label::new(Text::<Active>::new(label.clone()).set_color(text_color)).set_name(label)
      }
      Source::Derived(label) => {
        let named = Rc::clone(&label);

        Label::new(Text::<Active>::derive(move || label()).set_color(text_color))
          .derive_name(move || named())
      }
    };

    skene_foundation::Control::new(Activate::new())
      .set_min_height(Dimension::Length(32.))
      .set_padding(Inset::symmetric(12, 4))
      .set_border_width(Inset::all(2))
      .set_border_color(ring)
      .set_corner_radius(6)
      .set_background(background)
      .set_cursor(MouseCursor::Click.when(State::Disabled, MouseCursor::Default))
      .map_layout_box(|layout| layout.set_arrangement(Arrangement::Center))
      .set_label(label)
  }
}

impl HasControl<Active, Activate> for Button {
  #[inline(always)]
  fn control(&self) -> &skene_foundation::Control<Active, Activate> {
    &self.control
  }

  #[inline(always)]
  fn map_control(
    mut self,
    map: impl FnOnce(
      skene_foundation::Control<Active, Activate>,
    ) -> skene_foundation::Control<Active, Activate>,
  ) -> Self {
    self.control = map(self.control);

    self
  }
}

impl HasBoxStyle<Active> for Button {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<Active> {
    self.control.box_style_mut()
  }
}

impl HasLayoutBox<Active> for Button {
  type Kind = PlainChild;

  #[inline(always)]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<Active, PlainChild>) -> LayoutBox<Active, PlainChild>,
  ) -> Self {
    self.map_control(|control| control.map_layout_box(map))
  }
}

impl ControlBase<Active, Activate> for Button {}

impl DecorationBase<Active> for Button {}

impl MotionBase<Active> for Button {}

impl AnyKind for Button {}

impl View<Active> for Button {
  fn build(self) -> NodeId {
    let Self {
      control,
      label,
      prominent,
      text_color,
    } = self;
    let text_color = text_color.unwrap_or_else(|| {
      Value::new(Self::ink(&prominent, Colors::INK, Colors::ON_ACCENT))
        .when(State::Disabled.into(), Colors::MUTED_INK)
    });

    control
      .over(Self::defaults(label, &prominent, text_color))
      .build()
  }
}
