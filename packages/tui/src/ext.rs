//! The terminal's decoration and text setters on the facade's views: border glyphs, a
//! shadow, scrollbars, boldness.

use skene_common::tui::{BorderGlyphs, ScrollbarGlyphs, TuiExt};
use skene_core::{BoxStyle, Color, Condition, Declared, Leaf, Length, RunStyle, StaticLeaf, Value};

use crate::Tui;

/// The declared value of a terminal property: a leaf under the terminal's conditions.
type Spelled<T> = Value<T, Condition<u32>>;

/// The declared style of a terminal box.
type DeclaredBox = BoxStyle<u32, Declared<Condition<u32>>, TuiExt>;

/// The declared style of a terminal run.
type DeclaredRun = RunStyle<Declared<Condition<u32>>, TuiExt>;

/// The terminal's own decoration on a box: the glyphs its border draws with, the
/// shadow it casts and its scrollbars.
///
/// Every view with a decorated box on the active platform has it: [`Container`](crate::Container),
/// [`Control`](crate::Control) and [`Overlay`](crate::Overlay).
pub trait TuiDecoration: Sized {
  /// The declared style the decoration lands on.
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox;

  /// Sets the characters the border draws with.
  #[inline(always)]
  #[must_use]
  fn set_border_glyphs(mut self, glyphs: impl Into<Spelled<BorderGlyphs>>) -> Self {
    self.terminal_style_mut().ext.border_glyphs = Some(glyphs.into());

    self
  }

  /// Derives the characters the border draws with from the signals `glyphs` reads: a
  /// value, a token or an expression.
  #[inline(always)]
  #[must_use]
  fn derive_border_glyphs<R: Into<StaticLeaf<BorderGlyphs>>>(
    mut self,
    glyphs: impl Fn() -> R + 'static,
  ) -> Self {
    self.terminal_style_mut().ext.border_glyphs = Some(Value::new(Leaf::derive(glyphs)));

    self
  }

  /// Sets the shadow color; its alpha is the strength of the mix into the colors
  /// beneath where the shadow falls.
  #[inline(always)]
  #[must_use]
  fn set_shadow(mut self, shadow: impl Into<Spelled<Color>>) -> Self {
    self.terminal_style_mut().ext.shadow = Some(shadow.into());

    self
  }

  /// Derives the shadow color from the signals `shadow` reads: a value, a token or an
  /// expression.
  #[inline(always)]
  #[must_use]
  fn derive_shadow<R: Into<StaticLeaf<Color>>>(mut self, shadow: impl Fn() -> R + 'static) -> Self {
    self.terminal_style_mut().ext.shadow = Some(Value::new(Leaf::derive(shadow)));

    self
  }

  /// Sets how far the shadow falls down and right, in cells: the cells it reaches, the
  /// offset rounded up, and its exact place where the terminal shows graphics.
  #[inline(always)]
  #[must_use]
  fn set_shadow_offset(mut self, offset: impl Into<Spelled<Length>>) -> Self {
    self.terminal_style_mut().ext.shadow_offset = Some(offset.into());

    self
  }

  /// Derives the shadow's offset from the signals `offset` reads: a value, a token or an
  /// expression.
  #[inline(always)]
  #[must_use]
  fn derive_shadow_offset<R: Into<StaticLeaf<Length>>>(
    mut self,
    offset: impl Fn() -> R + 'static,
  ) -> Self {
    self.terminal_style_mut().ext.shadow_offset = Some(Value::new(Leaf::derive(offset)));

    self
  }

  /// Sets how far the shadow's edge softens past its offset, in cells, where the
  /// terminal shows graphics.
  #[inline(always)]
  #[must_use]
  fn set_shadow_softening(mut self, softening: impl Into<Spelled<Length>>) -> Self {
    self.terminal_style_mut().ext.shadow_softening = Some(softening.into());

    self
  }

