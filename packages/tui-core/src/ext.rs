//! The terminal medium's own style properties: the glyphs and radius of a border, a
//! box's shadow, and a run's weight; and the extension every terminal driver shares.

use std::hash::{Hash, Hasher};

use skene_core::{
  BoxFieldVisitor,
  BoxValue,
  Color,
  Fingerprint,
  IntoBoxValue,
  IntoRunValue,
  PropertyName,
  RunFieldVisitor,
  RunValue,
  Scalar,
  Shape,
  StyleExtension,
};
use skene_views::{CornerRadius, TypeMetrics};

/// The characters a one-cell border draws with.
///
/// The named sets cover the box-drawing families—[`LIGHT`](Self::LIGHT) (the default),
/// [`ROUNDED`](Self::ROUNDED), [`DOUBLE`](Self::DOUBLE), [`HEAVY`](Self::HEAVY), and
/// [`ASCII`](Self::ASCII)—and a custom set is the struct built directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct BorderGlyphs {
  /// The top and bottom edges.
  pub horizontal: char,
  /// The left and right edges.
  pub vertical: char,
  /// The top-left corner.
  pub top_left: char,
  /// The top-right corner.
  pub top_right: char,
  /// The bottom-left corner.
  pub bottom_left: char,
  /// The bottom-right corner.
  pub bottom_right: char,
}

impl BorderGlyphs {
  /// Light box-drawing lines with square corners: `─│┌┐└┘`.
  pub const LIGHT: Self = Self::new('─', '│', '┌', '┐', '└', '┘');
  /// Light lines with arc corners—the terminal's corner radius: `─│╭╮╰╯`.
  pub const ROUNDED: Self = Self::new('─', '│', '╭', '╮', '╰', '╯');
  /// Double lines: `═║╔╗╚╝`.
  pub const DOUBLE: Self = Self::new('═', '║', '╔', '╗', '╚', '╝');
  /// Heavy lines: `━┃┏┓┗┛`.
  pub const HEAVY: Self = Self::new('━', '┃', '┏', '┓', '┗', '┛');
  /// Plain ASCII, for terminals without box-drawing characters: `-|++++`.
  pub const ASCII: Self = Self::new('-', '|', '+', '+', '+', '+');

  /// A set from the edge and corner characters.
  #[inline(always)]
  pub const fn new(
    horizontal: char,
    vertical: char,
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
  ) -> Self {
    Self {
      horizontal,
      vertical,
      top_left,
      top_right,
      bottom_left,
      bottom_right,
    }
  }
}

impl Default for BorderGlyphs {
  #[inline(always)]
  fn default() -> Self {
    Self::LIGHT
  }
}

impl Fingerprint for BorderGlyphs {
  #[inline(always)]
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.hash(state);
  }
}

/// The style extension of the terminal: a box draws its border in a glyph set, rounds
/// it through the corner radius, and casts a shadow; a run is bold, by its own flag or
/// by a weight of `600` and above.
///
/// Lengths are cells; the radius is a plain number whose non-zero values all select
/// [`BorderGlyphs::ROUNDED`] where no glyph set is spelled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct TerminalExt;

/// A terminal box's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct TerminalBoxFields<S: Shape> {
  /// The characters the border draws with; [`BorderGlyphs::LIGHT`] when unset.
  pub border_glyphs: S::Of<BorderGlyphs>,
  /// The corner radius; any non-zero value rounds the border where no glyph set is
  /// spelled.
  pub corner_radius: S::Of<f32>,
  /// The shadow color, cast one cell down and right and mixed into the colors beneath.
  pub shadow: S::Of<Color>,
}

/// A terminal run's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct TerminalRunFields<S: Shape> {
  /// Whether the run is bold.
  pub bold: S::Of<bool>,
  /// The font weight, `100` to `900`; `600` and above reads as bold where `bold` is
  /// unset.
  pub weight: S::Of<f32>,
}

/// The names of the terminal's own properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub enum TerminalProperty {
  /// The border glyphs.
  BorderGlyphs,
  /// The corner radius.
  CornerRadius,
  /// The shadow.
  Shadow,
  /// The bold flag.
  Bold,
  /// The font weight.
  Weight,
}

impl TerminalProperty {
  /// The property's name in the framework's vocabulary: the field it is set through.
  #[inline(always)]
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::BorderGlyphs => "border_glyphs",
      Self::CornerRadius => "corner_radius",
      Self::Shadow => "shadow",
      Self::Bold => "bold",
      Self::Weight => "font_weight",
    }
  }
}

impl PropertyName for TerminalProperty {
  #[inline(always)]
  fn name(&self) -> &'static str {
    Self::name(*self)
  }
}

/// An erased value of one of the terminal's own properties; a shadow's color erases
/// through the shared color variant.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum TerminalValue {
  /// A border glyph set.
  BorderGlyphs(BorderGlyphs),
  /// A bold flag.
  Bold(bool),
}

impl<N: Scalar> IntoBoxValue<N, TerminalExt> for BorderGlyphs {
  #[inline(always)]
  fn into_box_value(self) -> BoxValue<N, TerminalExt> {
    BoxValue::Extension(TerminalValue::BorderGlyphs(self))
  }

  #[inline(always)]
  fn from_box_value(value: BoxValue<N, TerminalExt>) -> Option<Self> {
    match value {
      BoxValue::Extension(TerminalValue::BorderGlyphs(glyphs)) => Some(glyphs),
      _ => None,
    }
  }
}

impl IntoRunValue<TerminalExt> for bool {
  #[inline(always)]
  fn into_run_value(self) -> RunValue<TerminalExt> {
    RunValue::Extension(TerminalValue::Bold(self))
  }

