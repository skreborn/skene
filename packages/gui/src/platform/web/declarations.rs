//! Raw CSS declarations: the escape hatch for styling a node with arbitrary CSS.
//!
//! A low-level custom view builds [`Declarations`] and applies them with
//! [`set_style`](super::set_style), or drives the batched path
//! through [`Styler`](super::Styler). A declaration's value is CSS text the caller
//! renders — `"8px"`, `"oklch(0.7 0.1 250 / 1)"`, `"flex"`, `"repeat(3, 1fr)"` — so the
//! full open-ended CSS value space is reachable and nothing is special-cased.
//!
//! Declarations are keyed by property and sorted, so a set is deterministic and
//! content-addressed for class dedup and server-side rendering.

use std::borrow::Cow;

use skene_core::{Color, CrossAxisAlignment, MainAxisAlignment, MouseCursor};
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
    Declarations::default()
  }

  /// Sets one declaration to its CSS text, replacing any previous value for that
  /// property. A `&'static str` (a keyword) is stored without allocating; a rendered
  /// `String` is taken by value.
  #[inline(always)]
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
  pub fn render_cursor(cursor: MouseCursor) -> &'static str {
    match cursor {
      MouseCursor::Default => "default",
      MouseCursor::Click => "pointer",
      MouseCursor::Text => "text",
    }
  }

  /// Renders a [`MainAxisAlignment`] to its CSS `justify-content` keyword.
  #[must_use]
  pub fn render_main_axis(alignment: MainAxisAlignment) -> &'static str {
    match alignment {
      MainAxisAlignment::Start => "flex-start",
      MainAxisAlignment::Center => "center",
      MainAxisAlignment::End => "flex-end",
      MainAxisAlignment::SpaceBetween => "space-between",
      MainAxisAlignment::SpaceAround => "space-around",
      MainAxisAlignment::SpaceEvenly => "space-evenly",
    }
  }

  /// Renders a [`CrossAxisAlignment`] to its CSS `align-items` keyword.
  #[must_use]
  pub fn render_cross_axis(alignment: CrossAxisAlignment) -> &'static str {
    match alignment {
      CrossAxisAlignment::Start => "flex-start",
      CrossAxisAlignment::End => "flex-end",
      CrossAxisAlignment::Center => "center",
      CrossAxisAlignment::Stretch => "stretch",
      CrossAxisAlignment::Baseline => "baseline",
    }
  }

  /// Iterates the declarations as `(property, value)` in canonical order.
  pub(super) fn iter(&self) -> impl Iterator<Item = (&'static str, &str)> {
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

#[cfg(test)]
mod tests {
  use super::Declarations;

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
