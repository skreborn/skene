//! A ready-made button in the Simple design language.

use std::rc::Rc;

use skene_foundation::{Arrangement, Property, Transition, realize::Source};
use skene_simple::SimpleVariant;
use skene_tui::{
  Button,
  Color,
  DecorationBase,
  DecorationWithCornerRadius,
  HasLayoutBox,
  Inset,
  Leaf,
  MotionBase,
  State,
  TokenRef,
  Value,
};

use crate::{SimplePalette, shape};

/// A ready-made button: a text label on its variant's surface, with hover and focus
/// feedback and the platform's full keyboard behavior, a preset of [`Button`].
///
/// A constructor per variant returns the button, so every member of a button applies:
/// [`new`](Self::new) is the primary action, [`secondary`](Self::secondary),
/// [`outline`](Self::outline), [`ghost`](Self::ghost) and
/// [`destructive`](Self::destructive) its siblings, and
/// [`derive_variant`](Self::derive_variant) follows a signal, re-inking the button in
/// place as it changes. A disabled button takes the palette's disabled surface and ink
/// whatever its variant. [`styled`](Self::styled) and
/// [`derive_styled`](Self::derive_styled) dress a button built otherwise, a derived
/// label's or an icon's. Its colors are [`SimplePalette`]'s tokens, inked in the
/// palette's default unless a [`Theme`](skene_tui::Theme) of the palette above redefines
/// them.
///
/// ```
/// use skene_simple_tui::prelude::*;
/// use skene_tui::prelude::*;
///
/// let removed = Signal::new(0);
/// let delete = SimpleButton::destructive("Delete")
///   .handle_activate(move || removed.update(|count| *count += 1));
/// ```
#[derive(Debug)]
pub enum SimpleButton {}

impl SimpleButton {
  /// How long the surface takes to settle on a hover or focus change, in seconds.
  const SURFACE_SETTLE: f32 = 0.15;

  /// The main action's button, carrying `label`.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(label: impl Into<String>) -> Button {
    Self::styled(Button::new(label), SimpleVariant::Primary)
  }

  /// A supporting action's button, carrying `label`.
  pub fn secondary(label: impl Into<String>) -> Button {
    Self::styled(Button::new(label), SimpleVariant::Secondary)
  }

  /// A hairline-bordered action's button, carrying `label`.
  pub fn outline(label: impl Into<String>) -> Button {
    Self::styled(Button::new(label), SimpleVariant::Outline)
  }

  /// A bare action's button, carrying `label`.
  pub fn ghost(label: impl Into<String>) -> Button {
    Self::styled(Button::new(label), SimpleVariant::Ghost)
  }

  /// A dangerous action's button, carrying `label`.
  pub fn destructive(label: impl Into<String>) -> Button {
    Self::styled(Button::new(label), SimpleVariant::Destructive)
  }

  /// A button carrying `label` in the variant `variant` returns, re-inked in place as
  /// the signals it reads change.
  pub fn derive_variant(
    label: impl Into<String>,
    variant: impl Fn() -> SimpleVariant + 'static,
  ) -> Button {
    Self::derive_styled(Button::new(label), variant)
  }

  /// `button` dressed in `variant`'s look.
  pub fn styled(button: Button, variant: SimpleVariant) -> Button {
    Self::dress(button, &Source::Fixed(variant))
  }

  /// `button` dressed in the look of the variant `variant` returns, re-inked in place as
  /// the signals it reads change.
  pub fn derive_styled(button: Button, variant: impl Fn() -> SimpleVariant + 'static) -> Button {
    Self::dress(button, &Source::derive(variant))
  }

  /// `button` on the variant's surface, in its ink and border, the shape's padding,
  /// ring and radius around its label at the start, its surface settling on a change.
  fn dress(button: Button, variant: &Source<SimpleVariant>) -> Button {
    let shape = shape();

    button
      .set_padding(shape.control_inset)
      .set_background(
        Value::new(Self::leaf(variant, SimplePalette::surface_of))
          .when(
            State::Hovered.into(),
            Self::leaf(variant, SimplePalette::hover_of),
          )
          .when(
            State::FocusVisible.into(),
            Self::leaf(variant, SimplePalette::focus_of),
          )
          .when(State::Disabled.into(), SimplePalette::DISABLED_SURFACE),
      )
      .set_text_color(
        Value::new(Self::leaf(variant, SimplePalette::ink_of))
          .when(State::Disabled.into(), SimplePalette::DISABLED_INK),
      )
      .set_border_width(Inset::all(shape.control_ring))
      .set_border_color(Value::new(Self::leaf(variant, SimplePalette::border_of)))
      .set_corner_radius(shape.control_radius)
      .add_transition(Transition::new(Property::Background, Self::SURFACE_SETTLE))
      .map_layout_box(|layout| layout.set_arrangement(Arrangement::Start))
  }

  /// The leaf naming `token` of the variant: the token itself where the variant is
  /// fixed, and a leaf derived from it where it follows a signal.
  fn leaf(
    variant: &Source<SimpleVariant>,
    token: fn(SimpleVariant) -> TokenRef<Color>,
  ) -> Leaf<Color> {
    match variant {
      Source::Fixed(variant) => Leaf::from(token(*variant)),
      Source::Derived(variant) => {
        let variant = Rc::clone(variant);

        Leaf::derive(move || token(variant()))
      }
    }
  }
}
