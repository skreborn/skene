//! # acme-ui
//!
//! A third-party component library: one palette and one component, written once over
//! [`skene_views`] against any [`Driver`], exactly as the built-ins are. A facade's app
//! names the driver (`AcmeBadge<ActiveDriver>`); the crate names none.

use std::marker::PhantomData;

use skene_core::{
  AnyKind,
  Color,
  Definition,
  Driver,
  HasChildren,
  Inset,
  Media,
  NodeId,
  Scalar,
  Token,
  TokenName,
  TokenRef,
  TokenSet,
  View,
};
use skene_views::{Container, CornerRadius, Text, ThemedIn};

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

/// A short label on the Acme surface: rounded where the medium rounds corners, square
/// where it has none.
#[must_use]
pub struct AcmeBadge<D: Driver> {
  text: String,
  driver: PhantomData<fn() -> D>,
}

impl<D: Driver> AcmeBadge<D> {
  /// A badge reading `text`.
  #[inline(always)]
  pub fn new(text: impl Into<String>) -> Self {
    Self {
      text: text.into(),
      driver: PhantomData,
    }
  }
}

impl<D: Driver> AnyKind for AcmeBadge<D> {}

impl<D: Driver> View<D> for AcmeBadge<D>
where
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    // Registers the palette's defaults at the root when no theme is above.
    let _ = <AcmePalette<D::Scalar> as ThemedIn<D>>::current();

    Container::<D>::new()
      .set_padding(Inset::symmetric(
        D::Scalar::from_f32(8.),
        D::Scalar::from_f32(2.),
      ))
      .set_color(AcmePalette::<D::Scalar>::BADGE)
      .set_corner_radius(6.)
      .set_child(Text::new(self.text).set_color(AcmePalette::<D::Scalar>::BADGE_INK))
      .build()
  }
}
