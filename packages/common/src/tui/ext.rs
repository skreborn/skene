//! The terminal medium's own style properties: the glyphs and radius of a border, a
//! box's shadow and scrollbars, and a run's size and weight; and the extension every
//! terminal platform shares.

use std::hash::{Hash, Hasher};

use skene_core::{
  BoxFieldVisitor,
  BoxValue,
  Color,
  Fingerprint,
  IntoBoxValue,
  IntoRunValue,
  Length,
  PropertyName,
  RunFieldVisitor,
  RunValue,
  Scalar,
  Shape,
  StyleExtension,
};

use crate::{CornerRadius, TypeMetrics};

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

/// The characters a scrollbar draws with: its thumb over its track, along each axis.
///
/// A thumb of the full block (`█`) moves by eighths of a cell, the block elements
/// drawing its partial cells; any other thumb moves by whole cells. The named sets are
/// [`BLOCK`](Self::BLOCK) (the default) and [`LINE`](Self::LINE), and a custom set is
/// the struct built directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct ScrollbarGlyphs {
  /// A vertical scrollbar's thumb.
  pub vertical_thumb: char,
  /// A vertical scrollbar's track.
  pub vertical_track: char,
  /// A horizontal scrollbar's thumb.
  pub horizontal_thumb: char,
  /// A horizontal scrollbar's track.
  pub horizontal_track: char,
}

impl ScrollbarGlyphs {
  /// A full-block thumb over a blank track, moving by eighths: `█` on ` `.
  pub const BLOCK: Self = Self::new('█', ' ', '█', ' ');
  /// A heavy line over a light one, a slimmer bar moving by cells: `┃` on `│`, `━` on
  /// `─`.
  pub const LINE: Self = Self::new('┃', '│', '━', '─');

  /// A set from each axis's thumb and track characters.
  #[inline(always)]
  pub const fn new(
    vertical_thumb: char,
    vertical_track: char,
    horizontal_thumb: char,
    horizontal_track: char,
  ) -> Self {
    Self {
      vertical_thumb,
      vertical_track,
      horizontal_thumb,
      horizontal_track,
    }
  }
}

impl Default for ScrollbarGlyphs {
  #[inline(always)]
  fn default() -> Self {
    Self::BLOCK
  }
}

impl Fingerprint for ScrollbarGlyphs {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.hash(state);
  }
}

impl Default for BorderGlyphs {
  #[inline(always)]
  fn default() -> Self {
    Self::LIGHT
  }
}

impl Fingerprint for BorderGlyphs {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    self.hash(state);
  }
}

/// The style extension of the terminal.
///
/// A box draws its border in a glyph set, rounds it through the corner radius, and
/// casts a shadow; a run is bold, by its own flag or by a weight of `600` and above,
/// and sized in cells.
///
/// Lengths are cells; any radius above zero selects [`BorderGlyphs::ROUNDED`] where no
/// glyph set is spelled. A run's size is a length in cells: a whole number of cells
/// tall and as many times as wide, one to seven, or a fraction of one cell; a terminal
/// without text sizing draws every size at one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[must_use]
pub struct TuiStyle;

/// A terminal box's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct TuiBoxFields<S: Shape> {
  /// The characters the border draws with; [`BorderGlyphs::LIGHT`] when unset.
  pub border_glyphs: S::Of<BorderGlyphs>,
  /// The corner radius, in cells; any radius above zero rounds the border where no
  /// glyph set is spelled.
  pub corner_radius: S::Of<Length>,
  /// The shadow color, mixed into the colors beneath where the shadow falls.
  pub shadow: S::Of<Color>,
  /// How far the shadow falls down and right, in cells; half a cell when unset.
  pub shadow_offset: S::Of<Length>,
  /// How far the shadow's edge softens past its offset, in cells, where the terminal
  /// shows graphics; half a cell when unset.
  pub shadow_softening: S::Of<Length>,
  /// The scrollbar thumb's color; the terminal's foreground when unset.
  pub scrollbar_thumb: S::Of<Color>,
  /// The scrollbar track's color; a quarter of the thumb's over the colors beneath
  /// when unset.
  pub scrollbar_track: S::Of<Color>,
  /// The characters the scrollbars draw with; [`ScrollbarGlyphs::BLOCK`] when unset.
  pub scrollbar_glyphs: S::Of<ScrollbarGlyphs>,
}

