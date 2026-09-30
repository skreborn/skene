//! Raw CSS declarations: what the compiler renders a style's properties into, and the
//! escape hatch for styling a node with arbitrary CSS.
//!
//! A declaration's value is CSS text the caller renders—`"8px"`,
//! `"oklch(0.7 0.1 250 / 1)"`, `"flex"`, `"repeat(3, 1fr)"`—so the full open-ended CSS
//! value space is reachable and nothing is special-cased.
//!
//! Declarations are keyed by property and sorted, so a set is deterministic and
//! content-addressed for class dedup and server-side rendering.

use std::{
  borrow::Cow,
  fmt::{self, Display, Formatter, Write as _},
};

use skene_core::{
  Arrangement,
  Color,
  Dimension,
  GridAreas,
  GridPlacement,
  GridTemplate,
  GridTrack,
  Inset,
  ItemAlignment,
  Line,
  MouseCursor,
  TemplateEntry,
  TrackLimit,
};
use smallvec::SmallVec;

/// A set of CSS declarations (`property: value`), kept sorted by property.
///
/// A value is its final CSS text, borrowed when static (a keyword) or owned when
/// rendered (a length, a color). Backed by an inline [`SmallVec`], so the small sets a
/// view has need no heap allocation, and the sorted order makes the set deterministic
/// and content-addressed for class dedup and server-side rendering.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Declarations(SmallVec<[(&'static str, Cow<'static, str>); 8]>);

impl Declarations {
  /// Creates an empty declaration set.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// Sets one declaration to its CSS text, replacing any previous value for that
  /// property. A `&'static str` (a keyword) is stored without allocating; a rendered
  /// `String` is taken by value.
  pub fn set(mut self, property: &'static str, value: impl Into<Cow<'static, str>>) -> Self {
    let value = value.into();

    match self
      .0
      .binary_search_by_key(&property, |(existing, _)| *existing)
    {
      Ok(index) => self.0[index].1 = value,
      Err(index) => self.0.insert(index, (property, value)),
    }

    self
  }

