//! Raw CSS declarations: the escape hatch for styling a node with arbitrary CSS.
//!
//! A low-level custom view builds [`Declarations`] and applies them with
//! [`WebBackend::set_css`](crate::WebBackend::set_css). A declaration's value is a
//! [`StyleValue`], which centralizes how each *kind* of CSS value renders (a
//! [`Length`] as `px`, a [`Color`] in its own color space, …) while [`StyleValue::Custom`] passes
//! any CSS text through verbatim, so the full open-ended CSS value space is reachable.
//!
//! Declarations are keyed by property and sorted, so a set is deterministic and
//! content-addressed for class dedup and server-side rendering.

use std::fmt::Write;

use skene_core::{Color, Inset, Number};
use smallvec::SmallVec;

/// A CSS declaration value, by kind.
///
/// The typed variants render themselves ([`Length`] → `px`, [`Color`] → its color space, …);
/// [`StyleValue::Custom`] carries arbitrary CSS text for anything the typed
/// variants don't cover (`calc()`, `var()`, gradients, future syntax).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[must_use]
pub enum StyleValue {
  /// A `<length>`, rendered in logical pixels.
  Length(Number),
  /// A `<color>`, rendered in the color space it was defined in.
  Color(Color),
  /// A unitless flex `<number>`.
  Number(Number),
  /// Four-sided spacing, rendered as the `top right bottom left` shorthand.
  Padding(Inset),
  /// A `<url>`, rendered as `url("…")`.
  Url(String),
  /// A CSS keyword or identifier (`flex`, `center`, `solid`, …).
  Keyword(&'static str),
  /// Arbitrary CSS text, rendered verbatim.
  Custom(String),
}

impl StyleValue {
  /// Renders this value to its CSS text.
  #[must_use]
  pub fn to_css(&self) -> String {
    let mut out = String::new();

    self.write_css(&mut out);

    out
  }

  /// Writes this value's CSS text into `out`, so a caller can reuse one buffer
  /// instead of allocating a `String` per value.
  pub fn write_css(&self, out: &mut String) {
    match self {
      Self::Length(value) => {
        _ = write!(out, "{}px", f64::from(*value));
      }
      Self::Color(value) => {
        let [lightness, chroma, hue, alpha] = value.to_oklch();

        _ = write!(out, "oklch({lightness} {chroma} {hue} / {alpha})");
      }
      Self::Number(value) => {
        _ = write!(out, "{}", f64::from(*value));
      }
      Self::Padding(value) => {
        _ = write!(
          out,
          "{}px {}px {}px {}px",
          f64::from(value.top),
          f64::from(value.right),
          f64::from(value.bottom),
          f64::from(value.left),
        );
      }
      Self::Url(value) => {
        _ = write!(out, "url(\"{value}\")");
      }
      Self::Keyword(value) => out.push_str(value),
      Self::Custom(value) => out.push_str(value),
    }
  }
}

/// A set of CSS declarations (`property: value`), kept sorted by property.
///
/// Backed by an inline [`SmallVec`], so the small sets a view has need no heap
/// allocation, and the sorted order makes the set deterministic and content-addressed
/// for class dedup and server-side rendering.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[must_use]
pub struct Declarations(SmallVec<[(&'static str, StyleValue); 8]>);

impl Declarations {
  /// Creates an empty declaration set.
  #[inline(always)]
  pub fn new() -> Self {
    Declarations::default()
  }

  /// Sets one declaration, replacing any previous value for that property.
  #[inline(always)]
  pub fn set(mut self, property: &'static str, value: impl Into<StyleValue>) -> Self {
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

  /// Iterates the declarations as `(property, value)` in canonical order.
  pub(crate) fn iter(&self) -> impl Iterator<Item = (&'static str, &StyleValue)> {
    self.0.iter().map(|(property, value)| (*property, value))
  }

  /// Renders the declaration block (`prop:value;…`) in canonical order.
  pub(crate) fn render(&self) -> String {
    let mut out = String::new();

    for (property, value) in &self.0 {
      out.push_str(property);
      out.push(':');
      value.write_css(&mut out);
      out.push(';');
    }

    out
  }
}

#[cfg(test)]
mod tests {
  use skene_core::{Color, Inset};

  use super::{Declarations, StyleValue};

  /// Each value kind renders itself to the right CSS text.
  #[test]
  fn style_values_render() {
    assert_eq!(StyleValue::Length(8.into()).to_css(), "8px");
    assert_eq!(
      StyleValue::Color(Color::TRANSPARENT).to_css(),
      "oklch(0 0 0 / 0)"
    );
    assert_eq!(StyleValue::Keyword("center").to_css(), "center");
    assert_eq!(
      StyleValue::Custom("calc(100% - 8px)".into()).to_css(),
      "calc(100% - 8px)"
    );
    assert_eq!(
      StyleValue::Padding(Inset::symmetric(20.0, 10.0)).to_css(),
      "10px 20px 10px 20px"
    );
  }

  /// Identical sets render identically regardless of insertion order, so they dedup.
  #[test]
  fn renders_in_canonical_order() {
    let a = Declarations::new()
      .set("gap", StyleValue::Length(8.into()))
      .set("color", StyleValue::Color(Color::TRANSPARENT));
    let b = Declarations::new()
      .set("color", StyleValue::Color(Color::TRANSPARENT))
      .set("gap", StyleValue::Length(8.into()));

    assert_eq!(a, b);
    assert_eq!(a.render(), b.render());
    assert_eq!(a.render(), "color:oklch(0 0 0 / 0);gap:8px;");
  }

  /// A later `set` for the same property replaces the earlier value.
  #[test]
  fn later_set_wins() {
    let declarations = Declarations::new()
      .set("gap", StyleValue::Length(8.into()))
      .set("gap", StyleValue::Length(16.into()));

    assert_eq!(declarations.render(), "gap:16px;");
  }
}