  /// Derives the shadow's softening from the signals `softening` reads: a value, a
  /// token or an expression.
  #[inline(always)]
  #[must_use]
  fn derive_shadow_softening<R: Into<StaticLeaf<Length>>>(
    mut self,
    softening: impl Fn() -> R + 'static,
  ) -> Self {
    self.terminal_style_mut().ext.shadow_softening = Some(Value::new(Leaf::derive(softening)));

    self
  }

  /// Sets the color of the scrollbars' thumbs.
  #[inline(always)]
  #[must_use]
  fn set_scrollbar_thumb(mut self, thumb: impl Into<Spelled<Color>>) -> Self {
    self.terminal_style_mut().ext.scrollbar_thumb = Some(thumb.into());

    self
  }

  /// Derives the color of the scrollbars' thumbs from the signals `thumb` reads: a
  /// value, a token or an expression.
  #[inline(always)]
  #[must_use]
  fn derive_scrollbar_thumb<R: Into<StaticLeaf<Color>>>(
    mut self,
    thumb: impl Fn() -> R + 'static,
  ) -> Self {
    self.terminal_style_mut().ext.scrollbar_thumb = Some(Value::new(Leaf::derive(thumb)));

    self
  }

  /// Sets the color of the scrollbars' tracks.
  #[inline(always)]
  #[must_use]
  fn set_scrollbar_track(mut self, track: impl Into<Spelled<Color>>) -> Self {
    self.terminal_style_mut().ext.scrollbar_track = Some(track.into());

    self
  }

  /// Derives the color of the scrollbars' tracks from the signals `track` reads: a
  /// value, a token or an expression.
  #[inline(always)]
  #[must_use]
  fn derive_scrollbar_track<R: Into<StaticLeaf<Color>>>(
    mut self,
    track: impl Fn() -> R + 'static,
  ) -> Self {
    self.terminal_style_mut().ext.scrollbar_track = Some(Value::new(Leaf::derive(track)));

    self
  }

  /// Sets the characters the scrollbars draw with.
  #[inline(always)]
  #[must_use]
  fn set_scrollbar_glyphs(mut self, glyphs: impl Into<Spelled<ScrollbarGlyphs>>) -> Self {
    self.terminal_style_mut().ext.scrollbar_glyphs = Some(glyphs.into());

    self
  }

  /// Derives the characters the scrollbars draw with from the signals `glyphs` reads: a
  /// value, a token or an expression.
  #[inline(always)]
  #[must_use]
  fn derive_scrollbar_glyphs<R: Into<StaticLeaf<ScrollbarGlyphs>>>(
    mut self,
    glyphs: impl Fn() -> R + 'static,
  ) -> Self {
    self.terminal_style_mut().ext.scrollbar_glyphs = Some(Value::new(Leaf::derive(glyphs)));

    self
  }
}

impl TuiDecoration for skene_common::Container<Tui> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
  }
}

impl<B: skene_common::Behavior<Tui>> TuiDecoration for skene_common::Control<Tui, B> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
  }
}

impl TuiDecoration for skene_common::Overlay<Tui> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
  }
}

/// The terminal's own text property: whether a run is bold.
///
/// Every text view on the active platform has it: [`Text`](crate::Text) and
/// [`Span`](crate::Span).
pub trait TuiText: Sized {
  /// The declared style the weight lands on.
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun;

  /// Sets whether the text is bold.
  #[inline(always)]
  #[must_use]
  fn set_bold(mut self, bold: impl Into<Spelled<bool>>) -> Self {
    self.terminal_style_mut().ext.bold = Some(bold.into());

    self
  }

  /// Derives whether the text is bold from the signals `bold` reads: a value, a token
  /// or an expression.
  #[inline(always)]
  #[must_use]
  fn derive_bold<R: Into<StaticLeaf<bool>>>(mut self, bold: impl Fn() -> R + 'static) -> Self {
    self.terminal_style_mut().ext.bold = Some(Value::new(Leaf::derive(bold)));

    self
  }
}

impl TuiText for skene_common::Text<Tui> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun {
    self.style_mut()
  }
}

impl TuiText for skene_common::Span<Tui> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun {
    self.style_mut()
  }
}
