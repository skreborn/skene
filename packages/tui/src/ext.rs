//! The terminal's decoration and text setters on the facade's views: border glyphs, a
//! shadow, boldness.

use skene_core::{BoxStyle, Color, Condition, Declared, Leaf, RunStyle, StaticLeaf, Value};
use skene_tui_core::{BorderGlyphs, TuiExt};

use crate::Tui;

/// The declared value of a terminal property: a leaf under the terminal's conditions.
type Spelled<T> = Value<T, Condition<u32>>;

/// The declared style of a terminal box.
type DeclaredBox = BoxStyle<u32, Declared<Condition<u32>>, TuiExt>;

/// The declared style of a terminal run.
type DeclaredRun = RunStyle<Declared<Condition<u32>>, TuiExt>;

/// The terminal's own decoration on a box: the glyphs its border draws with and the
/// shadow it casts.
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

  /// Sets the shadow color, cast one cell down and right; its alpha is the strength of
  /// the mix into the colors beneath.
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
}

impl TuiDecoration for skene_views::Container<Tui> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
  }
}

impl<B: skene_views::Behavior<Tui>, C> TuiDecoration for skene_views::Control<Tui, B, C> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredBox {
    self.style_mut()
  }
}

impl<C> TuiDecoration for skene_views::Overlay<Tui, C> {
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

impl TuiText for skene_views::Text<Tui> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun {
    self.style_mut()
  }
}

impl TuiText for skene_views::Span<Tui> {
  #[inline(always)]
  fn terminal_style_mut(&mut self) -> &mut DeclaredRun {
    self.style_mut()
  }
}