  /// Returns whether no declarations are set.
  #[inline(always)]
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }

  /// Renders a [`Color`] to its CSS `oklch(…)` text, ready for [`set`](Self::set).
  #[must_use]
  pub fn render_color(color: Color) -> String {
    let [lightness, chroma, hue, alpha] = color.to_oklch();

    format!("oklch({lightness} {chroma} {hue} / {alpha})")
  }

  /// Renders a [`MouseCursor`] to its CSS `cursor` keyword.
  #[must_use]
  pub const fn render_cursor(cursor: MouseCursor) -> &'static str {
    match cursor {
      MouseCursor::Default => "default",
      MouseCursor::Click => "pointer",
      MouseCursor::Text => "text",
      MouseCursor::Grab => "grab",
      MouseCursor::Grabbing => "grabbing",
    }
  }

  /// Renders an [`Arrangement`] to its CSS distribution keyword.
  #[must_use]
  pub const fn render_arrangement(arrangement: Arrangement) -> &'static str {
    match arrangement {
      Arrangement::Start => "flex-start",
      Arrangement::End => "flex-end",
      Arrangement::Center => "center",
      Arrangement::Stretch => "stretch",
      Arrangement::SpaceBetween => "space-between",
      Arrangement::SpaceAround => "space-around",
      Arrangement::SpaceEvenly => "space-evenly",
    }
  }

  /// Renders an [`ItemAlignment`] to its CSS keyword.
  #[must_use]
  pub const fn render_item_alignment(alignment: ItemAlignment) -> &'static str {
    match alignment {
      ItemAlignment::Start => "flex-start",
      ItemAlignment::End => "flex-end",
      ItemAlignment::Center => "center",
      ItemAlignment::Baseline => "baseline",
      ItemAlignment::Stretch => "stretch",
    }
  }

  /// Renders a fraction (`0.` to `1.`) as CSS percentage text.
  ///
  /// Widened for the multiply, then narrowed back so `0.1` renders as `10`, not
  /// `10.000001`.
  #[must_use]
  fn render_fraction(value: f32) -> String {
    #[expect(
      clippy::cast_possible_truncation,
      reason = "a fraction's percentage is precise enough in f32"
    )]
    let percent = (f64::from(value) * 100.) as f32;

    format!("{percent}%")
  }

  /// Renders a [`Dimension`] to its CSS length text.
  #[must_use]
  pub fn render_dimension(dimension: Dimension<f32>) -> String {
    match dimension {
      Dimension::Length(value) => format!("{value}px"),
      Dimension::Fraction(value) => Self::render_fraction(value),
      Dimension::Auto => "auto".to_owned(),
    }
  }

  /// Renders a dimension inset to its four-value CSS shorthand (`t r b l`).
  #[must_use]
  pub fn render_dimension_inset(inset: Inset<Dimension<f32>>) -> String {
    format!(
      "{} {} {} {}",
      Self::render_dimension(inset.top),
      Self::render_dimension(inset.right),
      Self::render_dimension(inset.bottom),
      Self::render_dimension(inset.left),
    )
  }

  /// Renders one [`TrackLimit`] of a `minmax()` track.
  #[must_use]
  fn render_track_limit(limit: TrackLimit<f32>) -> String {
    match limit {
      TrackLimit::Length(value) => format!("{value}px"),
      TrackLimit::Fraction(value) => Self::render_fraction(value),
      TrackLimit::Flex(value) => format!("{value}fr"),
      TrackLimit::Auto => "auto".to_owned(),
      TrackLimit::MinContent => "min-content".to_owned(),
      TrackLimit::MaxContent => "max-content".to_owned(),
    }
  }

  /// Renders one [`TrackLimit`] as the minimum of a `minmax()` track, where a flexible
  /// share sizes as `auto`.
  #[must_use]
  fn render_track_minimum(limit: TrackLimit<f32>) -> String {
    match limit {
      TrackLimit::Flex(_) => "auto".to_owned(),
      limit => Self::render_track_limit(limit),
    }
  }

  /// Renders a [`GridTrack`] to its CSS track text.
  #[must_use]
  pub fn render_track(track: GridTrack<f32>) -> String {
    match track {
      GridTrack::Length(value) => format!("{value}px"),
      GridTrack::Fraction(value) => Self::render_fraction(value),
      GridTrack::Flex(value) => format!("{value}fr"),
      GridTrack::Auto => "auto".to_owned(),
      GridTrack::MinContent => "min-content".to_owned(),
      GridTrack::MaxContent => "max-content".to_owned(),
      GridTrack::FitContent(value) => format!("fit-content({value}px)"),
      GridTrack::Minmax(min, max) => format!(
        "minmax({}, {})",
        Self::render_track_minimum(min),
        Self::render_track_limit(max),
      ),
      GridTrack::Adaptive(min, max) => format!(
        "repeat(auto-fill, minmax({}, {}))",
        Self::render_track_minimum(min),
        Self::render_track_limit(max),
      ),
    }
  }

  /// Renders a track list to its space-separated CSS text.
  #[must_use]
  pub fn render_tracks(tracks: &[GridTrack<f32>]) -> String {
    tracks
      .iter()
      .map(|track| Self::render_track(*track))
      .collect::<Vec<_>>()
      .join(" ")
  }

  /// Renders a grid template to its CSS track text, each line's names in one bracketed
  /// group before the track it starts.
  #[must_use]
  pub fn render_template(template: &GridTemplate<f32>) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();

    for entry in template.entries() {
      match entry {
        TemplateEntry::Line(name) => names.push(Ident(name.as_str()).to_string()),
        TemplateEntry::Track(track) => {
          if !names.is_empty() {
            parts.push(format!("[{}]", names.join(" ")));
            names.clear();
          }

          parts.push(Self::render_track(*track));
        }
      }
    }

    if !names.is_empty() {
      parts.push(format!("[{}]", names.join(" ")));
    }

    parts.join(" ")
  }

  /// Renders named grid areas to their CSS text: a quoted row of cell names per row.
  #[must_use]
  pub fn render_areas(areas: &GridAreas) -> String {
    areas
      .rows()
      .iter()
      .map(|row| {
        let cells = row
          .iter()
          .map(|cell| cell.map_or_else(|| ".".to_owned(), |name| Ident(name.as_str()).to_string()))
          .collect::<Vec<_>>()
          .join(" ");

        format!("\"{cells}\"")
      })
      .collect::<Vec<_>>()
      .join(" ")
  }

  /// Renders one grid placement axis to its `start / end` CSS text.
  #[must_use]
  pub fn render_grid_line(line: Line<GridPlacement>) -> String {
    let end = Self::render_placement(line.end);
    let mut out = Self::render_placement(line.start);

    out.push_str(" / ");
    out.push_str(&end);

    out
  }

  /// Renders one [`GridPlacement`] to its CSS text.
  #[must_use]
  fn render_placement(placement: GridPlacement) -> String {
    match placement {
      GridPlacement::Auto => "auto".to_owned(),
      GridPlacement::Line(index) => index.to_string(),
      GridPlacement::Span(count) => format!("span {count}"),
      GridPlacement::Named(name, 1) => Ident(name.as_str()).to_string(),
      GridPlacement::Named(name, index) => format!("{index} {}", Ident(name.as_str())),
      GridPlacement::NamedSpan(name, 1) => format!("span {}", Ident(name.as_str())),
      GridPlacement::NamedSpan(name, count) => {
        format!("span {count} {}", Ident(name.as_str()))
      }
    }
  }

  /// Every declaration as `(property, value)`, in canonical order.
  #[inline(always)]
  pub fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> {
    self.0.iter().map(|(property, value)| (*property, &**value))
  }

  /// Renders the declaration block (`prop:value;…`) in canonical order.
  pub(super) fn render(&self) -> String {
    let mut out = String::new();

    for (property, value) in &self.0 {
      out.push_str(property);
      out.push(':');
      out.push_str(value);
      out.push(';');
    }

    out
  }
}

