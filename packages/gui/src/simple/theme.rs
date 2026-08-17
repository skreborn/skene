//! The Simple design language's theme: the palettes a subtree paints with, and the
//! setters that read them.

use skene_simple::{Palette, Palettes};

use crate::{
  AnyView,
  Border,
  Color,
  ColorScheme,
  Condition,
  Container,
  HasDecoration,
  IntoView,
  NoChild,
  NodeId,
  RunStyle,
  Span,
  Text,
  TextStyle,
  TextTheme,
  View,
};

/// The Simple palettes a subtree paints with, threaded through the reactive `Context`.
///
/// Wrap a subtree to dress every `Simple*` view beneath it: the theme holds a light
/// and a dark palette, follows the platform's scheme by default, and a nested theme
/// overlays an outer one. A themed property is spelled with the `theme_` setters, so
/// a component library and an app dress their own chrome from the same palettes the
/// built-in components use.
///
/// Following the platform costs no script: each themed property compiles to its light
/// value plus a dark-scheme variant, which the web renders as a `prefers-color-scheme`
/// media query, so a server-rendered page answers the user's preference before any
/// bundle arrives. [`set_scheme`](Self::set_scheme) fixes the scheme instead, and
/// resolves each property to a single value.
///
/// To bind the scheme to a user setting, rebuild the theme from the signal holding it:
///
/// ```no_run
/// # use skene_gui::prelude::*;
/// # fn page() -> impl IntoView { Text::new("") }
/// # let scheme = Signal::new(ColorScheme::Dark);
/// DerivedView::new(move || SimpleTheme::new().set_scheme(scheme.get()).set_child(page))
/// # ;
/// ```
///
/// The subtree rebuilds on each change, which is what keeps the conditional variants
/// correct: a variant is static data by the time it reaches a driver, so a reactive
/// scheme cannot reach one any other way.
#[must_use]
pub struct SimpleTheme<C = NoChild> {
  child: C,
  palettes: Palettes,
}

impl SimpleTheme {
  /// Creates a theme wrapping its child, carrying the stock palettes and following the
  /// platform's scheme.
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

  /// Fixes the subtree to one scheme, overriding the platform's.
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
    let Self { child, palettes } = self;

    palettes.provide(move || ambient_text(&palettes).set_child(child).build())
  }
}

/// The theme's ambient run style: the ink every run beneath inherits, and the colors a
/// selection paints in. A run's own setters still win, so a control's label keeps its
/// own ink.
fn ambient_text(palettes: &Palettes) -> TextTheme {
  let ink = |palette: &Palette| {
    TextStyle::new()
      .set_color(palette.foreground)
      .set_selection_color(palette.background)
      .set_selection_background(palette.foreground)
  };

  match palettes.fixed() {
    Some(palette) => TextTheme::new().set_base(ink(palette)),
    None => TextTheme::new().set_base(ink(&palettes.light)).set_when(
      Condition::DarkMode,
      RunStyle::new()
        .set_color(palettes.dark.foreground)
        .set_selection_color(palettes.dark.background)
        .set_selection_background(palettes.dark.foreground),
    ),
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

  /// Sets a border of `width` in `token`.
  #[must_use]
  fn theme_border(self, width: f32, token: impl Fn(&Palette) -> Color) -> Self;

  /// Sets a border of `width` in `token` while `condition` holds.
  #[must_use]
  fn theme_border_when(
    self,
    condition: Condition,
    width: f32,
    token: impl Fn(&Palette) -> Color,
  ) -> Self;
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
  fn theme_border(self, width: f32, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(
      self,
      token,
      |target, color| target.set_border(Border::new(width, color)),
      |target, condition, color| target.set_border_when(condition, Border::new(width, color)),
    )
  }

  #[inline(always)]
  fn theme_border_when(
    self,
    condition: Condition,
    width: f32,
    token: impl Fn(&Palette) -> Color,
  ) -> Self {
    Palettes::current().apply_when(self, condition, token, |target, condition, color| {
      target.set_border_when(condition, Border::new(width, color))
    })
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

  /// Sets a border of `width` in `token`.
  #[inline(always)]
  #[must_use]
  fn theme_border(self, width: f32, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(
      self,
      token,
      |target, color| target.set_border(Border::new(width, color)),
      |target, condition, color| target.set_border_when(condition, Border::new(width, color)),
    )
  }

  /// Sets a border of `width` in `token` while `condition` holds.
  #[inline(always)]
  #[must_use]
  fn theme_border_when(
    self,
    condition: Condition,
    width: f32,
    token: impl Fn(&Palette) -> Color,
  ) -> Self {
    Palettes::current().apply_when(self, condition, token, |target, condition, color| {
      target.set_border_when(condition, Border::new(width, color))
    })
  }
}

impl<T: HasDecoration> ThemedControl for T {}

/// The themed twin of a run's ink setter, on the views that carry one.
pub trait ThemedText: Sized {
  /// Sets the ink to `token`.
  #[must_use]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self;
}

impl ThemedText for Text {
  #[inline(always)]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(self, token, Self::set_color, |target, condition, color| {
      target.set_when(condition, RunStyle::new().set_color(color))
    })
  }
}

impl ThemedText for Span {
  #[inline(always)]
  fn theme_color(self, token: impl Fn(&Palette) -> Color) -> Self {
    Palettes::current().apply(self, token, Self::set_color, |target, condition, color| {
      target.set_when(condition, RunStyle::new().set_color(color))
    })
  }
}
