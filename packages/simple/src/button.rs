//! A ready-made button in the Simple design language.

use std::rc::Rc;

use skene_foundation::{
  Activate,
  AnyKind,
  AnyView,
  BasicButton,
  Color,
  CornerRadius,
  Definition,
  Definitions,
  Inset,
  Label,
  NodeId,
  Platform,
  Property,
  Scene,
  State,
  Text,
  TokenRef,
  Transition,
  View,
  realize::Source,
  typography,
};
use skene_reactive::{Effect, With};

use super::{
  SimpleLook,
  SimplePalette,
  SimpleVariant,
  shape::{SimpleUnit, click_cursor, palette, shape},
};

/// The surface of the look in force on a button.
const SURFACE: TokenRef<Color> = TokenRef::new("simple-look-surface");
/// The ink of the look in force on a button.
const INK: TokenRef<Color> = TokenRef::new("simple-look-ink");
/// The border of the look in force on a button; transparent for a filled look.
const BORDER: TokenRef<Color> = TokenRef::new("simple-look-border");
/// The hover surface of the look in force on a button.
const HOVER: TokenRef<Color> = TokenRef::new("simple-look-hover");
/// How long the surface takes to settle on a hover or focus change, in seconds.
const SURFACE_SETTLE: f32 = 0.15;
/// The focus surface of the look in force on a button.
const FOCUS: TokenRef<Color> = TokenRef::new("simple-look-focus");

/// What a button draws: its label's text, or a view in its place.
enum Content<P: Platform> {
  Text(Source<String>),
  View(AnyView<P>),
}

/// A ready-made button: a text label on the variant's surface, with hover and focus
/// feedback and the platform's full keyboard behavior.
///
/// Pick the emphasis with [`set_variant`](Self::set_variant) or follow a signal with
/// [`derive_variant`](Self::derive_variant); the look is redefined on the button's own
/// node, so a change re-inks it without a rebuild. A disabled button takes the
/// palette's disabled surface and ink whatever its variant, and the default cursor.
/// [`set_child`](Self::set_child) draws a view in the label's place, inked in the look's ink. For
/// anything beyond that, compose [`BasicButton`] directly.
#[must_use]
pub struct SimpleButton<P: Platform> {
  raw: BasicButton<P>,
  content: Content<P>,
  variant: Source<SimpleVariant>,
  disabled: Source<bool>,
}

impl<P: Platform> std::fmt::Debug for SimpleButton<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("SimpleButton")
      .finish_non_exhaustive()
  }
}

impl<P: Platform> SimpleButton<P> {
  /// A button carrying `label`.
  pub fn new(label: impl Into<String>) -> Self {
    Self {
      raw: BasicButton::new(Activate::new()),
      content: Content::Text(Source::Fixed(label.into())),
      variant: Source::Fixed(SimpleVariant::Primary),
      disabled: Source::Fixed(false),
    }
  }

  /// A button whose label is recomputed whenever a signal `label` reads changes.
  pub fn derive(label: impl Fn() -> String + 'static) -> Self {
    Self {
      raw: BasicButton::new(Activate::new()),
      content: Content::Text(Source::derive(label)),
      variant: Source::Fixed(SimpleVariant::Primary),
      disabled: Source::Fixed(false),
    }
  }

  /// Sets the emphasis variant.
  #[inline(always)]
  pub fn set_variant(mut self, variant: SimpleVariant) -> Self {
    self.variant = Source::Fixed(variant);

    self
  }

  /// Derives the emphasis variant from the signals `variant` reads.
  pub fn derive_variant(mut self, variant: impl Fn() -> SimpleVariant + 'static) -> Self {
    self.variant = Source::derive(variant);

    self
  }

  /// Sets whether the button is present but not operable.
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.disabled = Source::Fixed(disabled);

    self
  }

  /// Derives whether the button is disabled from the signals `disabled` reads.
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.disabled = Source::derive(disabled);

    self
  }

  /// Sets the name the button reports to assistive technology, over its label's: for a
  /// button [`set_child`](Self::set_child) draws with an icon alone.
  pub fn set_accessible_name(mut self, name: impl Into<String>) -> Self {
    self.raw = self.raw.set_accessible_name(name);

    self
  }

  /// Derives the name the button reports to assistive technology from the signals
  /// `name` reads.
  pub fn derive_accessible_name(mut self, name: impl Fn() -> String + 'static) -> Self {
    self.raw = self.raw.derive_accessible_name(name);

    self
  }

  /// Runs `action` on activation by pointer, `Enter` or `Space`.
  pub fn handle_click(mut self, action: impl Fn() + 'static) -> Self {
    self.raw = self.raw.handle_click(action);

    self
  }

  /// Sets a view drawn in the label's place; the button then has no text of its own.
  pub fn set_child(mut self, child: impl View<P>) -> Self {
    self.content = Content::View(child.into_any());

    self
  }
}

