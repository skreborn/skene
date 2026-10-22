//! A hash over a style's values, for a platform caching what it compiled from them.

use std::hash::{Hash, Hasher};

use super::{
  BoxDynamics,
  BoxStyle,
  BoxVisitor,
  Conditional,
  IntoBoxValue,
  IntoRunValue,
  KeptLeaf,
  Keyframe,
  Plain,
  PropertyKey,
  PropertyName,
  RunDynamics,
  RunStyle,
  RunVisitor,
  Scalar,
  Static,
  StaticLeaf,
  StyleExtension,
};
use crate::spine::{
  Animation,
  Arrangement,
  AspectRatio,
  Axes,
  Color,
  Dimension,
  Easing,
  Flexibility,
  GridAreas,
  GridFlow,
  GridPlacement,
  GridTemplate,
  GridTrack,
  Inset,
  ItemAlignment,
  Iterations,
  Length,
  Line,
  MouseCursor,
  Overflow,
  Spacing,
  StyleProperty,
  TemplateEntry,
  TextDecoration,
  TextDecorationStyle,
  TrackLimit,
  Transition,
  Wrap,
};

/// A value that feeds a hasher so equal values hash alike, lengths included.
///
/// Every type a style property holds has it; a platform keys its compiled output on the
/// resulting hash.
#[diagnostic::on_unimplemented(
  message = "`{Self}` has no fingerprint",
  label = "a style property's type implements `Fingerprint` so a platform can key what it compiled on the style"
)]
pub trait Fingerprint {
  /// Feeds this value to `state`.
  fn fingerprint<H: Hasher>(&self, state: &mut H);
}

/// The types whose `Hash` is already value-exact.
macro_rules! hashed {
  ($($type:ty),+ $(,)?) => {$(
    impl Fingerprint for $type {
      fn fingerprint<H: Hasher>(&self, state: &mut H) {
        self.hash(state);
      }
    }
  )+};
}

hashed!(
  Color,
  MouseCursor,
  TextDecoration,
  TextDecorationStyle,
  Wrap,
  Arrangement,
  ItemAlignment,
  GridFlow,
  GridPlacement,
  GridAreas,
  Overflow,
  Iterations,
  bool,
  u16,
);

impl<P: PropertyName> Fingerprint for StyleProperty<P> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.hash(state);
  }
}

impl<N: Scalar> Fingerprint for N {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    state.write_u64(self.hash_bits());
  }
}

impl Fingerprint for Length {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.0.fingerprint(state);
  }
}

impl<T: Fingerprint> Fingerprint for Axes<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.horizontal.fingerprint(state);
    self.vertical.fingerprint(state);
  }
}

impl Fingerprint for AspectRatio {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.0.fingerprint(state);
  }
}

impl<T: Fingerprint> Fingerprint for Option<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    match self {
      Some(value) => {
        state.write_u8(1);
        value.fingerprint(state);
      }
      None => state.write_u8(0),
    }
  }
}

impl<T: Fingerprint> Fingerprint for Vec<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    state.write_usize(self.len());

    for item in self {
      item.fingerprint(state);
    }
  }
}

impl<T: Fingerprint> Fingerprint for [T] {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    state.write_usize(self.len());

    for item in self {
      item.fingerprint(state);
    }
  }
}

impl<N: Scalar> Fingerprint for Dimension<N> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::Length(length) => state.write_u64(length.hash_bits()),
      Self::Fraction(fraction) => fraction.fingerprint(state),
      Self::Auto => {}
    }
  }
}

impl<T: Fingerprint> Fingerprint for Inset<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.top.fingerprint(state);
    self.end.fingerprint(state);
    self.bottom.fingerprint(state);
    self.start.fingerprint(state);
  }
}

impl<T: Fingerprint> Fingerprint for Spacing<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.horizontal.fingerprint(state);
    self.vertical.fingerprint(state);
  }
}

impl<N: Scalar> Fingerprint for Flexibility<N> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.initial.fingerprint(state);
    self.grow_factor.fingerprint(state);
    self.shrink_factor.fingerprint(state);
  }
}

impl<N: Scalar> Fingerprint for TrackLimit<N> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::Length(length) => state.write_u64(length.hash_bits()),
      Self::Fraction(value) | Self::Flex(value) => value.fingerprint(state),
      Self::Auto | Self::MinContent | Self::MaxContent => {}
    }
  }
}

