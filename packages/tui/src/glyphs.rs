//! The glyphs terminal controls draw with, as a token set.

use skene_core::{TokenName, TokenRef, TokenSet};

use crate::{Definition, Token};

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

/// A slider's track.
pub const SLIDER_TRACK: TokenRef<String> = TokenRef::new("glyph-slider-track");

/// A slider's thumb.
pub const SLIDER_THUMB: TokenRef<String> = TokenRef::new("glyph-slider-thumb");

/// The glyphs terminal controls draw with: a token set a theme defines beneath it, and
/// a terminal-aware view reads through [`ThemedIn`](crate::ThemedIn).
///
/// Every default is one cell per character in every terminal. Box-symbol marks (`☑`,
/// `◉`) render two cells wide in some emulators; a theme opting into them takes that
/// on itself.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Glyphs {
  /// A checkbox's checked mark.
  pub checkbox_checked: Token<String>,
  /// A checkbox's unchecked mark.
  pub checkbox_unchecked: Token<String>,
  /// A radio's checked mark.
  pub radio_checked: Token<String>,
  /// A radio's unchecked mark.
  pub radio_unchecked: Token<String>,
  /// The indicator after a picker's chosen label.
  pub picker_indicator: Token<String>,
  /// A slider's track.
  pub slider_track: Token<String>,
  /// A slider's thumb.
  pub slider_thumb: Token<String>,
}

impl Glyphs {
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

  /// Sets the track and thumb of a slider.
  #[inline(always)]
  pub fn set_slider(mut self, track: impl Into<String>, thumb: impl Into<String>) -> Self {
    self.slider_track = self.slider_track.redefine(track.into());
    self.slider_thumb = self.slider_thumb.redefine(thumb.into());

    self
  }
}

impl Default for Glyphs {
  /// `[x]`/`[ ]` for a checkbox, `(x)`/`( )` for a radio, `▾` for the picker indicator,
  /// and `█`/`█` for a slider.
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
      slider_track: define(SLIDER_TRACK, "\u{2588}"),
      slider_thumb: define(SLIDER_THUMB, "\u{2588}"),
    }
  }
}

impl<V: From<Definition<String>> + 'static> TokenSet<V> for Glyphs {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![
      self.checkbox_checked.define(),
      self.checkbox_unchecked.define(),
      self.radio_checked.define(),
      self.radio_unchecked.define(),
      self.picker_indicator.define(),
      self.slider_track.define(),
      self.slider_thumb.define(),
    ]
  }
}
