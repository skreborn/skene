//! A node's style as the realization draws it: the record compiled with nothing kept
//! ([`skene_foundation::seam::style::Keep::Nothing`]), every value resolved by the shared half,
//! split into the box's look and the run properties the texts beneath inherit.

use skene_foundation::{
  Color, Corners, FontFamily, FontStyle, FontWeight, TextAlignment, TextDecoration,
  TextDecorationStyle,
  seam::{
    record::{Prop, Resolved, Transition},
    style::{CompiledStyle, KeptLeaf},
  },
  text::{LineHeight, PaintStyle, ParagraphStyle, RunStyle},
  value::{Border, Offset, Ring, Shadows},
};

/// A box's look, resolved.
#[derive(Clone, PartialEq, Debug)]
pub struct Look {
  /// The fill behind its content.
  pub background: Option<Color>,
  /// The line around it.
  pub border: Option<Border>,
  /// The line's color, the text color where unset.
  pub border_color: Option<Color>,
  /// The corners' radii.
  pub corners: Corners,
  /// The shadows it casts.
  pub shadows: Option<Shadows>,
  /// Its opacity.
  pub opacity: f32,
  /// Whether its content is cut off at its edges.
  pub clip: bool,
  /// The line around it that takes no room.
  pub ring: Ring,
  /// The ring's color, the text color where unset.
  pub ring_color: Option<Color>,
  /// How far it and its subtree are drawn from where they were laid out.
  pub offset: Offset,
  /// The transitions its look animates through.
  pub transitions: Vec<Transition>,
}

impl Default for Look {
  fn default() -> Self {
    Self {
      background: None,
      border: None,
      border_color: None,
      corners: Corners::all(0.),
      shadows: None,
      opacity: 1.,
      clip: false,
      ring: Ring::NONE,
      ring_color: None,
      offset: Offset::ZERO,
      transitions: Vec::new(),
    }
  }
}

impl Look {
  /// The width of its border line.
  pub fn border_width(&self) -> f32 {
    self.border.map_or(0., |border| border.width)
  }

  /// How far it is drawn from where it was laid out at `width` by `height`, in pixels.
  pub const fn offset_at(&self, width: f32, height: f32) -> (f32, f32) {
    self.offset.in_pixels(width, height)
  }

  /// Whether it draws nothing of its own.
  pub fn is_plain(&self) -> bool {
    self.background.is_none()
      && self.border_width() <= 0.
      && self
        .shadows
        .as_ref()
        .is_none_or(|shadows| shadows.0.is_empty())
  }
}

/// The run properties a node sets, each unset where it inherits.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct RunProps {
  /// The text's color.
  pub color: Option<Color>,
  /// The typeface.
  pub family: Option<FontFamily>,
  /// The font size, in pixels.
  pub size: Option<f32>,
  /// The weight.
  pub weight: Option<FontWeight>,
  /// Upright or slanted.
  pub slant: Option<FontStyle>,
  /// The line height, a multiple of the size.
  pub line_height: Option<f32>,
  /// Extra space between letters.
  pub letter_spacing: Option<f32>,
  /// The decoration lines.
  pub decoration: Option<TextDecoration>,
  /// How they are drawn.
  pub decoration_style: Option<TextDecorationStyle>,
  /// Their color.
  pub decoration_color: Option<Color>,
  /// Where a paragraph's lines sit.
  pub alignment: Option<TextAlignment>,
  /// The most lines a paragraph shows.
  pub max_lines: Option<u32>,
  /// A text entry's caret.
  pub caret_color: Option<Color>,
  /// The fill behind a text entry's selection.
  pub selection_color: Option<Color>,
  /// An empty text entry's placeholder.
  pub placeholder_color: Option<Color>,
}

impl RunProps {
  /// These properties over `inherited`: each set one replaces the inherited one.
  #[must_use]
  pub fn over(&self, inherited: &Self) -> Self {
    Self {
      color: self.color.or(inherited.color),
      family: self.family.clone().or_else(|| inherited.family.clone()),
      size: self.size.or(inherited.size),
      weight: self.weight.or(inherited.weight),
      slant: self.slant.or(inherited.slant),
      line_height: self.line_height.or(inherited.line_height),
      letter_spacing: self.letter_spacing.or(inherited.letter_spacing),
      decoration: self.decoration.or(inherited.decoration),
      decoration_style: self.decoration_style.or(inherited.decoration_style),
      decoration_color: self.decoration_color.or(inherited.decoration_color),
      alignment: self.alignment.or(inherited.alignment),
      max_lines: self.max_lines.or(inherited.max_lines),
      caret_color: self.caret_color.or(inherited.caret_color),
      selection_color: self.selection_color.or(inherited.selection_color),
      placeholder_color: self.placeholder_color.or(inherited.placeholder_color),
    }
  }

