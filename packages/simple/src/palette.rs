//! The Simple design language's tokens: the palette as a token set, the emphasis
//! looks, and the default every facade starts from.

use skene_foundation::{
  Color,
  Definition,
  Media,
  Scalar,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  typography,
};

/// A Simple action's emphasis, selecting its surface and ink.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum SimpleVariant {
  /// The main action: a high-contrast surface.
  #[default]
  Primary,
  /// A supporting action: a subdued surface.
  Secondary,
  /// A hairline-bordered action on the ambient background.
  Outline,
  /// A bare action, surfacing only under the pointer.
  Ghost,
  /// A dangerous action: the destructive surface.
  Destructive,
}

/// One emphasis variant's colors: the surface it paints, the ink on that surface, and
/// the surfaces its interaction states paint instead.
///
/// Each color is a token named for the variant, so a consumer emits the name and the
/// palette's default, or a theme of the palette above it, supplies the value. A `None`
/// border says the variant is filled rather than bordered: the palette then defines the
/// variant's border token as transparent, so a Simple preset draws a transparent hairline
/// and switching variants shifts nothing.
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
      surface: Token::new(SimplePalette::<N>::surface_of(variant), surface.clone()),
      ink: Token::new(SimplePalette::<N>::ink_of(variant), ink),
      border: None,
      hover: Token::new(SimplePalette::<N>::hover_of(variant), surface.clone()),
      focus: Token::new(SimplePalette::<N>::focus_of(variant), surface),
      variant,
    }
  }

  /// The variant the look colors.
  #[inline(always)]
  pub const fn variant(&self) -> SimpleVariant {
    self.variant
  }

  /// Sets the action's surface.
  pub fn set_surface(mut self, surface: impl Into<Definition<Color, N>>) -> Self {
    self.surface = self.surface.redefine(surface);

    self
  }

  /// Sets the ink on the action's surface.
  pub fn set_ink(mut self, ink: impl Into<Definition<Color, N>>) -> Self {
    self.ink = self.ink.redefine(ink);

    self
  }

  /// Gives the action a hairline border, making it a bordered variant.
  pub fn set_border(mut self, border: impl Into<Definition<Color, N>>) -> Self {
    self.border = Some(Token::new(
      SimplePalette::<N>::border_of(self.variant),
      border,
    ));

    self
  }

  /// Sets the surface shown under the pointer.
  pub fn set_hover(mut self, hover: impl Into<Definition<Color, N>>) -> Self {
    self.hover = self.hover.redefine(hover);

    self
  }

  /// Sets the surface shown while focused.
  pub fn set_focus(mut self, focus: impl Into<Definition<Color, N>>) -> Self {
    self.focus = self.focus.redefine(focus);

    self
  }

  /// The look with its tokens named for `variant`, its definitions kept.
  fn named_for(self, variant: SimpleVariant) -> Self {
    let rename =
      |token: &Token<Color, N>, name: TokenRef<Color>| Token::new(name, token.definition().clone());

    Self {
      surface: rename(&self.surface, SimplePalette::<N>::surface_of(variant)),
      ink: rename(&self.ink, SimplePalette::<N>::ink_of(variant)),
      border: self
        .border
        .as_ref()
        .map(|border| rename(border, SimplePalette::<N>::border_of(variant))),
      hover: rename(&self.hover, SimplePalette::<N>::hover_of(variant)),
      focus: rename(&self.focus, SimplePalette::<N>::focus_of(variant)),
      variant,
    }
  }

  /// Appends the look's definitions, erased into `V`.
  fn define_into<V: From<Definition<Color, N>>>(&self, definitions: &mut Vec<(TokenName, V)>) {
    definitions.push(self.surface.define());
    definitions.push(self.ink.define());

    definitions.push(self.border.as_ref().map_or_else(
      || {
        (
          SimplePalette::<N>::border_of(self.variant).name(),
          V::from(Definition::from(Color::TRANSPARENT)),
        )
      },
      Token::define,
    ));

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
/// The palette also defines the [`typography`] tokens as aliases—the ink is the
/// foreground, the link ink the link color, selected text the background on the
/// foreground—so a plain text wherever the palette is defined reads as one design with
/// the controls. Use the tokens for the chrome around the components too.
///
/// This type is the Simple library's, not the framework's: a third-party component
/// library defines its own struct of [`Token`]s.
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
  /// The surface of a control that cannot be operated.
  pub disabled_surface: Token<Color, N>,
  /// The ink on that surface.
  pub disabled_ink: Token<Color, N>,
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
  pub const BACKGROUND: TokenRef<Color> = TokenRef::of::<Self, N>("simple-background");
  /// Hairline borders and separators.
  pub const BORDER: TokenRef<Color> = TokenRef::of::<Self, N>("simple-border");
  /// A raised surface.
  pub const CARD: TokenRef<Color> = TokenRef::of::<Self, N>("simple-card");
  /// The dangerous action's border.
  pub const DESTRUCTIVE_BORDER: TokenRef<Color> = Self::border_of(SimpleVariant::Destructive);
  /// The dangerous action's focus surface.
  pub const DESTRUCTIVE_FOCUS: TokenRef<Color> = Self::focus_of(SimpleVariant::Destructive);
  /// The dangerous action's hover surface.
  pub const DESTRUCTIVE_HOVER: TokenRef<Color> = Self::hover_of(SimpleVariant::Destructive);
  /// The dangerous action's ink.
  pub const DESTRUCTIVE_INK: TokenRef<Color> = Self::ink_of(SimpleVariant::Destructive);
  /// The dangerous action's surface.
  pub const DESTRUCTIVE_SURFACE: TokenRef<Color> = Self::surface_of(SimpleVariant::Destructive);
  /// The ink of a control that cannot be operated.
  pub const DISABLED_INK: TokenRef<Color> = TokenRef::of::<Self, N>("simple-disabled-ink");
  /// The surface of a control that cannot be operated.
  pub const DISABLED_SURFACE: TokenRef<Color> = TokenRef::of::<Self, N>("simple-disabled-surface");
  /// The default text ink.
  pub const FOREGROUND: TokenRef<Color> = TokenRef::of::<Self, N>("simple-foreground");
  /// The bare action's border.
  pub const GHOST_BORDER: TokenRef<Color> = Self::border_of(SimpleVariant::Ghost);
  /// The bare action's focus surface.
  pub const GHOST_FOCUS: TokenRef<Color> = Self::focus_of(SimpleVariant::Ghost);
  /// The bare action's hover surface.
  pub const GHOST_HOVER: TokenRef<Color> = Self::hover_of(SimpleVariant::Ghost);
  /// The bare action's ink.
  pub const GHOST_INK: TokenRef<Color> = Self::ink_of(SimpleVariant::Ghost);
  /// The bare action's surface.
  pub const GHOST_SURFACE: TokenRef<Color> = Self::surface_of(SimpleVariant::Ghost);
  /// The ink of a link.
  pub const LINK: TokenRef<Color> = TokenRef::of::<Self, N>("simple-link");
  /// De-emphasized text.
  pub const MUTED_FOREGROUND: TokenRef<Color> = TokenRef::of::<Self, N>("simple-muted-foreground");
  /// The hairline-bordered action's border.
  pub const OUTLINE_BORDER: TokenRef<Color> = Self::border_of(SimpleVariant::Outline);
  /// The hairline-bordered action's focus surface.
  pub const OUTLINE_FOCUS: TokenRef<Color> = Self::focus_of(SimpleVariant::Outline);
  /// The hairline-bordered action's hover surface.
  pub const OUTLINE_HOVER: TokenRef<Color> = Self::hover_of(SimpleVariant::Outline);
  /// The hairline-bordered action's ink.
  pub const OUTLINE_INK: TokenRef<Color> = Self::ink_of(SimpleVariant::Outline);
  /// The hairline-bordered action's surface.
  pub const OUTLINE_SURFACE: TokenRef<Color> = Self::surface_of(SimpleVariant::Outline);
  /// The main action's border.
  pub const PRIMARY_BORDER: TokenRef<Color> = Self::border_of(SimpleVariant::Primary);
  /// The main action's focus surface.
  pub const PRIMARY_FOCUS: TokenRef<Color> = Self::focus_of(SimpleVariant::Primary);
  /// The main action's hover surface.
  pub const PRIMARY_HOVER: TokenRef<Color> = Self::hover_of(SimpleVariant::Primary);
  /// The main action's ink.
  pub const PRIMARY_INK: TokenRef<Color> = Self::ink_of(SimpleVariant::Primary);
  /// The main action's surface.
  pub const PRIMARY_SURFACE: TokenRef<Color> = Self::surface_of(SimpleVariant::Primary);
  /// The focused control's border ink.
  pub const RING: TokenRef<Color> = TokenRef::of::<Self, N>("simple-ring");
  /// The supporting action's border.
  pub const SECONDARY_BORDER: TokenRef<Color> = Self::border_of(SimpleVariant::Secondary);
  /// The supporting action's focus surface.
  pub const SECONDARY_FOCUS: TokenRef<Color> = Self::focus_of(SimpleVariant::Secondary);
  /// The supporting action's hover surface.
  pub const SECONDARY_HOVER: TokenRef<Color> = Self::hover_of(SimpleVariant::Secondary);
  /// The supporting action's ink.
  pub const SECONDARY_INK: TokenRef<Color> = Self::ink_of(SimpleVariant::Secondary);
  /// The supporting action's surface.
  pub const SECONDARY_SURFACE: TokenRef<Color> = Self::surface_of(SimpleVariant::Secondary);

  /// The surface token of `variant`.
  pub const fn surface_of(variant: SimpleVariant) -> TokenRef<Color> {
    TokenRef::of::<Self, N>(match variant {
      SimpleVariant::Primary => "simple-primary-surface",
      SimpleVariant::Secondary => "simple-secondary-surface",
      SimpleVariant::Outline => "simple-outline-surface",
      SimpleVariant::Ghost => "simple-ghost-surface",
      SimpleVariant::Destructive => "simple-destructive-surface",
    })
  }

  /// The ink token of `variant`.
  pub const fn ink_of(variant: SimpleVariant) -> TokenRef<Color> {
    TokenRef::of::<Self, N>(match variant {
      SimpleVariant::Primary => "simple-primary-ink",
      SimpleVariant::Secondary => "simple-secondary-ink",
      SimpleVariant::Outline => "simple-outline-ink",
      SimpleVariant::Ghost => "simple-ghost-ink",
      SimpleVariant::Destructive => "simple-destructive-ink",
    })
  }

  /// The border token of `variant`: a bordered look's hairline, and transparent for a
  /// filled one.
  pub const fn border_of(variant: SimpleVariant) -> TokenRef<Color> {
    TokenRef::of::<Self, N>(match variant {
      SimpleVariant::Primary => "simple-primary-border",
      SimpleVariant::Secondary => "simple-secondary-border",
      SimpleVariant::Outline => "simple-outline-border",
      SimpleVariant::Ghost => "simple-ghost-border",
      SimpleVariant::Destructive => "simple-destructive-border",
    })
  }

  /// The hover surface token of `variant`.
  pub const fn hover_of(variant: SimpleVariant) -> TokenRef<Color> {
    TokenRef::of::<Self, N>(match variant {
      SimpleVariant::Primary => "simple-primary-hover",
      SimpleVariant::Secondary => "simple-secondary-hover",
      SimpleVariant::Outline => "simple-outline-hover",
      SimpleVariant::Ghost => "simple-ghost-hover",
      SimpleVariant::Destructive => "simple-destructive-hover",
    })
  }

  /// The focus surface token of `variant`.
  pub const fn focus_of(variant: SimpleVariant) -> TokenRef<Color> {
    TokenRef::of::<Self, N>(match variant {
      SimpleVariant::Primary => "simple-primary-focus",
      SimpleVariant::Secondary => "simple-secondary-focus",
      SimpleVariant::Outline => "simple-outline-focus",
      SimpleVariant::Ghost => "simple-ghost-focus",
      SimpleVariant::Destructive => "simple-destructive-focus",
    })
  }

  /// The look of `variant`.
  #[must_use]
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
  pub fn set_background(mut self, background: impl Into<Definition<Color, N>>) -> Self {
    self.background = self.background.redefine(background);

    self
  }

  /// Sets the default text ink.
  pub fn set_foreground(mut self, foreground: impl Into<Definition<Color, N>>) -> Self {
    self.foreground = self.foreground.redefine(foreground);

    self
  }

  /// Sets the de-emphasized text ink.
  pub fn set_muted_foreground(mut self, muted_foreground: impl Into<Definition<Color, N>>) -> Self {
    self.muted_foreground = self.muted_foreground.redefine(muted_foreground);

    self
  }

  /// Sets the raised surface.
  pub fn set_card(mut self, card: impl Into<Definition<Color, N>>) -> Self {
    self.card = self.card.redefine(card);

    self
  }

  /// Sets the hairline border and separator ink.
  pub fn set_border(mut self, border: impl Into<Definition<Color, N>>) -> Self {
    self.border = self.border.redefine(border);

    self
  }

  /// Sets the focused control's border ink.
  pub fn set_ring(mut self, ring: impl Into<Definition<Color, N>>) -> Self {
    self.ring = self.ring.redefine(ring);

    self
  }

  /// Sets the ink of a link.
  pub fn set_link(mut self, link: impl Into<Definition<Color, N>>) -> Self {
    self.link = self.link.redefine(link);

    self
  }

  /// Sets the surface of a control that cannot be operated.
  pub fn set_disabled_surface(mut self, surface: impl Into<Definition<Color, N>>) -> Self {
    self.disabled_surface = self.disabled_surface.redefine(surface);

    self
  }

  /// Sets the ink of a control that cannot be operated.
  pub fn set_disabled_ink(mut self, ink: impl Into<Definition<Color, N>>) -> Self {
    self.disabled_ink = self.disabled_ink.redefine(ink);

    self
  }

  /// Sets the main action's colors, naming the look's tokens for the main action
  /// whichever variant it was made for.
  #[inline(always)]
  pub fn set_primary(mut self, primary: SimpleLook<N>) -> Self {
    self.primary = primary.named_for(SimpleVariant::Primary);

    self
  }

  /// Sets the supporting action's colors, naming the look's tokens for the supporting
  /// action whichever variant it was made for.
  #[inline(always)]
  pub fn set_secondary(mut self, secondary: SimpleLook<N>) -> Self {
    self.secondary = secondary.named_for(SimpleVariant::Secondary);

    self
  }

  /// Sets the hairline-bordered action's colors, naming the look's tokens for it
  /// whichever variant it was made for.
  #[inline(always)]
  pub fn set_outline(mut self, outline: SimpleLook<N>) -> Self {
    self.outline = outline.named_for(SimpleVariant::Outline);

    self
  }

  /// Sets the bare action's colors, naming the look's tokens for it whichever variant
  /// it was made for.
  #[inline(always)]
  pub fn set_ghost(mut self, ghost: SimpleLook<N>) -> Self {
    self.ghost = ghost.named_for(SimpleVariant::Ghost);

    self
  }

  /// Sets the dangerous action's colors, naming the look's tokens for it whichever
  /// variant it was made for.
  #[inline(always)]
  pub fn set_destructive(mut self, destructive: SimpleLook<N>) -> Self {
    self.destructive = destructive.named_for(SimpleVariant::Destructive);

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
      background: Token::new(Self::BACKGROUND, pair(page, page_dark)),
      foreground: Token::new(Self::FOREGROUND, pair(ink, ink_dark)),
      muted_foreground: Token::new(Self::MUTED_FOREGROUND, pair(muted, muted_dark)),
      card: Token::new(Self::CARD, pair(raised, raised_dark)),
      border: Token::new(Self::BORDER, pair(edge, edge_dark)),
      ring: Token::new(Self::RING, pair(muted, muted_dark)),
      link: Token::new(Self::LINK, pair(anchor, anchor_dark)),
      disabled_surface: Token::new(Self::DISABLED_SURFACE, pair(subdued, subdued_dark)),
      disabled_ink: Token::new(Self::DISABLED_INK, pair(muted, muted_dark)),
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
      self.disabled_surface.define(),
      self.disabled_ink.define(),
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

#[cfg(test)]
mod tests {
  use skene_foundation::{Color, Definition, TokenName, TokenSet};

  use super::{SimpleLook, SimplePalette, SimpleVariant};

  /// A look set into a slot takes the slot's token names whichever variant it was made
  /// for, so every variant is defined once.
  #[test]
  fn a_slot_names_the_look_it_holds() {
    let red = Color::srgb(1., 0., 0.);
    let palette =
      SimplePalette::<f32>::default().set_primary(SimpleLook::new(SimpleVariant::Ghost, red, red));
    let definitions: Vec<(TokenName, Definition<Color, f32>)> = palette.definitions();
    let defined = |name: TokenName| {
      definitions
        .iter()
        .filter(|(defined, _)| *defined == name)
        .count()
    };

    assert_eq!(palette.primary.variant(), SimpleVariant::Primary);
    assert_eq!(
      palette.primary.surface.name(),
      SimplePalette::<f32>::PRIMARY_SURFACE.name()
    );
    assert_eq!(defined(SimplePalette::<f32>::PRIMARY_SURFACE.name()), 1);
    assert_eq!(defined(SimplePalette::<f32>::GHOST_SURFACE.name()), 1);
  }

  /// Every variant's border token is defined once: a bordered look's as its hairline,
  /// and a filled look's as transparent.
  #[test]
  fn every_variant_defines_its_border() {
    let palette = SimplePalette::<f32>::default();
    let definitions: Vec<(TokenName, Definition<Color, f32>)> = palette.definitions();

    for variant in [
      SimpleVariant::Primary,
      SimpleVariant::Secondary,
      SimpleVariant::Outline,
      SimpleVariant::Ghost,
      SimpleVariant::Destructive,
    ] {
      let borders: Vec<_> = definitions
        .iter()
        .filter(|(name, _)| *name == SimplePalette::<f32>::border_of(variant).name())
        .map(|(_, definition)| definition)
        .collect();
      let expected = palette.look(variant).border.as_ref().map_or_else(
        || Definition::from(Color::TRANSPARENT),
        |border| border.definition().clone(),
      );

      assert_eq!(borders, [&expected], "{variant:?}");
    }
  }
}