impl<P: Platform> AnyKind for SimpleButton<P> {}

impl<P: Platform> View<P> for SimpleButton<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    let palette = palette::<P>();
    let variant: Rc<dyn Fn() -> SimpleVariant> = match self.variant {
      Source::Fixed(variant) => Rc::new(move || variant),
      Source::Derived(variant) => variant,
    };
    let shape = shape::<P>();
    let (raw, disabled): (_, Rc<dyn Fn() -> bool>) = match self.disabled {
      Source::Fixed(disabled) => (self.raw.set_disabled(disabled), Rc::new(move || disabled)),
      Source::Derived(disabled) => (
        self.raw.derive_disabled({
          let disabled = Rc::clone(&disabled);

          move || disabled()
        }),
        disabled,
      ),
    };
    let raw = raw
      .set_padding(shape.control_inset)
      .set_color(
        SURFACE
          .when(State::Hovered, HOVER)
          .when(State::FocusVisible, FOCUS),
      )
      .set_border_width(Inset::all(shape.control_ring))
      .set_border_color(BORDER)
      .set_corner_radius(shape.control_radius)
      .set_cursor(click_cursor())
      .set_transition(Transition::new(Property::Color, SURFACE_SETTLE));
    let node = match self.content {
      Content::Text(Source::Fixed(label)) => raw.set_label(label).build(),
      Content::Text(Source::Derived(label)) => raw
        .set_label(Label::new(Text::<P>::derive(move || label())))
        .build(),
      Content::View(view) => raw.set_child(view).build(),
    };

    Effect::render(move |_: Option<()>| {
      let definitions = palette.with(|palette| {
        let look = palette.look(variant());

        if disabled() {
          disabled_definitions::<P>(look.border.is_some())
        }
        else {
          look_definitions::<P>(look)
        }
      });

      Scene::<P>::with(|scene| scene.define_tokens(node, definitions));
    });

    node
  }
}

/// The button's own tokens defined as the palette's disabled surface and ink, which
/// neither hover nor focus changes, its border the palette's where the look is
/// `bordered` and transparent otherwise, and the typography ink as the disabled ink.
fn disabled_definitions<P: Platform>(bordered: bool) -> Definitions<P::TokenValue> {
  let alias = |token: TokenRef<Color>, target: TokenRef<Color>| {
    (
      token.name(),
      P::TokenValue::from(Definition::<Color, P::Scalar>::from(target)),
    )
  };
  let border = if bordered {
    Definition::<Color, P::Scalar>::from(SimplePalette::<P::Scalar>::BORDER)
  }
  else {
    Definition::from(Color::TRANSPARENT)
  };

  Definitions::from(vec![
    alias(SURFACE, SimplePalette::<P::Scalar>::DISABLED_SURFACE),
    alias(INK, SimplePalette::<P::Scalar>::DISABLED_INK),
    (BORDER.name(), P::TokenValue::from(border)),
    alias(HOVER, SimplePalette::<P::Scalar>::DISABLED_SURFACE),
    alias(FOCUS, SimplePalette::<P::Scalar>::DISABLED_SURFACE),
    alias(typography::INK, INK),
  ])
}

/// The button's own tokens defined as `look`'s, and the typography ink as the look's
/// ink.
fn look_definitions<P: Platform>(look: &SimpleLook<P::Scalar>) -> Definitions<P::TokenValue> {
  let alias = |token: TokenRef<Color>, target: TokenRef<Color>| {
    (
      token.name(),
      P::TokenValue::from(Definition::<Color, P::Scalar>::from(target)),
    )
  };
  let border = look.border.as_ref().map_or_else(
    || Definition::<Color, P::Scalar>::from(Color::TRANSPARENT),
    |border| Definition::from(border.reference()),
  );

  Definitions::from(vec![
    alias(SURFACE, look.surface.reference()),
    alias(INK, look.ink.reference()),
    (BORDER.name(), P::TokenValue::from(border)),
    alias(HOVER, look.hover.reference()),
    alias(FOCUS, look.focus.reference()),
    alias(typography::INK, INK),
  ])
}
