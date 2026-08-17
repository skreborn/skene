//! The Simple design language's theme: the palette a subtree paints with.

use skene_reactive::Effect;

use crate::{
  AnyView,
  ColorScheme,
  IntoView,
  NoChild,
  NodeId,
  RunStyle,
  SimplePalette,
  TextStyle,
  TextTheme,
  View,
  driver,
};

/// A theme carrying one palette, with no scheme left to choose.
#[must_use]
pub struct OnePalette;

/// A theme carrying a light and a dark palette, either of which may still be chosen.
#[must_use]
pub struct TwoPalettes {
  light: SimplePalette,
  dark: SimplePalette,
}

/// The palette a subtree paints with, threaded through the reactive `Context`.
///
/// Wrap a subtree to dress every `Simple*` view beneath it; an app dresses its own
/// chrome from the same palette by reading [`SimplePalette::current`]. A nested theme replaces
/// the palette beneath it.
///
/// [`new`](Self::new) takes one palette. [`adaptive`](Self::adaptive) takes a light and
/// a dark one and follows the platform's scheme—which costs no script, because each
/// token carries its dark value as an ordinary conditional variant and the web compiles
/// that to a `prefers-color-scheme` media query. A server-rendered page therefore
/// answers the reader's preference before any bundle arrives.
///
/// ```no_run
/// # use skene_gui::prelude::*;
/// # fn page() -> impl IntoView { Text::new("") }
/// SimpleTheme::adaptive(SimplePalette::light(), SimplePalette::dark()).set_child(page)
/// # ;
/// ```
///
/// Only the adaptive form can be told which scheme to paint, through
/// [`set_scheme`](SimpleTheme::set_scheme) and
/// [`derive_scheme`](SimpleTheme::derive_scheme): a theme carrying one palette has
/// nothing to choose, and the type says so.
#[must_use]
pub struct SimpleTheme<C = NoChild, S = OnePalette> {
  child: C,
  palette: SimplePalette,
  schemes: S,
}

impl SimpleTheme {
  /// Creates a theme painting `palette`, whatever the platform's scheme.
  #[inline(always)]
  pub fn new(palette: SimplePalette) -> Self {
    Self {
      child: NoChild,
      palette,
      schemes: OnePalette,
    }
  }
}

impl SimpleTheme<NoChild, TwoPalettes> {
  /// Creates a theme following the platform's color scheme, painting `light` under the
  /// light one and `dark` under the dark one.
  pub fn adaptive(light: SimplePalette, dark: SimplePalette) -> Self {
    Self {
      child: NoChild,
      palette: SimplePalette::adapt(light.clone(), dark.clone()),
      schemes: TwoPalettes { light, dark },
    }
  }
}

impl<C, S> SimpleTheme<C, S> {
  /// Sets the child, which the view requires before it can build.
  ///
  /// The child comes as a closure because the palette has to be in scope while it is
  /// built: [`SimplePalette::current`] answers with the enclosing theme's, and an argument is
  /// evaluated before the call it is an argument to.
  #[inline(always)]
  pub fn set_child<V: IntoView>(
    self,
    child: impl FnOnce() -> V + 'static,
  ) -> SimpleTheme<AnyView, S> {
    SimpleTheme {
      child: AnyView::new(move || child().into_view().build()),
      palette: self.palette,
      schemes: self.schemes,
    }
  }
}

impl<C> SimpleTheme<C, TwoPalettes> {
  /// Paints `scheme` rather than the platform's.
  ///
  /// Resolved where the theme is built, so the chosen palette applies unconditionally
  /// and a server render carries it—nothing is left for a media query to answer, and
  /// nothing is left to choose, which is why this leaves a theme carrying one palette.
  #[inline(always)]
  pub fn set_scheme(self, scheme: ColorScheme) -> SimpleTheme<C, OnePalette> {
    let TwoPalettes { light, dark } = self.schemes;

    SimpleTheme {
      child: self.child,
      palette: match scheme {
        ColorScheme::Light => light,
        ColorScheme::Dark => dark,
      },
      schemes: OnePalette,
    }
  }

  /// Paints the scheme `scheme` reports, following it as it changes.
  ///
  /// Bind it to the signal holding a user's setting. Every token keeps both values and
  /// the driver is told which is in force, so a change re-inks the page without
  /// rebuilding anything: the state inside the subtree, the focus, the scroll position
  /// and a half-typed input all survive it.
  ///
  /// The choice is the document's, not the subtree's—the platform's own scheme is
  /// document-wide too—so a second derived scheme deeper in the tree overrides the
  /// first rather than scoping to its own subtree. Use [`set_scheme`](Self::set_scheme)
  /// for a subtree, which resolves before anything is built.
  #[inline(always)]
  pub fn derive_scheme(self, scheme: impl Fn() -> ColorScheme + 'static) -> Self {
    Effect::new(move |_| driver::set_color_scheme(Some(scheme())));

    self
  }
}

impl<S: 'static> View for SimpleTheme<AnyView, S> {
  fn build(self) -> NodeId {
    let Self { child, palette, .. } = self;
    let text = ambient_text(&palette);

    palette.provide(move || text.set_child(child).build())
  }
}

/// The theme's ambient text: the ink every run beneath inherits and the colors a
/// selection paints in, each token's own conditions carried through as inherited
/// variants. A run's own setters still win, so a control's label keeps its own ink.
fn ambient_text(palette: &SimplePalette) -> TextTheme {
  let (ink, ink_variants) = palette.foreground.clone().parts();
  let (page, page_variants) = palette.background.clone().parts();
  let mut theme = TextTheme::new().set_base(
    TextStyle::new()
      .set_color(ink)
      .set_selection_color(page)
      .set_selection_background(ink),
  );

  for (condition, ink) in ink_variants {
    theme = theme.set_when(
      condition,
      RunStyle::new().set_color(ink).set_selection_background(ink),
    );
  }

  for (condition, page) in page_variants {
    theme = theme.set_when(condition, RunStyle::new().set_selection_color(page));
  }

  theme
}
