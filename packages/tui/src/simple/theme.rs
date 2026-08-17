//! The Simple design language's theme: the palettes a subtree paints with, and the
//! setters that read them.

use skene_core::{AnyView, Color, ColorScheme, IntoView, NoChild, NodeId, View};
use skene_simple::{Palette, Palettes};

use crate::{
  Container,
  HasDecoration,
  Span,
  Text,
  props::{Condition, RunStyle},
};

/// The Simple palettes a subtree paints with, threaded through the reactive `Context`.
///
/// Wrap a subtree to dress every `Simple*` view beneath it: the theme holds a light
/// and a dark palette, follows the terminal's detected background by default, and a
/// nested theme overlays an outer one. A themed property is spelled with the `theme_`
/// setters, so a component library and an app dress their own chrome from the same
/// palettes the built-in components use.
///
/// Each themed property compiles to its light value plus a dark-scheme variant, which
/// the terminal resolves against the background it detected—so a subtree re-inks itself
/// when the terminal's own scheme changes. [`set_scheme`](Self::set_scheme) fixes the
/// scheme instead, and resolves each property to a single value.
///
/// To bind the scheme to a user setting, rebuild the theme from the signal holding it:
///
/// ```no_run
/// # use skene_tui::prelude::*;
/// # fn page() -> impl IntoView { Text::new("") }
/// # let scheme = Signal::new(ColorScheme::Dark);
/// DerivedView::new(move || SimpleTheme::new().set_scheme(scheme.get()).set_child(page))
/// # ;
/// ```
///
/// The subtree rebuilds on each change, which is what keeps the conditional variants
/// correct: a variant is static data by the time it reaches the paint, so a reactive
/// scheme cannot reach one any other way.
#[must_use]
pub struct SimpleTheme<C = NoChild> {
  child: C,
  palettes: Palettes,
}

impl SimpleTheme {
  /// Creates a theme wrapping its child, carrying the stock palettes and following the
  /// terminal's detected background.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      child: NoChild,
      palettes: Palettes::default(),
    }
  }
}

impl<C> SimpleTheme<C> {
  /// Sets the child, which the view requires before it can build.
  ///
  /// The child comes as a closure because the palettes have to be in scope while it is
  /// built: a `theme_` setter resolves its token the moment it is called, and a child
  /// passed by value would already have been built by then.
  #[inline(always)]
  pub fn set_child<V: IntoView>(self, child: impl FnOnce() -> V + 'static) -> SimpleTheme<AnyView> {
    SimpleTheme {
      child: AnyView::new(move || child().into_view().build()),
      palettes: self.palettes,
    }
  }

  /// Sets the palette painted under the light scheme.
  #[inline(always)]
  pub fn set_light(mut self, palette: Palette) -> Self {
    self.palettes.light = palette;

    self
  }

  /// Sets the palette painted under the dark scheme.
  #[inline(always)]
  pub fn set_dark(mut self, palette: Palette) -> Self {
    self.palettes.dark = palette;

    self
  }

  /// Fixes the subtree to one scheme, overriding the terminal's.
  #[inline(always)]
  pub fn set_scheme(mut self, scheme: ColorScheme) -> Self {
    self.palettes.scheme = Some(scheme);

    self
  }
}

impl Default for SimpleTheme {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl View for SimpleTheme<AnyView> {
  fn build(self) -> NodeId {
    self.palettes.provide(|| self.child.build())
  }
}

impl skene_simple::SchemeCondition for Condition {
  #[inline(always)]
  fn dark_mode() -> Self {
    Self::DarkMode
  }

  #[inline(always)]
  fn with_dark_mode(self) -> Self {
    Self::and([self, Self::DarkMode])
  }
}

/// The palette in force this frame: the theme's fixed one, or the one matching the
/// background the terminal detected.
///
/// For the properties a variant cannot reach—a popup's whole style, a highlight's
/// color pair—which the terminal resolves eagerly instead.
pub(crate) fn painted_palette() -> Palette {
  let palettes = Palettes::current();

  match palettes.fixed() {
    Some(palette) => *palette,
    None if crate::with_state(|state| state.dark.get()) => palettes.dark,
    None => palettes.light,
  }
}

/// The themed twin of [`Container`]'s decoration setters: each takes the token to paint
/// rather than a color, and resolves it against the enclosing [`SimpleTheme`].
pub trait ThemedBox: Sized {
  /// Sets the background to `token`.
  #[must_use]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self;

  /// Sets the background to `token` while `condition` holds.
  #[must_use]
  fn theme_color_when(self, condition: Condition, token: impl Fn(&Palette) -> Color) -> Self;

  /// Sets the border to `token`.
  #[must_use]
  fn theme_border(self, token: impl Fn(&Palette) -> Color) -> Self;

  /// Sets the border to `token` while `condition` holds.
  #[must_use]
  fn theme_border_when(self, condition: Condition, token: impl Fn(&Palette) -> Color) -> Self;
}

impl ThemedBox for Container {
  #[inline(always)]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(self, token, Self::set_color, Self::set_color_when)
  }

  #[inline(always)]
  fn theme_color_when(self, condition: Condition, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply_when(self, condition, token, Self::set_color_when)
  }

  #[inline(always)]
  fn theme_border(self, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(self, token, Self::set_border, Self::set_border_when)
  }

  #[inline(always)]
  fn theme_border_when(self, condition: Condition, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply_when(self, condition, token, Self::set_border_when)
  }
}

/// The themed twin of [`HasDecoration`]'s setters: each takes the token to paint rather
/// than a color, and resolves it against the enclosing [`SimpleTheme`].
pub trait ThemedControl: HasDecoration {
  /// Sets the background to `token`.
  #[inline(always)]
  #[must_use]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(self, token, Self::set_color, Self::set_color_when)
  }

  /// Sets the background to `token` while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn theme_color_when(self, condition: Condition, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply_when(self, condition, token, Self::set_color_when)
  }

  /// Sets the border to `token`.
  #[inline(always)]
  #[must_use]
  fn theme_border(self, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(self, token, Self::set_border, Self::set_border_when)
  }

  /// Sets the border to `token` while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn theme_border_when(self, condition: Condition, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply_when(self, condition, token, Self::set_border_when)
  }
}

impl<T: HasDecoration> ThemedControl for T {}

/// The themed twin of a run's ink setter, on the views that carry one.
pub trait ThemedText: Sized {
  /// Sets the ink to `token`.
  #[must_use]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self;
}

impl ThemedText for Span {
  /// A span carries no conditional variants, so its ink resolves against the scheme in
  /// force when the span is built rather than following a later change.
  #[inline(always)]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self {
    self.set_color(token(&painted_palette()))
  }
}

impl ThemedText for Text {
  #[inline(always)]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(self, token, Self::set_color, |target, condition, color| {
      target.set_when(condition, RunStyle::new().set_color(color))
    })
  }
}