/// A string written as a CSS identifier: every character that would end the identifier
/// or change what it means is escaped, so it names exactly the string.
pub(crate) struct Ident<'a>(pub(crate) &'a str);

impl Display for Ident<'_> {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    let leading_hyphen = self.0.starts_with('-');

    if self.0 == "-" {
      return formatter.write_str("\\-");
    }

    for (index, character) in self.0.chars().enumerate() {
      let code = u32::from(character);

      match character {
        '\0' => formatter.write_char('\u{fffd}')?,
        '\u{1}'..='\u{1f}' | '\u{7f}' => write!(formatter, "\\{code:x} ")?,
        '0'..='9' if index == 0 || (index == 1 && leading_hyphen) => {
          write!(formatter, "\\{code:x} ")?;
        }
        '-' | '_' | 'a'..='z' | 'A'..='Z' | '0'..='9' => formatter.write_char(character)?,
        _ if code >= 0x80 => formatter.write_char(character)?,
        _ => {
          formatter.write_char('\\')?;
          formatter.write_char(character)?;
        }
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::{Declarations, Ident};

  /// An identifier is written as is, and anything else escaped so it stays one.
  #[test]
  fn an_identifier_escapes_what_would_end_it() {
    assert_eq!(Ident("post-7").to_string(), "post-7");
    assert_eq!(Ident("héllo_world").to_string(), "héllo_world");
    assert_eq!(Ident("7up").to_string(), "\\37 up");
    assert_eq!(Ident("-7").to_string(), "-\\37 ");
    assert_eq!(Ident("-").to_string(), "\\-");
    assert_eq!(Ident("a\u{1}b").to_string(), "a\\1 b");
    assert_eq!(
      Ident("x}</style><img src=x>").to_string(),
      "x\\}\\<\\/style\\>\\<img\\ src\\=x\\>"
    );
  }

  /// Identical sets render identically regardless of insertion order, so they dedup.
  #[test]
  fn renders_in_canonical_order() {
    let a = Declarations::new().set("gap", "8px").set("color", "red");
    let b = Declarations::new().set("color", "red").set("gap", "8px");

    assert_eq!(a, b);
    assert_eq!(a.render(), b.render());
    assert_eq!(a.render(), "color:red;gap:8px;");
  }

  /// A later `set` for the same property replaces the earlier value.
  #[test]
  fn later_set_wins() {
    let declarations = Declarations::new().set("gap", "8px").set("gap", "16px");

    assert_eq!(declarations.render(), "gap:16px;");
  }
}
