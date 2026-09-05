//! A ready-made button in the Simple design language.

use std::rc::Rc;

use skene_core::{
  AnyKind,
  AnyView,
  Color,
  Definition,
  Definitions,
  Driver,
  IntoView,
  MouseCursor,
  NodeId,
  Scene,
  State,
  TokenRef,
  View,
};
use skene_reactive::{Effect, With};
use skene_views::{Activate, CornerRadius, Label, RawButton, Text, realize::Source, typography};

use super::{
  SimpleLook,
  SimpleVariant,
  shape::{RADIUS, SimpleUnit, border, palette, shape},
};

/// The surface of the look in force on a button.
const SURFACE: TokenRef<Color> = TokenRef::new("simple-look-surface");
/// The ink of the look in force on a button.
const INK: TokenRef<Color> = TokenRef::new("simple-look-ink");
/// The border of the look in force on a button; transparent for a filled look.
const BORDER: TokenRef<Color> = TokenRef::new("simple-look-border");
/// The hover surface of the look in force on a button.
const HOVER: TokenRef<Color> = TokenRef::new("simple-look-hover");
/// The focus surface of the look in force on a button.
const FOCUS: TokenRef<Color> = TokenRef::new("simple-look-focus");

/// What a button draws: its label's text, or a view in its place.
enum Content<D: Driver> {
  Text(Source<String>),
  View(AnyView<D>),
}

/// A ready-made button: a text label on the variant's surface, with hover and focus
/// feedback and the platform's full keyboard behavior.
///
/// Pick the emphasis with [`set_variant`](Self::set_variant) or follow a signal with
/// [`derive_variant`](Self::derive_variant); the look is redefined on the button's own
/// node, so a change re-inks it without a rebuild. [`set_child`](Self::set_child)
/// draws a view in the label's place, inked in the look's ink. For anything beyond
/// that, compose [`RawButton`] directly.
#[must_use]
pub struct SimpleButton<D: Driver> {
  raw: RawButton<D>,
  content: Content<D>,
  variant: Source<SimpleVariant>,
}

impl<D: Driver> SimpleButton<D> {
  /// A button carrying `label`.
  #[inline(always)]
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      raw: RawButton::new(Activate::new()),
      content: Content::Text(Source::Fixed(label.into())),
      variant: Source::Fixed(SimpleVariant::Primary),
    }
  }

  /// A button whose label is recomputed whenever a signal `label` reads changes.
  #[inline(always)]
  pub fn derive(label: impl Fn() -> String + 'static) -> Self {
    Self {
      raw: RawButton::new(Activate::new()),
      content: Content::Text(Source::derive(label)),
      variant: Source::Fixed(SimpleVariant::Primary),
    }
  }

  /// Sets the emphasis variant.
  #[inline(always)]
  pub fn set_variant(mut self, variant: SimpleVariant) -> Self {
    self.variant = Source::Fixed(variant);

    self
  }

  /// Derives the emphasis variant from the signals `variant` reads.
  #[inline(always)]
  pub fn derive_variant(mut self, variant: impl Fn() -> SimpleVariant + 'static) -> Self {
    self.variant = Source::derive(variant);

    self
  }

  /// Sets whether the button is present but not operable.
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.raw = self.raw.set_disabled(disabled);

    self
  }

  /// Derives whether the button is disabled from the signals `disabled` reads.
  #[inline(always)]
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.raw = self.raw.derive_disabled(disabled);

    self
  }

  /// Runs `action` on activation by pointer, `Enter` or `Space`.
  #[inline(always)]
  pub fn handle_click(mut self, action: impl Fn() + 'static) -> Self {
    self.raw = self.raw.handle_click(action);

    self
  }

  /// Sets a view drawn in the label's place; the button then has no text of its own.
  #[inline(always)]
  pub fn set_child(mut self, child: impl IntoView<D>) -> Self {
    self.content = Content::View(child.into_view());

    self
  }
}

impl<D: Driver> AnyKind for SimpleButton<D> {}

impl<D: Driver> View<D> for SimpleButton<D>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    let palette = palette::<D>();
    let variant: Rc<dyn Fn() -> SimpleVariant> = match self.variant {
      Source::Fixed(variant) => Rc::new(move || variant),
      Source::Derived(variant) => variant,
    };
    let shape = shape::<D>();
    let raw = self
      .raw
      .set_padding(shape.control_inset)
      .set_color(
        SURFACE
          .when(State::Hovered, HOVER)
          .when(State::FocusVisible, FOCUS),
      )
      .set_border(border(shape.control_ring, BORDER))
      .set_corner_radius(RADIUS)
      .set_cursor(MouseCursor::Click);
    let node = match self.content {
      Content::Text(Source::Fixed(label)) => raw.set_label(label).build(),
      Content::Text(Source::Derived(label)) => raw
        .set_label(Label::new(Text::<D>::derive(move || label())))
        .build(),
      Content::View(view) => raw.set_child(view).build(),
    };

    Effect::render(move |_: Option<()>| {
      let definitions = palette.with(|palette| look_definitions::<D>(palette.look(variant())));

      Scene::<D>::with(|scene| scene.define_tokens(node, definitions));
    });

    node
  }
}

/// The button's own tokens defined as `look`'s, and the typography ink as the look's
/// ink.
fn look_definitions<D: Driver>(look: &SimpleLook<D::Scalar>) -> Definitions<D::TokenValue> {
  let alias = |token: TokenRef<Color>, target: TokenRef<Color>| {
    (
      token.name(),
      D::TokenValue::from(Definition::<Color, D::Scalar>::from(target)),
    )
  };
  let border = look.border.as_ref().map_or_else(
    || Definition::<Color, D::Scalar>::from(Color::TRANSPARENT),
    |border| Definition::from(border.reference()),
  );

  Definitions::from(vec![
    alias(SURFACE, look.surface.reference()),
    alias(INK, look.ink.reference()),
    (BORDER.name(), D::TokenValue::from(border)),
    alias(HOVER, look.hover.reference()),
    alias(FOCUS, look.focus.reference()),
    alias(typography::INK, INK),
  ])
}
