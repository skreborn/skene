//! The Simple design language's tokens: the palette as a token set, the emphasis
//! looks, and the default every facade starts from.

use skene_core::{
  Color,
  next::{Definition, Media, Scalar, Token, TokenName, TokenRef, TokenSet},
};
use skene_views::typography;

use crate::palette::SimpleVariant;

impl SimpleVariant {
  /// The variant's surface token.
  #[inline(always)]
  pub const fn surface(self) -> TokenRef<Color> {
    TokenRef::new(match self {
      Self::Primary => "simple-primary-surface",
      Self::Secondary => "simple-secondary-surface",
      Self::Outline => "simple-outline-surface",
      Self::Ghost => "simple-ghost-surface",
      Self::Destructive => "simple-destructive-surface",
    })
  }

  /// The variant's ink token.
  #[inline(always)]
  pub const fn ink(self) -> TokenRef<Color> {
    TokenRef::new(match self {
      Self::Primary => "simple-primary-ink",
      Self::Secondary => "simple-secondary-ink",
      Self::Outline => "simple-outline-ink",
      Self::Ghost => "simple-ghost-ink",
      Self::Destructive => "simple-destructive-ink",
    })
  }

  /// The variant's border token, defined only by a bordered look.
  #[inline(always)]
  pub const fn border(self) -> TokenRef<Color> {
    TokenRef::new(match self {
      Self::Primary => "simple-primary-border",
      Self::Secondary => "simple-secondary-border",
      Self::Outline => "simple-outline-border",
      Self::Ghost => "simple-ghost-border",
      Self::Destructive => "simple-destructive-border",
    })
  }

  /// The variant's hover surface token.
  #[inline(always)]
  pub const fn hover(self) -> TokenRef<Color> {
    TokenRef::new(match self {
      Self::Primary => "simple-primary-hover",
      Self::Secondary => "simple-secondary-hover",
      Self::Outline => "simple-outline-hover",
      Self::Ghost => "simple-ghost-hover",
      Self::Destructive => "simple-destructive-hover",
    })
  }

  /// The variant's focus surface token.
  #[inline(always)]
  pub const fn focus(self) -> TokenRef<Color> {
    TokenRef::new(match self {
      Self::Primary => "simple-primary-focus",
      Self::Secondary => "simple-secondary-focus",
      Self::Outline => "simple-outline-focus",
      Self::Ghost => "simple-ghost-focus",
      Self::Destructive => "simple-destructive-focus",
    })
  }
}

/// One emphasis variant's colors: the surface it paints, the ink on that surface, and
/// the surfaces its interaction states paint instead.
///
/// Each color is a token named for the variant, so a consumer emits the name and the
/// theme in force supplies the value. A `None` border says the variant is filled
/// rather than bordered; a Simple view then draws a transparent hairline, so switching
/// variants shifts nothing.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct SimpleLook<N: Scalar> {
  /// The action's surface.
  pub surface: Token<Color, N>,
  /// The ink on that surface.
  pub ink: Token<Color, N>,
  /// The hairline border, on the variants that carry one.
  pub border: Option<Token<Color, N>>,
  /// The surface shown under the pointer.
  pub hover: Token<Color, N>,
  /// The surface shown while focused, on platforms without a focus ring of their own.
  pub focus: Token<Color, N>,
  variant: SimpleVariant,
}

impl<N: Scalar> SimpleLook<N> {
  /// A look for `variant` painting `ink` on `surface`, with no border and neither a
  /// hover nor a focus surface of its own.
  pub fn new(
    variant: SimpleVariant,
    surface: impl Into<Definition<Color, N>>,
    ink: impl Into<Definition<Color, N>>,
  ) -> Self {
    let surface = surface.into();

    Self {
      surface: Token::new(variant.surface().name().as_str(), surface.clone()),
      ink: Token::new(variant.ink().name().as_str(), ink),
      border: None,
      hover: Token::new(variant.hover().name().as_str(), surface.clone()),
      focus: Token::new(variant.focus().name().as_str(), surface),
      variant,
    }
  }

