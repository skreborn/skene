//! The web medium's own style properties: rounded corners and type metrics.

use std::fmt::Debug;

use crate::{
  BoxFieldVisitor,
  IntoRunValue,
  Length,
  PropertyName,
  RunFieldVisitor,
  RunValue,
  Scalar,
  Shape,
  StyleExtension,
  StyleProperty,
};

/// The style extension of the web: a box rounds its corners, and a run carries
/// its type metrics.
///
/// Lengths are logical pixels; `font_weight` and `line_height` are plain numbers, the
/// latter a multiple of the font size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct WebStyle;

/// A web box's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct WebBoxFields<S: Shape> {
  /// The corner radius, in logical pixels.
  pub corner_radius: S::Of<Length>,
}

/// A web run's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct WebRunFields<S: Shape> {
  /// The font size, in logical pixels.
  pub font_size: S::Of<Length>,
  /// The font weight, `100` to `900`.
  pub font_weight: S::Of<f32>,
  /// The line height, as a multiple of the font size.
  pub line_height: S::Of<f32>,
  /// The extra space between letters, in logical pixels.
  pub letter_spacing: S::Of<Length>,
}

/// The names of the web medium's own properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub enum WebProperty {
  /// The corner radius.
  CornerRadius,
  /// The font size.
  FontSize,
  /// The font weight.
  FontWeight,
  /// The line height.
  LineHeight,
  /// The letter spacing.
  LetterSpacing,
}

impl WebProperty {
  /// Every property, in declaration order.
  pub const ALL: [Self; 5] = [
    Self::CornerRadius,
    Self::FontSize,
    Self::FontWeight,
    Self::LineHeight,
    Self::LetterSpacing,
  ];

  /// The property's name in the framework's vocabulary: the field it is set through.
  #[inline(always)]
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::CornerRadius => "corner_radius",
      Self::FontSize => "font_size",
      Self::FontWeight => "font_weight",
      Self::LineHeight => "line_height",
      Self::LetterSpacing => "letter_spacing",
    }
  }
}

impl PropertyName for WebProperty {
  #[inline(always)]
  fn name(&self) -> &'static str {
    Self::name(*self)
  }
}

impl From<WebProperty> for StyleProperty<WebProperty> {
  #[inline(always)]
  fn from(property: WebProperty) -> Self {
    Self::Extension(property)
  }
}

impl IntoRunValue<WebStyle> for f32 {
  #[inline(always)]
  fn into_run_value(self) -> RunValue<WebStyle> {
    RunValue::Extension(self)
  }

  #[inline(always)]
  fn from_run_value(value: RunValue<WebStyle>) -> Option<Self> {
    match value {
      RunValue::Extension(number) => Some(number),
      RunValue::Color(_)
      | RunValue::Length(_)
      | RunValue::Decoration(_)
      | RunValue::DecorationStyle(_) => None,
    }
  }
}

crate::extension_fields! {
  /// The web medium's own properties, addressed by type.
  pub mod web_fields for WebStyle, WebProperty;
  box: corner_radius: Length => CornerRadius;
  run: font_size: Length => FontSize, font_weight: f32 => FontWeight,
    line_height: f32 => LineHeight, letter_spacing: Length => LetterSpacing,
}

impl StyleExtension for WebStyle {
  type BoxFields<S: Shape> = WebBoxFields<S>;
  type Property = WebProperty;
  type RunFields<S: Shape> = WebRunFields<S>;
  type Value = f32;

  fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V) {
    web_fields::visit_box(visitor);
  }

  fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V) {
    web_fields::visit_run(visitor);
  }

  #[inline(always)]
  fn run_value_from_number(number: f32) -> Option<RunValue<Self>> {
    Some(RunValue::Extension(number))
  }

  #[inline(always)]
  fn interpolate_value(from: &f32, to: &f32, amount: f32) -> Option<f32> {
    Some(from + (to - from) * amount)
  }
}

#[cfg(test)]
mod tests {
  use super::{WebProperty, WebStyle};
  use crate::{
    BoxStyle,
    BoxValue,
    BoxVisitor,
    Condition,
    Declared,
    IntoBoxValue,
    Length,
    Plain,
    PropertyKey,
    RunStyle,
    RunValue,
    Shape,
    State,
    StaticLeaf,
    When,
  };

  /// The extension's fields follow the shared ones in a visit, under their own names.
  #[test]
  fn visit_reaches_the_extension_fields() {
    struct Names(Vec<&'static str>);

    impl<S: Shape> BoxVisitor<f32, S, WebStyle> for Names {
      fn value<T: IntoBoxValue<f32, WebStyle>>(
        &mut self,
        key: PropertyKey<WebProperty>,
        _: &S::Of<T>,
      ) {
        self.0.push(key.name());
      }
    }

    let mut names = Names(Vec::new());

    BoxStyle::<f32, Plain, WebStyle>::default().visit(&mut names);

    assert_eq!(names.0.len(), 35);
    assert_eq!(names.0[34], "corner_radius");
  }

  /// A derived extension leaf splits into a slot naming the extension property, and its
  /// value erases into the variant of its type.
  #[test]
  fn extension_leaves_split_into_slots() {
    let mut style = BoxStyle::<f32, Declared<Condition<f32>>, WebStyle>::default();

    style.ext.corner_radius = Some(Length(4.).derive_when(State::Hovered.into(), || 8.));

    let (_, slots) = style.split();

    assert_eq!(slots.len(), 1);
    assert_eq!(
      slots[0].property,
      PropertyKey::Extension(WebProperty::CornerRadius)
    );
    assert_eq!(
      (slots[0].compute)(),
      StaticLeaf::Fixed(BoxValue::Length(Length(8.)))
    );

    let mut run = RunStyle::<Declared<Condition<f32>>, WebStyle>::default();

    run.ext.font_size = Some(Length(16.).derive_when(State::Hovered.into(), || 18.));

    let (_, slots) = run.split();

    assert_eq!(
      slots[0].property,
      PropertyKey::Extension(WebProperty::FontSize)
    );
    assert_eq!(
      (slots[0].compute)(),
      StaticLeaf::Fixed(RunValue::Length(Length(18.)))
    );
  }
}
