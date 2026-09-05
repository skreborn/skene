//! The properties only pixel media have: rounded corners and type metrics.

use std::fmt::Debug;

use super::{
  BoxField,
  BoxFieldVisitor,
  BoxStyle,
  IntoRunValue,
  PropertyKey,
  PropertyName,
  RunField,
  RunFieldVisitor,
  RunStyle,
  RunValue,
  Scalar,
  Shape,
  StyleExtension,
};

/// The style extension of a pixel medium: a box rounds its corners, and a run carries
/// its type metrics.
///
/// Lengths are logical pixels; `font_weight` and `line_height` are plain numbers, the
/// latter a multiple of the font size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct PixelExt;

/// A pixel box's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct PixelBoxFields<S: Shape> {
  /// The corner radius, in logical pixels.
  pub corner_radius: S::Of<f32>,
}

/// A pixel run's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct PixelRunFields<S: Shape> {
  /// The font size, in logical pixels.
  pub font_size: S::Of<f32>,
  /// The font weight, `100` to `900`.
  pub font_weight: S::Of<f32>,
  /// The line height, as a multiple of the font size.
  pub line_height: S::Of<f32>,
  /// The extra space between letters, in logical pixels.
  pub letter_spacing: S::Of<f32>,
}

/// The names of a pixel medium's own properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub enum PixelProperty {
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

impl PixelProperty {
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

impl PropertyName for PixelProperty {
  #[inline(always)]
  fn name(&self) -> &'static str {
    Self::name(*self)
  }
}

impl IntoRunValue<PixelExt> for f32 {
  #[inline(always)]
  fn into_run_value(self) -> RunValue<PixelExt> {
    RunValue::Extension(self)
  }

  #[inline(always)]
  fn from_run_value(value: RunValue<PixelExt>) -> Option<Self> {
    match value {
      RunValue::Extension(number) => Some(number),
      RunValue::Color(_) => None,
    }
  }
}

/// The pixel medium's properties as types, and the enumerations walking them.
macro_rules! pixel_fields {
  (box: $($box_field:ident => $box_property:ident),+ $(,)?; run: $($run_field:ident => $run_property:ident),+ $(,)?) => {
    /// The pixel medium's own properties, addressed by type.
    pub mod pixel_fields {
      use super::{
        BoxField,
        BoxStyle,
        PixelExt,
        PixelProperty,
        PropertyKey,
        RunField,
        RunStyle,
        Scalar,
        Shape,
      };

      $(
        #[doc = concat!("The `", stringify!($box_field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $box_property;

        impl<N: Scalar> BoxField<N, PixelExt> for $box_property {
          type Value = f32;

          const KEY: PropertyKey<PixelProperty> =
            PropertyKey::Extension(PixelProperty::$box_property);

          #[inline(always)]
          fn get<S: Shape>(style: &BoxStyle<N, S, PixelExt>) -> &S::Of<f32> {
            &style.ext.$box_field
          }

          #[inline(always)]
          fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, PixelExt>) -> &mut S::Of<f32> {
            &mut style.ext.$box_field
          }
        }
      )+

      $(
        #[doc = concat!("The `", stringify!($run_field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $run_property;

        impl RunField<PixelExt> for $run_property {
          type Value = f32;

          const KEY: PropertyKey<PixelProperty> =
            PropertyKey::Extension(PixelProperty::$run_property);

          #[inline(always)]
          fn get<S: Shape>(style: &RunStyle<S, PixelExt>) -> &S::Of<f32> {
            &style.ext.$run_field
          }

          #[inline(always)]
          fn get_mut<S: Shape>(style: &mut RunStyle<S, PixelExt>) -> &mut S::Of<f32> {
            &mut style.ext.$run_field
          }
        }
      )+
    }

    impl StyleExtension for PixelExt {
      type BoxFields<S: Shape> = PixelBoxFields<S>;
      type Property = PixelProperty;
      type RunFields<S: Shape> = PixelRunFields<S>;
      type Value = f32;

      #[inline(always)]
      fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V) {
        $(visitor.field::<pixel_fields::$box_property>();)+
      }

      #[inline(always)]
      fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V) {
        $(visitor.field::<pixel_fields::$run_property>();)+
      }
    }
  };
}

pixel_fields!(
  box: corner_radius => CornerRadius;
  run: font_size => FontSize, font_weight => FontWeight, line_height => LineHeight,
    letter_spacing => LetterSpacing,
);

#[cfg(test)]
mod tests {
  use super::{PixelExt, PixelProperty};
  use crate::next::{
    BoxStyle,
    BoxValue,
    BoxVisitor,
    Condition,
    Declared,
    IntoBoxValue,
    Plain,
    PropertyKey,
    RunStyle,
    RunValue,
    Shape,
    State,
    When,
  };

  /// The extension's fields follow the shared ones in a visit, under their own names.
  #[test]
  fn visit_reaches_the_extension_fields() {
    struct Names(Vec<&'static str>);

    impl<S: Shape> BoxVisitor<f32, S, PixelExt> for Names {
      fn value<T: IntoBoxValue<f32, PixelExt>>(
        &mut self,
        key: PropertyKey<PixelProperty>,
        _: &S::Of<T>,
      ) {
        self.0.push(key.name());
      }
    }

    let mut names = Names(Vec::new());

    BoxStyle::<f32, Plain, PixelExt>::default().visit(&mut names);

    assert_eq!(names.0.len(), 32);
    assert_eq!(names.0[31], "corner_radius");
  }

  /// A derived extension leaf splits into a slot naming the extension property, and its
  /// value erases into the extension variant of the erased value.
  #[test]
  fn extension_leaves_split_into_slots() {
    let mut style = BoxStyle::<f32, Declared<Condition<f32>>, PixelExt>::default();

    style.ext.corner_radius = Some(4f32.derive_when(State::Hovered.into(), || 8.));

    let (_, slots) = style.split();

    assert_eq!(slots.len(), 1);
    assert_eq!(
      slots[0].property,
      PropertyKey::Extension(PixelProperty::CornerRadius)
    );
    assert_eq!((slots[0].compute)(), BoxValue::Number(8.));

    let mut run = RunStyle::<Declared<Condition<f32>>, PixelExt>::default();

    run.ext.font_size = Some(16f32.derive_when(State::Hovered.into(), || 18.));

    let (_, slots) = run.split();

    assert_eq!(
      slots[0].property,
      PropertyKey::Extension(PixelProperty::FontSize)
    );
    assert_eq!((slots[0].compute)(), RunValue::Extension(18.));
  }
}
