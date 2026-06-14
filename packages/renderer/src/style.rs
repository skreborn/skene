//! The abstract style model.
//!
//! This is deliberately **semantic and backend-agnostic**: it describes *what*
//! a node should look like, not *how* any particular backend expresses that. In
//! particular it contains no CSS — the DOM backend is solely responsible for
//! translating a [`Style`] into CSS. A native backend would translate the same
//! [`Style`] into paint/layout commands, a TUI backend into cell attributes.
//!
//! [`Style`] is a flat bag of optional properties so that styles can be cheaply
//! merged (a later style overrides earlier set properties) and compared.

/// An RGBA color, 8 bits per channel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Color {
  pub r: u8,
  pub g: u8,
  pub b: u8,
  pub a: u8,
}

impl Color {
  pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);
  pub const BLACK: Color = Color::rgb(0, 0, 0);
  pub const WHITE: Color = Color::rgb(255, 255, 255);

  pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
  }

  pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { r, g, b, a }
  }
}

/// A length value.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Length {
  /// A length in (CSS/logical) pixels.
  Px(f64),
  /// A percentage of the relevant containing measure.
  Pct(f64),
  /// Automatic / content-driven sizing.
  Auto,
}

impl Length {
  pub const ZERO: Length = Length::Px(0.0);
}

impl From<f64> for Length {
  fn from(v: f64) -> Self {
    Length::Px(v)
  }
}

impl From<i32> for Length {
  fn from(v: i32) -> Self {
    Length::Px(v as f64)
  }
}