  /// The variant the look colors.
  #[inline(always)]
  pub const fn variant(&self) -> SimpleVariant {
    self.variant
  }

  /// Sets the action's surface.
  #[inline(always)]
  pub fn set_surface(mut self, surface: impl Into<Definition<Color, N>>) -> Self {
    self.surface = self.surface.redefine(surface);

    self
  }

  /// Sets the ink on the action's surface.
  #[inline(always)]
  pub fn set_ink(mut self, ink: impl Into<Definition<Color, N>>) -> Self {
    self.ink = self.ink.redefine(ink);

    self
  }

  /// Gives the action a hairline border, making it a bordered variant.
  #[inline(always)]
  pub fn set_border(mut self, border: impl Into<Definition<Color, N>>) -> Self {
    self.border = Some(Token::new(self.variant.border().name().as_str(), border));

    self
  }

  /// Sets the surface shown under the pointer.
  #[inline(always)]
  pub fn set_hover(mut self, hover: impl Into<Definition<Color, N>>) -> Self {
    self.hover = self.hover.redefine(hover);

    self
  }

  /// Sets the surface shown while focused.
  #[inline(always)]
  pub fn set_focus(mut self, focus: impl Into<Definition<Color, N>>) -> Self {
    self.focus = self.focus.redefine(focus);

    self
  }

  /// Appends the look's definitions, erased into `V`.
  fn define_into<V: From<Definition<Color, N>>>(&self, definitions: &mut Vec<(TokenName, V)>) {
    definitions.push(self.surface.define());
    definitions.push(self.ink.define());

    if let Some(border) = &self.border {
      definitions.push(border.define());
    }

    definitions.push(self.hover.define());
    definitions.push(self.focus.define());
  }
}

/// The Simple components' design tokens: the chrome colors and one [`SimpleLook`] per
/// emphasis variant, over the facade's scalar `N`.
///
/// A palette is a value: build one, hand it around, hand it to a `Theme`. Every field
/// is a [`Token`] and every setter takes a [`Definition`], which carries its own
/// conditions: `set_card(Definition::from(white).when(Media::Dark, ink))`. The color
/// scheme is one such condition and nothing more.
///
/// The palette also defines the views' [`typography`] tokens as aliases—the ink is the
/// foreground, the link ink the link color, selected text the background on the
/// foreground—so a plain text under a Simple theme reads as one design with the
/// controls. Use the tokens for the chrome around the components too.
///
/// This type is the Simple library's, not the framework's: a third-party component
/// library defines its own struct of [`Token`]s and is on exactly the same footing, and
/// [`default`](Self::default) is built from the same public constructors.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct SimplePalette<N: Scalar> {
  /// The page background.
  pub background: Token<Color, N>,
  /// The default text ink.
  pub foreground: Token<Color, N>,
  /// De-emphasized text: captions, hints, section titles.
  pub muted_foreground: Token<Color, N>,
  /// A raised surface: cards, popovers, modals.
  pub card: Token<Color, N>,
  /// Hairline borders and separators.
  pub border: Token<Color, N>,
  /// The focused control's border ink.
  pub ring: Token<Color, N>,
  /// The ink of a link.
  pub link: Token<Color, N>,
  /// The main action: a high-contrast surface.
  pub primary: SimpleLook<N>,
  /// A supporting action: a subdued surface.
  pub secondary: SimpleLook<N>,
  /// A hairline-bordered action on the ambient background.
  pub outline: SimpleLook<N>,
  /// A bare action, surfacing only under the pointer.
  pub ghost: SimpleLook<N>,
  /// A dangerous action.
  pub destructive: SimpleLook<N>,
}