/// A terminal run's own properties, in shape `S`.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct TuiRunFields<S: Shape> {
  /// Whether the run is bold.
  pub bold: S::Of<bool>,
  /// The font size, in cells: one to seven, or a fraction of one.
  pub size: S::Of<Length>,
  /// The font weight, `100` to `900`; `600` and above reads as bold where `bold` is
  /// unset.
  pub weight: S::Of<f32>,
}

/// The names of the terminal's own properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[must_use]
pub enum TuiProperty {
  /// The border glyphs.
  BorderGlyphs,
  /// The corner radius.
  CornerRadius,
  /// The shadow.
  Shadow,
  /// The shadow's offset.
  ShadowOffset,
  /// The shadow's softening.
  ShadowSoftening,
  /// The scrollbar thumb's color.
  ScrollbarThumb,
  /// The scrollbar track's color.
  ScrollbarTrack,
  /// The scrollbar glyphs.
  ScrollbarGlyphs,
  /// The bold flag.
  Bold,
  /// The font size.
  Size,
  /// The font weight.
  Weight,
}

impl TuiProperty {
  /// The property's name in the framework's vocabulary: the field it is set through.
  #[inline(always)]
  #[must_use]
  pub const fn name(self) -> &'static str {
    match self {
      Self::BorderGlyphs => "border_glyphs",
      Self::CornerRadius => "corner_radius",
      Self::Shadow => "shadow",
      Self::ShadowOffset => "shadow_offset",
      Self::ShadowSoftening => "shadow_softening",
      Self::ScrollbarThumb => "scrollbar_thumb",
      Self::ScrollbarTrack => "scrollbar_track",
      Self::ScrollbarGlyphs => "scrollbar_glyphs",
      Self::Bold => "bold",
      Self::Size => "font_size",
      Self::Weight => "font_weight",
    }
  }
}

impl PropertyName for TuiProperty {
  #[inline(always)]
  fn name(&self) -> &'static str {
    Self::name(*self)
  }
}

/// An erased value of one of the terminal's own properties; a color or a length erases
/// through the shared variants.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub enum TuiValue {
  /// A border glyph set.
  BorderGlyphs(BorderGlyphs),
  /// A scrollbar glyph set.
  ScrollbarGlyphs(ScrollbarGlyphs),
  /// A bold flag.
  Bold(bool),
  /// A weight.
  Number(f32),
}

impl<N: Scalar> IntoBoxValue<N, TuiStyle> for BorderGlyphs {
  #[inline(always)]
  fn into_box_value(self) -> BoxValue<N, TuiStyle> {
    BoxValue::Extension(TuiValue::BorderGlyphs(self))
  }

  #[inline(always)]
  fn from_box_value(value: BoxValue<N, TuiStyle>) -> Option<Self> {
    match value {
      BoxValue::Extension(TuiValue::BorderGlyphs(glyphs)) => Some(glyphs),
      _ => None,
    }
  }
}

impl<N: Scalar> IntoBoxValue<N, TuiStyle> for ScrollbarGlyphs {
  #[inline(always)]
  fn into_box_value(self) -> BoxValue<N, TuiStyle> {
    BoxValue::Extension(TuiValue::ScrollbarGlyphs(self))
  }

  #[inline(always)]
  fn from_box_value(value: BoxValue<N, TuiStyle>) -> Option<Self> {
    match value {
      BoxValue::Extension(TuiValue::ScrollbarGlyphs(glyphs)) => Some(glyphs),
      _ => None,
    }
  }
}