impl<N: Scalar> Fingerprint for GridTrack<N> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::Length(length) | Self::FitContent(length) => state.write_u64(length.hash_bits()),
      Self::Fraction(value) | Self::Flex(value) => value.fingerprint(state),
      Self::Auto | Self::MinContent | Self::MaxContent => {}
      Self::Between(min, max) | Self::Adaptive(min, max) => {
        min.fingerprint(state);
        max.fingerprint(state);
      }
    }
  }
}

impl<N: Scalar> Fingerprint for GridTemplate<N> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    state.write_usize(self.entries().len());

    for entry in self.entries() {
      std::mem::discriminant(entry).hash(state);

      match entry {
        TemplateEntry::Track(track) => track.fingerprint(state),
        TemplateEntry::Line(name) => name.hash(state),
      }
    }
  }
}

impl<T: Fingerprint> Fingerprint for Line<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.start.fingerprint(state);
    self.end.fingerprint(state);
  }
}

impl<T: Fingerprint> Fingerprint for StaticLeaf<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::Fixed(value) => value.fingerprint(state),
      Self::Token(name) => name.hash(state),
      Self::Expr(expr) => expr.hash(state),
    }
  }
}

impl<T: Fingerprint> Fingerprint for KeptLeaf<T> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    match self {
      Self::Static(leaf) => leaf.fingerprint(state),
      Self::Slot(slot) => slot.hash(state),
    }
  }
}

impl<L: Fingerprint, C: Hash> Fingerprint for Conditional<L, C> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.base().fingerprint(state);
    state.write_usize(self.variants().len());

    for (condition, leaf) in self.variants() {
      condition.hash(state);
      leaf.fingerprint(state);
    }
  }
}

impl Fingerprint for Easing {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    std::mem::discriminant(self).hash(state);

    if let Self::CubicBezier(x1, y1, x2, y2) = self {
      x1.fingerprint(state);
      y1.fingerprint(state);
      x2.fingerprint(state);
      y2.fingerprint(state);
    }
  }
}

impl<P: PropertyName> Fingerprint for Transition<P> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.property.fingerprint(state);
    self.duration.fingerprint(state);
    self.delay.fingerprint(state);
    self.easing.fingerprint(state);
    self.respects_reduced_motion.fingerprint(state);
  }
}

impl<S: Fingerprint> Fingerprint for Animation<S> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    state.write_usize(self.keyframes.len());

    for (percent, style) in &self.keyframes {
      percent.fingerprint(state);
      style.fingerprint(state);
    }

    self.duration.fingerprint(state);
    self.delay.fingerprint(state);
    self.easing.fingerprint(state);
    self.iterations.fingerprint(state);
    self.respects_reduced_motion.fingerprint(state);
    self.alternate.fingerprint(state);
  }
}

/// Feeds every set field of a style to the hasher, with its property.
struct Feed<'a, H> {
  state: &'a mut H,
}

impl<N: Scalar, X: StyleExtension, H: Hasher> BoxVisitor<N, Plain, X> for Feed<'_, H> {
  fn value<T: IntoBoxValue<N, X>>(&mut self, key: PropertyKey<X::Property>, value: &Option<T>) {
    key.hash(self.state);
    value.fingerprint(self.state);
  }
}

impl<
  N: Scalar,
  C: Clone + PartialEq + Hash + std::fmt::Debug + 'static,
  X: StyleExtension,
  H: Hasher,
> BoxVisitor<N, Static<C>, X> for Feed<'_, H>
{
  fn value<T: IntoBoxValue<N, X>>(
    &mut self,
    key: PropertyKey<X::Property>,
    value: &Option<Conditional<KeptLeaf<T>, C>>,
  ) {
    key.hash(self.state);
    value.fingerprint(self.state);
  }
}

impl<N: Scalar, X: StyleExtension, H: Hasher> BoxVisitor<N, Keyframe, X> for Feed<'_, H> {
  fn value<T: IntoBoxValue<N, X>>(
    &mut self,
    key: PropertyKey<X::Property>,
    value: &Option<StaticLeaf<T>>,
  ) {
    key.hash(self.state);
    value.fingerprint(self.state);
  }
}