impl<N: Scalar> SimplePalette<N> {
  /// The page background.
  pub const BACKGROUND: TokenRef<Color> = TokenRef::new("simple-background");
  /// Hairline borders and separators.
  pub const BORDER: TokenRef<Color> = TokenRef::new("simple-border");
  /// A raised surface.
  pub const CARD: TokenRef<Color> = TokenRef::new("simple-card");
  /// The dangerous action's border.
  pub const DESTRUCTIVE_BORDER: TokenRef<Color> = SimpleVariant::Destructive.border();
  /// The dangerous action's focus surface.
  pub const DESTRUCTIVE_FOCUS: TokenRef<Color> = SimpleVariant::Destructive.focus();
  /// The dangerous action's hover surface.
  pub const DESTRUCTIVE_HOVER: TokenRef<Color> = SimpleVariant::Destructive.hover();
  /// The dangerous action's ink.
  pub const DESTRUCTIVE_INK: TokenRef<Color> = SimpleVariant::Destructive.ink();
  /// The dangerous action's surface.
  pub const DESTRUCTIVE_SURFACE: TokenRef<Color> = SimpleVariant::Destructive.surface();
  /// The default text ink.
  pub const FOREGROUND: TokenRef<Color> = TokenRef::new("simple-foreground");
  /// The bare action's border.
  pub const GHOST_BORDER: TokenRef<Color> = SimpleVariant::Ghost.border();
  /// The bare action's focus surface.
  pub const GHOST_FOCUS: TokenRef<Color> = SimpleVariant::Ghost.focus();
  /// The bare action's hover surface.
  pub const GHOST_HOVER: TokenRef<Color> = SimpleVariant::Ghost.hover();
  /// The bare action's ink.
  pub const GHOST_INK: TokenRef<Color> = SimpleVariant::Ghost.ink();
  /// The bare action's surface.
  pub const GHOST_SURFACE: TokenRef<Color> = SimpleVariant::Ghost.surface();
  /// The ink of a link.
  pub const LINK: TokenRef<Color> = TokenRef::new("simple-link");
  /// De-emphasized text.
  pub const MUTED_FOREGROUND: TokenRef<Color> = TokenRef::new("simple-muted-foreground");
  /// The hairline-bordered action's border.
  pub const OUTLINE_BORDER: TokenRef<Color> = SimpleVariant::Outline.border();
  /// The hairline-bordered action's focus surface.
  pub const OUTLINE_FOCUS: TokenRef<Color> = SimpleVariant::Outline.focus();
  /// The hairline-bordered action's hover surface.
  pub const OUTLINE_HOVER: TokenRef<Color> = SimpleVariant::Outline.hover();
  /// The hairline-bordered action's ink.
  pub const OUTLINE_INK: TokenRef<Color> = SimpleVariant::Outline.ink();
  /// The hairline-bordered action's surface.
  pub const OUTLINE_SURFACE: TokenRef<Color> = SimpleVariant::Outline.surface();
  /// The main action's border.
  pub const PRIMARY_BORDER: TokenRef<Color> = SimpleVariant::Primary.border();
  /// The main action's focus surface.
  pub const PRIMARY_FOCUS: TokenRef<Color> = SimpleVariant::Primary.focus();
  /// The main action's hover surface.
  pub const PRIMARY_HOVER: TokenRef<Color> = SimpleVariant::Primary.hover();
  /// The main action's ink.
  pub const PRIMARY_INK: TokenRef<Color> = SimpleVariant::Primary.ink();
  /// The main action's surface.
  pub const PRIMARY_SURFACE: TokenRef<Color> = SimpleVariant::Primary.surface();
  /// The focused control's border ink.
  pub const RING: TokenRef<Color> = TokenRef::new("simple-ring");
  /// The supporting action's border.
  pub const SECONDARY_BORDER: TokenRef<Color> = SimpleVariant::Secondary.border();
  /// The supporting action's focus surface.
  pub const SECONDARY_FOCUS: TokenRef<Color> = SimpleVariant::Secondary.focus();
  /// The supporting action's hover surface.
  pub const SECONDARY_HOVER: TokenRef<Color> = SimpleVariant::Secondary.hover();
  /// The supporting action's ink.
  pub const SECONDARY_INK: TokenRef<Color> = SimpleVariant::Secondary.ink();
  /// The supporting action's surface.
  pub const SECONDARY_SURFACE: TokenRef<Color> = SimpleVariant::Secondary.surface();

