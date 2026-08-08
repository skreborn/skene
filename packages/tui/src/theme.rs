//! Control glyphs inherited by a subtree.

use skene_core::{AnyView, IntoView, NodeId, View};
use skene_reactive::{Owner, provide_context, use_context};

/// The glyphs terminal controls render with.
///
/// Defaults: `☑`/`☐` for checkboxes, `◉`/`○` for radios, `▾` for the select indicator,
/// and `─`/`●` for a slider's track and thumb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Glyphs {
  /// A checkbox's checked and unchecked marks.
  pub(crate) checkbox: (char, char),
  /// A radio's checked and unchecked marks.
  pub(crate) radio: (char, char),
  /// The indicator after a select's current label.
  pub(crate) picker_indicator: char,
  /// A slider's track and thumb.
  pub(crate) slider: (char, char),
}

impl Default for Glyphs {
  fn default() -> Self {
    Self {
      checkbox: ('\u{2611}', '\u{2610}'),
      radio: ('\u{25c9}', '\u{25cb}'),
      picker_indicator: '\u{25be}',
      slider: ('\u{2500}', '\u{25cf}'),
    }
  }
}

impl Glyphs {
  /// The glyphs the current subtree inherits: the nearest enclosing theme's, or the
  /// defaults.
  pub(crate) fn current() -> Self {
    use_context::<Self>().unwrap_or_default()
  }
}

/// Control glyphs inherited by a subtree, threaded through the reactive `Context`.
///
/// Wrap a subtree to restyle the characters its controls render with: each setter
/// overrides that glyph for every control beneath, a nested theme overlays an outer
/// one, and a control's own glyph setters win over any theme.
#[must_use]
pub struct ControlTheme {
  child: AnyView,
  checkbox: Option<(char, char)>,
  radio: Option<(char, char)>,
  picker_indicator: Option<char>,
  slider: Option<(char, char)>,
}

impl ControlTheme {
  /// Creates a control theme wrapping `child`.
  #[inline(always)]
  pub fn new(child: impl IntoView) -> Self {
    Self {
      child: child.into_view(),
      checkbox: None,
      radio: None,
      picker_indicator: None,
      slider: None,
    }
  }

  /// Sets the checked and unchecked marks of checkboxes in the subtree.
  #[inline(always)]
  pub fn set_checkbox(mut self, checked: char, unchecked: char) -> Self {
    self.checkbox = Some((checked, unchecked));

    self
  }

  /// Sets the checked and unchecked marks of radios in the subtree.
  #[inline(always)]
  pub fn set_radio(mut self, checked: char, unchecked: char) -> Self {
    self.radio = Some((checked, unchecked));

    self
  }

  /// Sets the indicator after a select's current label in the subtree.
  #[inline(always)]
  pub fn set_picker_indicator(mut self, indicator: char) -> Self {
    self.picker_indicator = Some(indicator);

    self
  }

  /// Sets the track and thumb of sliders in the subtree.
  #[inline(always)]
  pub fn set_slider(mut self, track: char, thumb: char) -> Self {
    self.slider = Some((track, thumb));

    self
  }
}

impl View for ControlTheme {
  fn build(self) -> NodeId {
    let mut inherited = Glyphs::current();

    if let Some(checkbox) = self.checkbox {
      inherited.checkbox = checkbox;
    }

    if let Some(radio) = self.radio {
      inherited.radio = radio;
    }

    if let Some(indicator) = self.picker_indicator {
      inherited.picker_indicator = indicator;
    }

    if let Some(slider) = self.slider {
      inherited.slider = slider;
    }

    let scope = Owner::current().child();

    scope.enter(|| {
      provide_context(inherited);

      self.child.build()
    })
  }
}