/// How children are laid out.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Display {
  Flex,
  Block,
  None,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FlexDirection {
  Row,
  Column,
  RowReverse,
  ColumnReverse,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum JustifyContent {
  Start,
  Center,
  End,
  SpaceBetween,
  SpaceAround,
  SpaceEvenly,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AlignItems {
  Start,
  Center,
  End,
  Stretch,
  Baseline,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Cursor {
  Default,
  Pointer,
  Text,
}

/// A flat, mergeable bag of style properties. Every property is optional; an
/// unset property means "inherit / use the backend default".
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Style {
  pub display: Option<Display>,
  pub flex_direction: Option<FlexDirection>,
  pub justify_content: Option<JustifyContent>,
  pub align_items: Option<AlignItems>,
  pub gap: Option<Length>,
  pub flex_grow: Option<f32>,
  pub flex_shrink: Option<f32>,
  pub flex_basis: Option<Length>,

  pub width: Option<Length>,
  pub height: Option<Length>,
  pub min_width: Option<Length>,
  pub min_height: Option<Length>,
  pub max_width: Option<Length>,
  pub max_height: Option<Length>,

  pub padding_top: Option<Length>,
  pub padding_right: Option<Length>,
  pub padding_bottom: Option<Length>,
  pub padding_left: Option<Length>,

  pub margin_top: Option<Length>,
  pub margin_right: Option<Length>,
  pub margin_bottom: Option<Length>,
  pub margin_left: Option<Length>,

  pub background: Option<Color>,
  pub color: Option<Color>,
  pub font_size: Option<Length>,
  pub font_weight: Option<u16>,

  pub border_width: Option<Length>,
  pub border_color: Option<Color>,
  pub border_radius: Option<Length>,

  pub cursor: Option<Cursor>,
}

impl Style {
  pub fn new() -> Self {
    Style::default()
  }

  /// Merge `other` on top of `self`: every property set in `other` overrides
  /// the corresponding property in `self`.
  pub fn merge(mut self, other: &Style) -> Self {
    macro_rules! over {
            ($($f:ident),* $(,)?) => {
                $( if other.$f.is_some() { self.$f = other.$f; } )*
            };
        }
    over!(
      display,
      flex_direction,
      justify_content,
      align_items,
      gap,
      flex_grow,
      flex_shrink,
      flex_basis,
      width,
      height,
      min_width,
      min_height,
      max_width,
      max_height,
      padding_top,
      padding_right,
      padding_bottom,
      padding_left,
      margin_top,
      margin_right,
      margin_bottom,
      margin_left,
      background,
      color,
      font_size,
      font_weight,
      border_width,
      border_color,
      border_radius,
      cursor,
    );
    self
  }
}

/// Fluent builder methods. These only set fields — they have no knowledge of any
/// backend or of CSS.
impl Style {
  pub fn display(mut self, v: Display) -> Self {
    self.display = Some(v);
    self
  }
  pub fn flex_col(mut self) -> Self {
    self.display = Some(Display::Flex);
    self.flex_direction = Some(FlexDirection::Column);
    self
  }
  pub fn flex_row(mut self) -> Self {
    self.display = Some(Display::Flex);
    self.flex_direction = Some(FlexDirection::Row);
    self
  }
  pub fn flex_direction(mut self, v: FlexDirection) -> Self {
    self.flex_direction = Some(v);
    self
  }
  pub fn justify_content(mut self, v: JustifyContent) -> Self {
    self.justify_content = Some(v);
    self
  }
  pub fn justify_center(mut self) -> Self {
    self.justify_content = Some(JustifyContent::Center);
    self
  }
  pub fn align_items(mut self, v: AlignItems) -> Self {
    self.align_items = Some(v);
    self
  }
  pub fn items_center(mut self) -> Self {
    self.align_items = Some(AlignItems::Center);
    self
  }
  pub fn gap(mut self, v: impl Into<Length>) -> Self {
    self.gap = Some(v.into());
    self
  }
  pub fn flex_grow(mut self, v: f32) -> Self {
    self.flex_grow = Some(v);
    self
  }
  pub fn flex_shrink(mut self, v: f32) -> Self {
    self.flex_shrink = Some(v);
    self
  }
  pub fn flex_basis(mut self, v: impl Into<Length>) -> Self {
    self.flex_basis = Some(v.into());
    self
  }

  pub fn width(mut self, v: impl Into<Length>) -> Self {
    self.width = Some(v.into());
    self
  }
  pub fn height(mut self, v: impl Into<Length>) -> Self {
    self.height = Some(v.into());
    self
  }
  pub fn size(mut self, w: impl Into<Length>, h: impl Into<Length>) -> Self {
    self.width = Some(w.into());
    self.height = Some(h.into());
    self
  }
  pub fn min_width(mut self, v: impl Into<Length>) -> Self {
    self.min_width = Some(v.into());
    self
  }
  pub fn min_height(mut self, v: impl Into<Length>) -> Self {
    self.min_height = Some(v.into());
    self
  }
  pub fn max_width(mut self, v: impl Into<Length>) -> Self {
    self.max_width = Some(v.into());
    self
  }
  pub fn max_height(mut self, v: impl Into<Length>) -> Self {
    self.max_height = Some(v.into());
    self
  }

  pub fn padding(mut self, v: impl Into<Length>) -> Self {
    let v = v.into();
    self.padding_top = Some(v);
    self.padding_right = Some(v);
    self.padding_bottom = Some(v);
    self.padding_left = Some(v);
    self
  }
  pub fn padding_horiz(mut self, v: impl Into<Length>) -> Self {
    let v = v.into();
    self.padding_left = Some(v);
    self.padding_right = Some(v);
    self
  }
  pub fn padding_vert(mut self, v: impl Into<Length>) -> Self {
    let v = v.into();
    self.padding_top = Some(v);
    self.padding_bottom = Some(v);
    self
  }

  pub fn margin(mut self, v: impl Into<Length>) -> Self {
    let v = v.into();
    self.margin_top = Some(v);
    self.margin_right = Some(v);
    self.margin_bottom = Some(v);
    self.margin_left = Some(v);
    self
  }

  pub fn background(mut self, v: Color) -> Self {
    self.background = Some(v);
    self
  }
  pub fn color(mut self, v: Color) -> Self {
    self.color = Some(v);
    self
  }
  pub fn font_size(mut self, v: impl Into<Length>) -> Self {
    self.font_size = Some(v.into());
    self
  }
  pub fn font_weight(mut self, v: u16) -> Self {
    self.font_weight = Some(v);
    self
  }

  pub fn border(mut self, width: impl Into<Length>, color: Color) -> Self {
    self.border_width = Some(width.into());
    self.border_color = Some(color);
    self
  }
  pub fn border_radius(mut self, v: impl Into<Length>) -> Self {
    self.border_radius = Some(v.into());
    self
  }

  pub fn cursor(mut self, v: Cursor) -> Self {
    self.cursor = Some(v);
    self
  }
}
