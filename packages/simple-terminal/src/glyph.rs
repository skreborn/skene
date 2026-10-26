//! The glyphs the Simple controls show their marks as in the terminal, as a token set.

use skene_foundation::{Definition, StaticLeaf, Token, TokenName, TokenRef, TokenSet};
use skene_terminal::reactive::{Get, Memo, use_context};

/// A checkbox's checked mark.
pub const CHECKBOX_CHECKED: TokenRef<String> =
  TokenRef::of::<SimpleGlyphs, i32>("simple-glyph-checkbox-checked");

/// A checkbox's unchecked mark.
pub const CHECKBOX_UNCHECKED: TokenRef<String> =
  TokenRef::of::<SimpleGlyphs, i32>("simple-glyph-checkbox-unchecked");

/// A radio's checked mark.
pub const RADIO_CHECKED: TokenRef<String> =
  TokenRef::of::<SimpleGlyphs, i32>("simple-glyph-radio-checked");

/// A radio's unchecked mark.
pub const RADIO_UNCHECKED: TokenRef<String> =
  TokenRef::of::<SimpleGlyphs, i32>("simple-glyph-radio-unchecked");

/// The indicator after a picker's chosen label.
pub const PICKER_INDICATOR: TokenRef<String> =
  TokenRef::of::<SimpleGlyphs, i32>("simple-glyph-picker-indicator");

/// The glyphs the Simple checkbox and radio marks and picker indicator are drawn with in
/// the terminal: a token set a theme defines beneath it, in cells.
///
/// Every default is one cell per character in every terminal. Box-symbol marks (`☑`,
/// `◉`) render two cells wide in some emulators; a theme opting into them takes that on
/// itself.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct SimpleGlyphs {
  /// A checkbox's checked mark.
  pub checkbox_checked: Token<String, i32>,
  /// A checkbox's unchecked mark.
  pub checkbox_unchecked: Token<String, i32>,
  /// A radio's checked mark.
  pub radio_checked: Token<String, i32>,
  /// A radio's unchecked mark.
  pub radio_unchecked: Token<String, i32>,
  /// The indicator after a picker's chosen label.
  pub picker_indicator: Token<String, i32>,
}

impl SimpleGlyphs {
  /// The set the nearest theme provides, read tracked; the default without one.
  #[inline(always)]
  pub fn current() -> Self {
    use_context::<Memo<Self>>().map_or_else(Self::default, |glyphs| glyphs.get())
  }

  /// Sets the checked and unchecked marks of a checkbox.
  pub fn set_checkbox(mut self, checked: impl Into<String>, unchecked: impl Into<String>) -> Self {
    self.checkbox_checked = self.checkbox_checked.redefine(checked.into());
    self.checkbox_unchecked = self.checkbox_unchecked.redefine(unchecked.into());

    self
  }

  /// Sets the checked and unchecked marks of a radio.
  pub fn set_radio(mut self, checked: impl Into<String>, unchecked: impl Into<String>) -> Self {
    self.radio_checked = self.radio_checked.redefine(checked.into());
    self.radio_unchecked = self.radio_unchecked.redefine(unchecked.into());

    self
  }

  /// Sets the indicator after a picker's chosen label.
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
fn glyph(definition: &Definition<String, i32>) -> String {
  match definition.base() {
    StaticLeaf::Fixed(glyph) => glyph.clone(),
    StaticLeaf::Token(_) | StaticLeaf::Expr(_) => String::new(),
  }
}

impl Default for SimpleGlyphs {
  /// `[x]`/`[ ]` for a checkbox, `(x)`/`( )` for a radio and `▾` for the picker
  /// indicator.
  fn default() -> Self {
    let define = |reference: TokenRef<String>, glyph: &str| Token::new(reference, glyph.to_owned());

    Self {
      checkbox_checked: define(CHECKBOX_CHECKED, "[x]"),
      checkbox_unchecked: define(CHECKBOX_UNCHECKED, "[ ]"),
      radio_checked: define(RADIO_CHECKED, "(x)"),
      radio_unchecked: define(RADIO_UNCHECKED, "( )"),
      picker_indicator: define(PICKER_INDICATOR, "\u{25be}"),
    }
  }
}

impl<V: From<Definition<String, i32>> + 'static> TokenSet<V> for SimpleGlyphs {
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