  /// Sets the property `prop` names to `value`, where it is a run property.
  pub fn put(&mut self, prop: Prop, value: &Resolved) {
    use Resolved as R;

    match (prop, value) {
      (Prop::Color, R::Color(v)) => self.color = Some(*v),
      (Prop::FontFamily, R::FontFamily(v)) => self.family = Some(v.clone()),
      (Prop::FontSize, R::Number(v)) => self.size = Some(*v),
      (Prop::FontWeight, R::FontWeight(v)) => self.weight = Some(*v),
      (Prop::FontStyle, R::FontStyle(v)) => self.slant = Some(*v),
      (Prop::LineHeight, R::Number(v)) => self.line_height = Some(*v),
      (Prop::LetterSpacing, R::Number(v)) => self.letter_spacing = Some(*v),
      (Prop::Decoration, R::TextDecoration(v)) => self.decoration = Some(*v),
      (Prop::DecorationStyle, R::TextDecorationStyle(v)) => self.decoration_style = Some(*v),
      (Prop::DecorationColor, R::Color(v)) => self.decoration_color = Some(*v),
      (Prop::TextAlignment, R::TextAlignment(v)) => self.alignment = Some(*v),
      (Prop::MaxLines, R::Count(v)) => self.max_lines = Some(*v),
      (Prop::CaretColor, R::Color(v)) => self.caret_color = Some(*v),
      (Prop::SelectionColor, R::Color(v)) => self.selection_color = Some(*v),
      (Prop::PlaceholderColor, R::Color(v)) => self.placeholder_color = Some(*v),
      _ => {}
    }
  }

  /// The run style these properties give a text, `ink` its color where none is set: the
  /// desktop's interface family where none is named, every size scaled as the desktop
  /// scales text.
  pub fn run_style(&self, ink: Color, default_size: f32) -> RunStyle {
    let family = match &self.family {
      None | Some(FontFamily::System) => crate::environment::system_family(),
      Some(family) => family.clone(),
    };

    RunStyle {
      family,
      size: self.size.unwrap_or(default_size) * crate::environment::text_scale(),
      weight: self.weight.unwrap_or(FontWeight::REGULAR),
      slant: self.slant.unwrap_or(FontStyle::Normal),
      letter_spacing: self.letter_spacing.unwrap_or(0.),
      paint: PaintStyle {
        color: self.color.unwrap_or(ink),
        decoration: self.decoration.unwrap_or(TextDecoration::NONE),
        decoration_style: self.decoration_style.unwrap_or(TextDecorationStyle::Solid),
        decoration_color: self.decoration_color,
      },
    }
  }

  /// The paragraph style these properties give a text.
  pub fn paragraph_style(&self) -> ParagraphStyle {
    ParagraphStyle {
      alignment: self.alignment.unwrap_or_default(),
      max_lines: self.max_lines,
      ellipsis: self.max_lines.is_some(),
      line_height: self
        .line_height
        .map_or(LineHeight::Normal, LineHeight::Factor),
      ..ParagraphStyle::default()
    }
  }
}

/// A compiled style's resolved values, split into a look and run properties.
pub fn resolve(style: &CompiledStyle) -> (Look, RunProps) {
  use Resolved as R;

  let mut look = Look::default();
  let mut run = RunProps::default();

  for (prop, value) in &style.props {
    let KeptLeaf::Value(resolved) = &value.base else {
      continue;
    };

    match (prop, resolved) {
      (Prop::Background, R::Color(v)) => look.background = Some(*v),
      (Prop::Border, R::Border(v)) => look.border = Some(*v),
      (Prop::BorderColor, R::Color(v)) => look.border_color = Some(*v),
      (Prop::CornerRadius, R::Corners(v)) => look.corners = *v,
      (Prop::Shadow, R::Shadows(v)) => look.shadows = Some(v.clone()),
      (Prop::Opacity, R::Number(v)) => look.opacity = v.clamp(0., 1.),
      (Prop::Clip, R::Flag(v)) => look.clip = *v,
      (Prop::Ring, R::Ring(v)) => look.ring = *v,
      (Prop::RingColor, R::Color(v)) => look.ring_color = Some(*v),
      (Prop::Offset, R::Offset(v)) => look.offset = *v,
      _ => run.put(*prop, resolved),
    }
  }

  look.transitions.clone_from(&style.transitions);

  (look, run)
}
