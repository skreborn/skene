//! The glyphs a medium that draws with characters shows its marks as, as a token set.

use skene_core::{Definition, Scalar, StaticLeaf, Token, TokenName, TokenRef, TokenSet};

/// A checkbox's checked mark.
pub const CHECKBOX_CHECKED: TokenRef<String> = TokenRef::new("glyph-checkbox-checked");

/// A checkbox's unchecked mark.
pub const CHECKBOX_UNCHECKED: TokenRef<String> = TokenRef::new("glyph-checkbox-unchecked");

/// A radio's checked mark.
pub const RADIO_CHECKED: TokenRef<String> = TokenRef::new("glyph-radio-checked");

/// A radio's unchecked mark.
pub const RADIO_UNCHECKED: TokenRef<String> = TokenRef::new("glyph-radio-unchecked");

/// The indicator after a picker's chosen label.
pub const PICKER_INDICATOR: TokenRef<String> = TokenRef::new("glyph-picker-indicator");

/// The glyphs a control's marks are drawn with where the medium draws with characters:
/// a token set a theme defines beneath it, over the facade's scalar `N`.
///
/// The terminal's checkbox and radio marks and its picker indicator are these glyphs; a
/// pixel medium draws its marks and defines the set to no effect. Every default is one
/// cell per character in every terminal. Box-symbol marks (`☑`, `◉`) render two cells
/// wide in some emulators; a theme opting into them takes that on itself.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Glyphs<N: Scalar> {
  /// A checkbox's checked mark.
  pub checkbox_checked: Token<String, N>,
  /// A checkbox's unchecked mark.
  pub checkbox_unchecked: Token<String, N>,
  /// A radio's checked mark.
  pub radio_checked: Token<String, N>,
  /// A radio's unchecked mark.
  pub radio_unchecked: Token<String, N>,
  /// The indicator after a picker's chosen label.
  pub picker_indicator: Token<String, N>,
}

impl<N: Scalar> Glyphs<N> {
  /// Sets the checked and unchecked marks of a checkbox.
  #[inline(always)]
  pub fn set_checkbox(mut self, checked: impl Into<String>, unchecked: impl Into<String>) -> Self {
    self.checkbox_checked = self.checkbox_checked.redefine(checked.into());
    self.checkbox_unchecked = self.checkbox_unchecked.redefine(unchecked.into());

    self
  }

  /// Sets the checked and unchecked marks of a radio.
  #[inline(always)]
  pub fn set_radio(mut self, checked: impl Into<String>, unchecked: impl Into<String>) -> Self {
    self.radio_checked = self.radio_checked.redefine(checked.into());
    self.radio_unchecked = self.radio_unchecked.redefine(unchecked.into());

    self
  }

  /// Sets the indicator after a picker's chosen label.
  #[inline(always)]
  pub fn set_picker_indicator(mut self, indicator: impl Into<String>) -> Self {
    self.picker_indicator = self.picker_indicator.redefine(indicator.into());

    self
  }

  /// The checkbox mark for `checked`.
  #[inline(always)]
  #[must_use]
  pub fn checkbox(&self, checked: bool) -> String {
    if checked {
      glyph(self.checkbox_checked.definition())
    }
    else {
      glyph(self.checkbox_unchecked.definition())
    }
  }

  /// The radio mark for `checked`.
  #[inline(always)]
  #[must_use]
  pub fn radio(&self, checked: bool) -> String {
    if checked {
      glyph(self.radio_checked.definition())
    }
    else {
      glyph(self.radio_unchecked.definition())
    }
  }

  /// The indicator after a picker's chosen label.
  #[inline(always)]
  #[must_use]
  pub fn indicator(&self) -> String {
    glyph(self.picker_indicator.definition())
  }
}

/// The glyph a definition fixes; the setters admit nothing else.
fn glyph<N: Scalar>(definition: &Definition<String, N>) -> String {
  match definition.base() {
    StaticLeaf::Fixed(glyph) => glyph.clone(),
    StaticLeaf::Token(_) | StaticLeaf::Expr(_) => String::new(),
  }
}

impl<N: Scalar> Default for Glyphs<N> {
  /// `[x]`/`[ ]` for a checkbox, `(x)`/`( )` for a radio and `▾` for the picker
  /// indicator.
  fn default() -> Self {
    let define = |reference: TokenRef<String>, glyph: &str| {
      Token::new(reference.name().as_str(), glyph.to_owned())
    };

    Self {
      checkbox_checked: define(CHECKBOX_CHECKED, "[x]"),
      checkbox_unchecked: define(CHECKBOX_UNCHECKED, "[ ]"),
      radio_checked: define(RADIO_CHECKED, "(x)"),
      radio_unchecked: define(RADIO_UNCHECKED, "( )"),
      picker_indicator: define(PICKER_INDICATOR, "\u{25be}"),
    }
  }
}

impl<N: Scalar, V: From<Definition<String, N>> + 'static> TokenSet<V> for Glyphs<N> {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![
      self.checkbox_checked.define(),
      self.checkbox_unchecked.define(),
      self.radio_checked.define(),
      self.radio_unchecked.define(),
      self.picker_indicator.define(),
    ]
  }
}
