//! The properties only pixel media have: rounded corners and type metrics.

use std::fmt::Debug;

use skene_core::{
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

/// The style extension of a pixel medium: a box rounds its corners, and a run carries
/// its type metrics.
///
/// Lengths are logical pixels; `font_weight` and `line_height` are plain numbers, the
/// latter a multiple of the font size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct GuiExt;

/// A pixel box's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct GuiBoxFields<S: Shape> {
  /// The corner radius, in logical pixels.
  pub corner_radius: S::Of<Length>,
}

/// A pixel run's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct GuiRunFields<S: Shape> {
  /// The font size, in logical pixels.
  pub font_size: S::Of<Length>,
  /// The font weight, `100` to `900`.
  pub font_weight: S::Of<f32>,
  /// The line height, as a multiple of the font size.
  pub line_height: S::Of<f32>,
  /// The extra space between letters, in logical pixels.
  pub letter_spacing: S::Of<Length>,
}

/// The names of a pixel medium's own properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub enum GuiProperty {
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

impl GuiProperty {
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

impl PropertyName for GuiProperty {
  #[inline(always)]
  fn name(&self) -> &'static str {
    Self::name(*self)
  }
}

impl From<GuiProperty> for StyleProperty<GuiProperty> {
  #[inline(always)]
  fn from(property: GuiProperty) -> Self {
    Self::Extension(property)
  }
}

impl IntoRunValue<GuiExt> for f32 {
  #[inline(always)]
  fn into_run_value(self) -> RunValue<GuiExt> {
    RunValue::Extension(self)
  }

  #[inline(always)]
  fn from_run_value(value: RunValue<GuiExt>) -> Option<Self> {
    match value {
      RunValue::Extension(number) => Some(number),
      RunValue::Color(_)
      | RunValue::Length(_)
      | RunValue::Decoration(_)
      | RunValue::DecorationStyle(_) => None,
    }
  }
}

/// The pixel medium's properties as types, and the enumerations walking them.
macro_rules! gui_fields {
  (
    box: $($box_field:ident: $box_value:ty => $box_property:ident),+ $(,)?;
    run: $($run_field:ident: $run_value:ty => $run_property:ident),+ $(,)?
  ) => {
    /// The pixel medium's own properties, addressed by type.
    pub mod gui_fields {
      use skene_core::{BoxField, BoxStyle, Length, PropertyKey, RunField, RunStyle, Scalar, Shape};

      use super::{GuiExt, GuiProperty};

      $(
        #[doc = concat!("The `", stringify!($box_field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $box_property;

        impl<N: Scalar> BoxField<N, GuiExt> for $box_property {
          type Value = $box_value;

          const KEY: PropertyKey<GuiProperty> =
            PropertyKey::Extension(GuiProperty::$box_property);

          #[inline(always)]
          fn get<S: Shape>(style: &BoxStyle<N, S, GuiExt>) -> &S::Of<$box_value> {
            &style.ext.$box_field
          }

          #[inline(always)]
          fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, GuiExt>) -> &mut S::Of<$box_value> {
            &mut style.ext.$box_field
          }
        }
      )+

      $(
        #[doc = concat!("The `", stringify!($run_field), "` property.")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $run_property;

        impl RunField<GuiExt> for $run_property {
          type Value = $run_value;

          const KEY: PropertyKey<GuiProperty> =
            PropertyKey::Extension(GuiProperty::$run_property);

          #[inline(always)]
          fn get<S: Shape>(style: &RunStyle<S, GuiExt>) -> &S::Of<$run_value> {
            &style.ext.$run_field
          }

          #[inline(always)]
          fn get_mut<S: Shape>(style: &mut RunStyle<S, GuiExt>) -> &mut S::Of<$run_value> {
            &mut style.ext.$run_field
          }
        }
      )+
    }

    impl StyleExtension for GuiExt {
      type BoxFields<S: Shape> = GuiBoxFields<S>;
      type Property = GuiProperty;
      type RunFields<S: Shape> = GuiRunFields<S>;
      type Value = f32;

      #[inline(always)]
      fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V) {
        $(visitor.field::<gui_fields::$box_property>();)+
      }

      #[inline(always)]
      fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V) {
        $(visitor.field::<gui_fields::$run_property>();)+
      }

      #[inline(always)]
      fn interpolate_value(from: &f32, to: &f32, amount: f32) -> Option<f32> {
        Some(from + (to - from) * amount)
      }
    }
  };
}

gui_fields!(
  box: corner_radius: Length => CornerRadius;
  run: font_size: Length => FontSize, font_weight: f32 => FontWeight,
    line_height: f32 => LineHeight, letter_spacing: Length => LetterSpacing,
);

#[cfg(test)]
mod tests {
  use skene_core::{
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

  use super::{GuiExt, GuiProperty};

  /// The extension's fields follow the shared ones in a visit, under their own names.
  #[test]
  fn visit_reaches_the_extension_fields() {
    struct Names(Vec<&'static str>);

    impl<S: Shape> BoxVisitor<f32, S, GuiExt> for Names {
      fn value<T: IntoBoxValue<f32, GuiExt>>(
        &mut self,
        key: PropertyKey<GuiProperty>,
        _: &S::Of<T>,
      ) {
        self.0.push(key.name());
      }
    }

    let mut names = Names(Vec::new());

    BoxStyle::<f32, Plain, GuiExt>::default().visit(&mut names);

    assert_eq!(names.0.len(), 35);
    assert_eq!(names.0[34], "corner_radius");
  }

  /// A derived extension leaf splits into a slot naming the extension property, and its
  /// value erases into the variant of its type.
  #[test]
  fn extension_leaves_split_into_slots() {
    let mut style = BoxStyle::<f32, Declared<Condition<f32>>, GuiExt>::default();

    style.ext.corner_radius = Some(Length(4.).derive_when(State::Hovered.into(), || 8.));

    let (_, slots) = style.split();

    assert_eq!(slots.len(), 1);
    assert_eq!(
      slots[0].property,
      PropertyKey::Extension(GuiProperty::CornerRadius)
    );
    assert_eq!(
      (slots[0].compute)(),
      StaticLeaf::Fixed(BoxValue::Length(Length(8.)))
    );

    let mut run = RunStyle::<Declared<Condition<f32>>, GuiExt>::default();

    run.ext.font_size = Some(Length(16.).derive_when(State::Hovered.into(), || 18.));

    let (_, slots) = run.split();

    assert_eq!(
      slots[0].property,
      PropertyKey::Extension(GuiProperty::FontSize)
    );
    assert_eq!(
      (slots[0].compute)(),
      StaticLeaf::Fixed(RunValue::Length(Length(18.)))
    );
  }
}
