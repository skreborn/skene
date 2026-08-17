//! The Simple design language's theme: the palette a subtree paints with.

use skene_core::{AnyView, Color, ColorScheme, Composable, IntoView, NoChild, NodeId, View};
use skene_reactive::Effect;

use crate::{
  SimplePalette,
  TextTheme,
  props::{Condition, Token},
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
/// a dark one and follows the background the terminal detected, because each token
/// carries its dark value as an ordinary conditional variant—so a subtree re-inks itself
/// when the terminal's own scheme changes.
///
/// ```no_run
/// # use skene_tui::prelude::*;
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
  /// Creates a theme painting `palette`, whatever the terminal's background.
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
  /// Creates a theme following the terminal's detected background, painting `light` on a
  /// light one and `dark` on a dark one.
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
  /// Paints `scheme` rather than the terminal's.
  ///
  /// Resolved where the theme is built, so the chosen palette applies unconditionally—
  /// nothing is left for a condition to answer, and nothing is left to choose, which is
  /// why this leaves a theme carrying one palette.
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
  /// the paint is told which is in force, so a change re-inks the next frame without
  /// rebuilding anything: the state inside the subtree, the focus, the scroll position
  /// and a half-typed input all survive it.
  ///
  /// The choice is the terminal's, not the subtree's—the detected background is too—so
  /// a second derived scheme deeper in the tree overrides the first rather than scoping
  /// to its own subtree. Use [`set_scheme`](Self::set_scheme) for a subtree, which
  /// resolves before anything is built.
  #[inline(always)]
  pub fn derive_scheme(self, scheme: impl Fn() -> ColorScheme + 'static) -> Self {
    Effect::new(move |_| {
      crate::with_state(|state| {
        state.scheme.set(Some(scheme()));
        state.dirty.set(true);
      });
    });

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

/// One token's color under the background the terminal detected.
///
/// For the properties a conditional variant cannot reach—a popup's whole style, a
/// highlight's color pair, a span's ink—which resolve eagerly instead, and which a
/// component library faces exactly as the built-ins do. Only the scheme leaves are
/// answered: a size or an interaction state is not something a static resolve can know,
/// and a property that can carry a variant should carry one rather than come through
/// here.
///
/// A free function rather than a method because [`Token`] is the spine's type, not the
/// terminal's.
pub fn painted(token: &Token) -> Color {
  let dark = crate::with_state(crate::state::State::dark);
  let holds = |leaf: &Condition| match leaf {
    Condition::DarkMode => dark,
    Condition::LightMode => !dark,
    _ => false,
  };
  let mut color = *token.base();

  for (condition, value) in token.variants() {
    if condition.holds(&holds) {
      color = *value;
    }
  }

  color
}

/// The theme's ambient text: the colors a selection paints in. The ink itself is the
/// terminal's own foreground, which already matches its background, so a theme sets
/// none.
fn ambient_text(palette: &SimplePalette) -> TextTheme {
  TextTheme::new()
    .set_selection_color(painted(&palette.background))
    .set_selection_background(painted(&palette.foreground))
}
