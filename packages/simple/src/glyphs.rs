//! The glyphs the Simple controls show their marks as where the medium draws with
//! characters, as a token set.

use skene_core::{Definition, StaticLeaf, Token, TokenName, TokenRef, TokenSet};
use skene_reactive::{Get, Memo, use_context};

/// A checkbox's checked mark.
pub const CHECKBOX_CHECKED: TokenRef<String> = TokenRef::new("simple-glyph-checkbox-checked");

/// A checkbox's unchecked mark.
pub const CHECKBOX_UNCHECKED: TokenRef<String> = TokenRef::new("simple-glyph-checkbox-unchecked");

/// A radio's checked mark.
pub const RADIO_CHECKED: TokenRef<String> = TokenRef::new("simple-glyph-radio-checked");

/// A radio's unchecked mark.
pub const RADIO_UNCHECKED: TokenRef<String> = TokenRef::new("simple-glyph-radio-unchecked");

/// The indicator after a picker's chosen label.
pub const PICKER_INDICATOR: TokenRef<String> = TokenRef::new("simple-glyph-picker-indicator");

/// The glyphs the Simple checkbox and radio marks and picker indicator are drawn with
/// where the medium draws with characters: a token set a theme defines beneath it, in
/// cells.
///
/// It exists only for a driver whose tokens hold text over cells—the terminal—so a
/// pixel medium, which draws its marks, cannot theme it. Every default is one cell per
/// character in every terminal. Box-symbol marks (`☑`, `◉`) render two cells wide in
/// some emulators; a theme opting into them takes that on itself.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct SimpleGlyphs {
  /// A checkbox's checked mark.
  pub checkbox_checked: Token<String, u32>,
  /// A checkbox's unchecked mark.
  pub checkbox_unchecked: Token<String, u32>,
  /// A radio's checked mark.
  pub radio_checked: Token<String, u32>,
  /// A radio's unchecked mark.
  pub radio_unchecked: Token<String, u32>,
  /// The indicator after a picker's chosen label.
  pub picker_indicator: Token<String, u32>,
}

impl SimpleGlyphs {
  /// The set the nearest theme provides, read tracked; the default without one.
  #[inline(always)]
  pub fn current() -> Self {
    use_context::<Memo<Self>>().map_or_else(Self::default, |glyphs| glyphs.get())
  }

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
fn glyph(definition: &Definition<String, u32>) -> String {
  match definition.base() {
    StaticLeaf::Fixed(glyph) => glyph.clone(),
    StaticLeaf::Token(_) | StaticLeaf::Expr(_) => String::new(),
  }
}

impl Default for SimpleGlyphs {
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

impl<V: From<Definition<String, u32>> + 'static> TokenSet<V> for SimpleGlyphs {
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