  #[inline(always)]
  fn from_run_value(value: RunValue<TerminalExt>) -> Option<Self> {
    match value {
      RunValue::Extension(TerminalValue::Bold(bold)) => Some(bold),
      RunValue::Extension(TerminalValue::BorderGlyphs(_)) | RunValue::Color(_) => None,
    }
  }
}

impl IntoRunValue<TerminalExt> for f32 {
  #[inline(always)]
  fn into_run_value(self) -> RunValue<TerminalExt> {
    RunValue::Extension(TerminalValue::Bold(self >= BOLD_WEIGHT))
  }

  #[inline(always)]
  fn from_run_value(value: RunValue<TerminalExt>) -> Option<Self> {
    match value {
      RunValue::Extension(TerminalValue::Bold(bold)) => Some(if bold { BOLD_WEIGHT } else { 400. }),
      RunValue::Extension(TerminalValue::BorderGlyphs(_)) | RunValue::Color(_) => None,
    }
  }
}

/// The lowest font weight that reads as bold.
pub const BOLD_WEIGHT: f32 = 600.;

/// The terminal's own properties, addressed by type.
pub mod terminal_fields {
  use skene_core::{BoxField, BoxStyle, Color, PropertyKey, RunField, RunStyle, Scalar, Shape};

  use super::{BorderGlyphs as Glyphs, TerminalExt, TerminalProperty};

  /// The `border_glyphs` property.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct BorderGlyphs;

  impl<N: Scalar> BoxField<N, TerminalExt> for BorderGlyphs {
    type Value = Glyphs;

    const KEY: PropertyKey<TerminalProperty> =
      PropertyKey::Extension(TerminalProperty::BorderGlyphs);

    #[inline(always)]
    fn get<S: Shape>(style: &BoxStyle<N, S, TerminalExt>) -> &S::Of<Glyphs> {
      &style.ext.border_glyphs
    }

    #[inline(always)]
    fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, TerminalExt>) -> &mut S::Of<Glyphs> {
      &mut style.ext.border_glyphs
    }
  }

  /// The `corner_radius` property.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct CornerRadius;

  impl<N: Scalar> BoxField<N, TerminalExt> for CornerRadius {
    type Value = f32;

    const KEY: PropertyKey<TerminalProperty> =
      PropertyKey::Extension(TerminalProperty::CornerRadius);

    #[inline(always)]
    fn get<S: Shape>(style: &BoxStyle<N, S, TerminalExt>) -> &S::Of<f32> {
      &style.ext.corner_radius
    }

    #[inline(always)]
    fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, TerminalExt>) -> &mut S::Of<f32> {
      &mut style.ext.corner_radius
    }
  }

  /// The `shadow` property.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct Shadow;

  impl<N: Scalar> BoxField<N, TerminalExt> for Shadow {
    type Value = Color;

    const KEY: PropertyKey<TerminalProperty> = PropertyKey::Extension(TerminalProperty::Shadow);

    #[inline(always)]
    fn get<S: Shape>(style: &BoxStyle<N, S, TerminalExt>) -> &S::Of<Color> {
      &style.ext.shadow
    }

    #[inline(always)]
    fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, TerminalExt>) -> &mut S::Of<Color> {
      &mut style.ext.shadow
    }
  }

  /// The `bold` property.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct Bold;

  impl RunField<TerminalExt> for Bold {
    type Value = bool;

    const KEY: PropertyKey<TerminalProperty> = PropertyKey::Extension(TerminalProperty::Bold);

    #[inline(always)]
    fn get<S: Shape>(style: &RunStyle<S, TerminalExt>) -> &S::Of<bool> {
      &style.ext.bold
    }

    #[inline(always)]
    fn get_mut<S: Shape>(style: &mut RunStyle<S, TerminalExt>) -> &mut S::Of<bool> {
      &mut style.ext.bold
    }
  }

  /// The `font_weight` property.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct Weight;

  impl RunField<TerminalExt> for Weight {
    type Value = f32;

    const KEY: PropertyKey<TerminalProperty> = PropertyKey::Extension(TerminalProperty::Weight);

    #[inline(always)]
    fn get<S: Shape>(style: &RunStyle<S, TerminalExt>) -> &S::Of<f32> {
      &style.ext.weight
    }

    #[inline(always)]
    fn get_mut<S: Shape>(style: &mut RunStyle<S, TerminalExt>) -> &mut S::Of<f32> {
      &mut style.ext.weight
    }
  }
}

impl StyleExtension for TerminalExt {
  type BoxFields<S: Shape> = TerminalBoxFields<S>;
  type Property = TerminalProperty;
  type RunFields<S: Shape> = TerminalRunFields<S>;
  type Value = TerminalValue;

  #[inline(always)]
  fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V) {
    visitor.field::<terminal_fields::BorderGlyphs>();
    visitor.field::<terminal_fields::CornerRadius>();
    visitor.field::<terminal_fields::Shadow>();
  }

  #[inline(always)]
  fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V) {
    visitor.field::<terminal_fields::Bold>();
    visitor.field::<terminal_fields::Weight>();
  }

  #[inline(always)]
  fn run_value_from_number(number: f32) -> Option<RunValue<Self>> {
    Some(RunValue::Extension(TerminalValue::Bold(
      number >= BOLD_WEIGHT,
    )))
  }
}

impl CornerRadius for TerminalExt {
  #[inline(always)]
  fn set_corner_radius<S: Shape>(fields: &mut Self::BoxFields<S>, radius: S::Of<f32>) {
    fields.corner_radius = radius;
  }
}

impl TypeMetrics for TerminalExt {
  #[inline(always)]
  fn set_font_weight<S: Shape>(fields: &mut Self::RunFields<S>, weight: S::Of<f32>) {
    fields.weight = weight;
  }
}