  /// The look of `variant`.
  #[inline(always)]
  pub const fn look(&self, variant: SimpleVariant) -> &SimpleLook<N> {
    match variant {
      SimpleVariant::Primary => &self.primary,
      SimpleVariant::Secondary => &self.secondary,
      SimpleVariant::Outline => &self.outline,
      SimpleVariant::Ghost => &self.ghost,
      SimpleVariant::Destructive => &self.destructive,
    }
  }

  /// Sets the page background.
  #[inline(always)]
  pub fn set_background(mut self, background: impl Into<Definition<Color, N>>) -> Self {
    self.background = self.background.redefine(background);

    self
  }

  /// Sets the default text ink.
  #[inline(always)]
  pub fn set_foreground(mut self, foreground: impl Into<Definition<Color, N>>) -> Self {
    self.foreground = self.foreground.redefine(foreground);

    self
  }

  /// Sets the de-emphasized text ink.
  #[inline(always)]
  pub fn set_muted_foreground(mut self, muted_foreground: impl Into<Definition<Color, N>>) -> Self {
    self.muted_foreground = self.muted_foreground.redefine(muted_foreground);

    self
  }

  /// Sets the raised surface.
  #[inline(always)]
  pub fn set_card(mut self, card: impl Into<Definition<Color, N>>) -> Self {
    self.card = self.card.redefine(card);

    self
  }

  /// Sets the hairline border and separator ink.
  #[inline(always)]
  pub fn set_border(mut self, border: impl Into<Definition<Color, N>>) -> Self {
    self.border = self.border.redefine(border);

    self
  }

  /// Sets the focused control's border ink.
  #[inline(always)]
  pub fn set_ring(mut self, ring: impl Into<Definition<Color, N>>) -> Self {
    self.ring = self.ring.redefine(ring);

    self
  }

  /// Sets the ink of a link.
  #[inline(always)]
  pub fn set_link(mut self, link: impl Into<Definition<Color, N>>) -> Self {
    self.link = self.link.redefine(link);

    self
  }

  /// Sets the main action's colors.
  #[inline(always)]
  pub fn set_primary(mut self, primary: SimpleLook<N>) -> Self {
    self.primary = primary;

    self
  }

  /// Sets the supporting action's colors.
  #[inline(always)]
  pub fn set_secondary(mut self, secondary: SimpleLook<N>) -> Self {
    self.secondary = secondary;

    self
  }

  /// Sets the hairline-bordered action's colors.
  #[inline(always)]
  pub fn set_outline(mut self, outline: SimpleLook<N>) -> Self {
    self.outline = outline;

    self
  }

  /// Sets the bare action's colors.
  #[inline(always)]
  pub fn set_ghost(mut self, ghost: SimpleLook<N>) -> Self {
    self.ghost = ghost;

    self
  }

  /// Sets the dangerous action's colors.
  #[inline(always)]
  pub fn set_destructive(mut self, destructive: SimpleLook<N>) -> Self {
    self.destructive = destructive;

    self
  }
}

