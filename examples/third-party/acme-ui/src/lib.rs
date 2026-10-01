//! # acme-ui
//!
//! A third-party component library: one palette and one component, written once over
//! [`skene_foundation`] against any [`Platform`], exactly as the built-ins are. A facade's app
//! names the platform (`AcmeBadge<ActivePlatform>`); the crate names none.

use std::marker::PhantomData;

use skene_foundation::{
  AnyKind,
  Color,
  Container,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  Definition,
  HasChild,
  Inset,
  Media,
  NodeId,
  Platform,
  RunBase,
  Scalar,
  Text,
  ThemedIn,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  View,
};

/// The Acme tokens: a badge's surface and its ink, over the facade's scalar `N`.
///
/// A palette is a value; hand it to a `Theme` and every badge beneath reads it by
/// name. Each definition carries its own conditions, the dark scheme among them.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct AcmePalette<N: Scalar> {
  /// The badge's surface.
  pub badge: Token<Color, N>,
  /// The ink on the badge.
  pub badge_ink: Token<Color, N>,
}

impl<N: Scalar> AcmePalette<N> {
  /// The badge's surface.
  pub const BADGE: TokenRef<Color> = TokenRef::new("acme-badge");
  /// The ink on the badge.
  pub const BADGE_INK: TokenRef<Color> = TokenRef::new("acme-badge-ink");

  /// Sets the badge's surface.
  #[inline(always)]
  pub fn set_badge(mut self, badge: impl Into<Definition<Color, N>>) -> Self {
    self.badge = self.badge.redefine(badge);

    self
  }

  /// Sets the ink on the badge.
  #[inline(always)]
  pub fn set_badge_ink(mut self, ink: impl Into<Definition<Color, N>>) -> Self {
    self.badge_ink = self.badge_ink.redefine(ink);

    self
  }
}

impl<N: Scalar> Default for AcmePalette<N> {
  /// Acme's amber on a dark ink, inverted under the dark scheme.
  fn default() -> Self {
    let amber = Color::srgb(0.98, 0.75, 0.18);
    let ink = Color::srgb(0.12, 0.09, 0.02);

    Self {
      badge: Token::new(
        Self::BADGE.name().as_str(),
        Definition::from(amber).when(Media::Dark, ink),
      ),
      badge_ink: Token::new(
        Self::BADGE_INK.name().as_str(),
        Definition::from(ink).when(Media::Dark, amber),
      ),
    }
  }
}

impl<N: Scalar, V: From<Definition<Color, N>>> TokenSet<V> for AcmePalette<N> {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.badge.define(), self.badge_ink.define()]
  }
}

/// A length unit the badge knows its shape in.
#[diagnostic::on_unimplemented(
  message = "the Acme badge has no shape in `{Self}`",
  label = "an Acme view is drawn in logical pixels (`f32`) or cells (`u32`)"
)]
pub trait AcmeUnit: Scalar {
  /// The badge's padding around its label.
  const BADGE_INSET: Inset<Self>;
}

impl AcmeUnit for f32 {
  const BADGE_INSET: Inset<Self> = Inset::symmetric(8., 2.);
}

impl AcmeUnit for u32 {
  const BADGE_INSET: Inset<Self> = Inset::symmetric(1, 0);
}

/// A short label on the Acme surface: rounded where the medium rounds corners, square
/// where it has none, padded by the unit's [`AcmeUnit::BADGE_INSET`].
#[derive(Debug)]
#[must_use]
pub struct AcmeBadge<P: Platform> {
  text: String,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> AcmeBadge<P> {
  /// A badge reading `text`.
  #[inline(always)]
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      text: text.into(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform> AnyKind for AcmeBadge<P> {}

impl<P: Platform> View<P> for AcmeBadge<P>
where
  P::Scalar: AcmeUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    // Registers the palette's defaults at the root when no theme is above.
    let _ = <AcmePalette<P::Scalar> as ThemedIn<P>>::current();

    Container::<P>::new()
      .set_padding(P::Scalar::BADGE_INSET)
      .set_color(AcmePalette::<P::Scalar>::BADGE)
      .set_corner_radius(6.)
      .set_child(Text::new(self.text).set_color(AcmePalette::<P::Scalar>::BADGE_INK))
      .build()
  }
}