impl IntoRunValue<TuiStyle> for bool {
  #[inline(always)]
  fn into_run_value(self) -> RunValue<TuiStyle> {
    RunValue::Extension(TuiValue::Bold(self))
  }

  #[inline(always)]
  fn from_run_value(value: RunValue<TuiStyle>) -> Option<Self> {
    match value {
      RunValue::Extension(TuiValue::Bold(bold)) => Some(bold),
      RunValue::Extension(
        TuiValue::BorderGlyphs(_) | TuiValue::ScrollbarGlyphs(_) | TuiValue::Number(_),
      )
      | RunValue::Color(_)
      | RunValue::Length(_)
      | RunValue::Decoration(_)
      | RunValue::DecorationStyle(_) => None,
    }
  }
}

impl IntoRunValue<TuiStyle> for f32 {
  #[inline(always)]
  fn into_run_value(self) -> RunValue<TuiStyle> {
    RunValue::Extension(TuiValue::Number(self))
  }

  #[inline(always)]
  fn from_run_value(value: RunValue<TuiStyle>) -> Option<Self> {
    match value {
      RunValue::Extension(TuiValue::Number(number)) => Some(number),
      RunValue::Extension(
        TuiValue::BorderGlyphs(_) | TuiValue::ScrollbarGlyphs(_) | TuiValue::Bold(_),
      )
      | RunValue::Color(_)
      | RunValue::Length(_)
      | RunValue::Decoration(_)
      | RunValue::DecorationStyle(_) => None,
    }
  }
}

/// The lowest font weight that reads as bold.
pub const BOLD_WEIGHT: f32 = 600.;

skene_core::extension_fields! {
  /// The terminal's own properties, addressed by type.
  pub mod tui_fields for TuiStyle, TuiProperty;
  box: border_glyphs: super::BorderGlyphs => BorderGlyphs, corner_radius: Length => CornerRadius,
    shadow: Color => Shadow, shadow_offset: Length => ShadowOffset,
    shadow_softening: Length => ShadowSoftening, scrollbar_thumb: Color => ScrollbarThumb,
    scrollbar_track: Color => ScrollbarTrack,
    scrollbar_glyphs: super::ScrollbarGlyphs => ScrollbarGlyphs;
  run: bold: bool => Bold, size: Length => Size, weight: f32 => Weight,
}

impl StyleExtension for TuiStyle {
  type BoxFields<S: Shape> = TuiBoxFields<S>;
  type Property = TuiProperty;
  type RunFields<S: Shape> = TuiRunFields<S>;
  type Value = TuiValue;

  fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V) {
    tui_fields::visit_box(visitor);
  }

  fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V) {
    tui_fields::visit_run(visitor);
  }

  #[inline(always)]
  fn run_value_from_number(number: f32) -> Option<RunValue<Self>> {
    Some(RunValue::Extension(TuiValue::Number(number)))
  }

  #[inline(always)]
  fn interpolate_value(from: &TuiValue, to: &TuiValue, amount: f32) -> Option<TuiValue> {
    match (from, to) {
      (TuiValue::Number(from), TuiValue::Number(to)) => {
        Some(TuiValue::Number((to - from).mul_add(amount, *from)))
      }
      _ => None,
    }
  }
}

impl CornerRadius for TuiStyle {
  fn set_corner_radius<S: Shape>(fields: &mut Self::BoxFields<S>, radius: S::Of<Length>) {
    fields.corner_radius = radius;
  }
}

impl TypeMetrics for TuiStyle {
  fn set_font_size<S: Shape>(fields: &mut Self::RunFields<S>, size: S::Of<Length>) {
    fields.size = size;
  }

  fn set_font_weight<S: Shape>(fields: &mut Self::RunFields<S>, weight: S::Of<f32>) {
    fields.weight = weight;
  }

  fn font_size<S: Shape>(fields: &Self::RunFields<S>) -> S::Of<Length> {
    fields.size.clone()
  }

  fn font_weight<S: Shape>(fields: &Self::RunFields<S>) -> S::Of<f32> {
    fields.weight.clone()
  }
}