impl<X: StyleExtension, H: Hasher> RunVisitor<Plain, X> for Feed<'_, H> {
  fn value<T: IntoRunValue<X>>(&mut self, key: PropertyKey<X::Property>, value: &Option<T>) {
    key.hash(self.state);
    value.fingerprint(self.state);
  }
}

impl<X: StyleExtension, H: Hasher> RunVisitor<Keyframe, X> for Feed<'_, H> {
  fn value<T: IntoRunValue<X>>(
    &mut self,
    key: PropertyKey<X::Property>,
    value: &Option<StaticLeaf<T>>,
  ) {
    key.hash(self.state);
    value.fingerprint(self.state);
  }
}

impl<C: Clone + PartialEq + Hash + std::fmt::Debug + 'static, X: StyleExtension, H: Hasher>
  RunVisitor<Static<C>, X> for Feed<'_, H>
{
  fn value<T: IntoRunValue<X>>(
    &mut self,
    key: PropertyKey<X::Property>,
    value: &Option<Conditional<KeptLeaf<T>, C>>,
  ) {
    key.hash(self.state);
    value.fingerprint(self.state);
  }
}

impl<N: Scalar, X: StyleExtension> Fingerprint for BoxStyle<N, Plain, X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.visit(&mut Feed { state });
  }
}

impl<N: Scalar, C: Clone + PartialEq + Hash + std::fmt::Debug + 'static, X: StyleExtension>
  Fingerprint for BoxStyle<N, Static<C>, X>
{
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.visit(&mut Feed { state });
  }
}

impl<N: Scalar, X: StyleExtension> Fingerprint for BoxStyle<N, Keyframe, X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.visit(&mut Feed { state });
  }
}

impl<X: StyleExtension> Fingerprint for RunStyle<Keyframe, X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.visit(&mut Feed { state });
  }
}

impl<X: StyleExtension> Fingerprint for RunStyle<Plain, X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.visit(&mut Feed { state });
  }
}

impl<C: Clone + PartialEq + Hash + std::fmt::Debug + 'static, X: StyleExtension> Fingerprint
  for RunStyle<Static<C>, X>
{
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.visit(&mut Feed { state });
  }
}

impl<N: Scalar, X: StyleExtension> Fingerprint for BoxDynamics<N, X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.transitions.fingerprint(state);
    self.animations.fingerprint(state);
  }
}

impl<X: StyleExtension> Fingerprint for RunDynamics<X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.transitions.fingerprint(state);
    self.animations.fingerprint(state);
  }
}

#[cfg(test)]
mod tests {
  use std::hash::Hasher;

  use rustc_hash::FxHasher;

  use super::Fingerprint;
  use crate::spine::{
    BoxStyle,
    Color,
    Condition,
    Declared,
    Dimension,
    Media,
    State,
    TokenRef,
    Value,
  };

  fn hash_of(value: &impl Fingerprint) -> u64 {
    let mut hasher = FxHasher::default();

    value.fingerprint(&mut hasher);

    hasher.finish()
  }

  /// Equal static styles hash alike, and a changed leaf or condition changes the hash.
  #[test]
  fn static_styles_hash_by_value() {
    let style = |dark: Color| {
      BoxStyle::<f32, Declared<Condition<f32>>> {
        width: Some(Value::from(Dimension::from(10.))),
        background: Some(
          TokenRef::<Color>::new("card")
            .when(Media::Dark.into(), dark)
            .when(State::Hovered.into(), Color::TRANSPARENT),
        ),
        ..Default::default()
      }
      .split()
      .0
    };
    let black = Color::srgb(0., 0., 0.);

    assert_eq!(hash_of(&style(black)), hash_of(&style(black)));
    assert_ne!(hash_of(&style(black)), hash_of(&style(Color::TRANSPARENT)));
  }

  /// Both zeros and every `None` fingerprint alike, as they compare alike.
  #[test]
  fn zeros_and_absences_hash_alike() {
    assert_eq!(hash_of(&0f32), hash_of(&-0f32));
    assert_eq!(hash_of(&None::<f32>), hash_of(&None::<f32>));
    assert_ne!(hash_of(&None::<f32>), hash_of(&Some(0f32)));
  }
}