impl<N: Scalar> Default for SimplePalette<N> {
  /// The palette the Simple components paint with unless an app hands over its own:
  /// dark ink on a near-white page, each token carrying the value it takes under the
  /// dark scheme.
  fn default() -> Self {
    let page = Color::srgb(0.98, 0.98, 0.98);
    let page_dark = Color::srgb(0.035, 0.035, 0.043);
    let ink = Color::srgb(0.035, 0.035, 0.043);
    let ink_dark = Color::srgb(0.98, 0.98, 0.98);
    let muted = Color::srgb(0.443, 0.443, 0.478);
    let muted_dark = Color::srgb(0.631, 0.631, 0.667);
    let raised = Color::srgb(1., 1., 1.);
    let raised_dark = Color::srgb(0.094, 0.094, 0.106);
    let edge = Color::srgb(0.894, 0.894, 0.906);
    let edge_dark = Color::srgb(0.153, 0.153, 0.165);
    let subdued = Color::srgb(0.957, 0.957, 0.961);
    let subdued_dark = Color::srgb(0.247, 0.247, 0.275);
    let anchor = Color::srgb(0.145, 0.388, 0.922);
    let anchor_dark = Color::srgb(0.376, 0.647, 0.98);

    Self {
      background: Token::new(Self::BACKGROUND.name().as_str(), pair(page, page_dark)),
      foreground: Token::new(Self::FOREGROUND.name().as_str(), pair(ink, ink_dark)),
      muted_foreground: Token::new(
        Self::MUTED_FOREGROUND.name().as_str(),
        pair(muted, muted_dark),
      ),
      card: Token::new(Self::CARD.name().as_str(), pair(raised, raised_dark)),
      border: Token::new(Self::BORDER.name().as_str(), pair(edge, edge_dark)),
      ring: Token::new(Self::RING.name().as_str(), pair(muted, muted_dark)),
      link: Token::new(Self::LINK.name().as_str(), pair(anchor, anchor_dark)),
      primary: SimpleLook::new(
        SimpleVariant::Primary,
        pair(raised_dark, ink_dark),
        pair(ink_dark, raised_dark),
      )
      .set_hover(pair(edge_dark, edge))
      .set_focus(pair(subdued_dark, Color::srgb(0.831, 0.831, 0.855))),
      secondary: SimpleLook::new(
        SimpleVariant::Secondary,
        pair(subdued, edge_dark),
        pair(ink, ink_dark),
      )
      .set_hover(pair(edge, subdued_dark))
      .set_focus(pair(
        Color::srgb(0.831, 0.831, 0.847),
        Color::srgb(0.322, 0.322, 0.357),
      )),
      outline: SimpleLook::new(
        SimpleVariant::Outline,
        Color::TRANSPARENT,
        pair(ink, ink_dark),
      )
      .set_border(pair(edge, edge_dark))
      .set_hover(pair(subdued, edge_dark))
      .set_focus(pair(edge, subdued_dark)),
      ghost: SimpleLook::new(
        SimpleVariant::Ghost,
        Color::TRANSPARENT,
        pair(ink, ink_dark),
      )
      .set_hover(pair(subdued, edge_dark))
      .set_focus(pair(edge, subdued_dark)),
      destructive: SimpleLook::new(
        SimpleVariant::Destructive,
        Color::srgb(0.863, 0.149, 0.149),
        Color::srgb(0.98, 0.98, 0.98),
      )
      .set_hover(Color::srgb(0.725, 0.11, 0.11))
      .set_focus(Color::srgb(0.6, 0.106, 0.106)),
    }
  }
}

/// A definition taking `dark`'s value under the dark scheme and `light`'s otherwise.
fn pair<N: Scalar>(light: Color, dark: Color) -> Definition<Color, N> {
  Definition::from(light).when(Media::Dark, dark)
}

impl<N: Scalar, V: From<Definition<Color, N>>> TokenSet<V> for SimplePalette<N> {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    let mut definitions = vec![
      self.background.define(),
      self.foreground.define(),
      self.muted_foreground.define(),
      self.card.define(),
      self.border.define(),
      self.ring.define(),
      self.link.define(),
    ];

    for look in [
      &self.primary,
      &self.secondary,
      &self.outline,
      &self.ghost,
      &self.destructive,
    ] {
      look.define_into(&mut definitions);
    }

    definitions.extend([
      alias::<N, V>(typography::INK, Self::FOREGROUND),
      alias::<N, V>(typography::LINK_INK, Self::LINK),
      alias::<N, V>(typography::SELECTION_INK, Self::BACKGROUND),
      alias::<N, V>(typography::SELECTION_SURFACE, Self::FOREGROUND),
    ]);

    definitions
  }
}

/// The definition of `token` as a reference to `target`, erased into `V`.
fn alias<N: Scalar, V: From<Definition<Color, N>>>(
  token: TokenRef<Color>,
  target: TokenRef<Color>,
) -> (TokenName, V) {
  (token.name(), V::from(Definition::<Color, N>::from(target)))
}
