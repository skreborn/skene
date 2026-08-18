//! The Simple design language's theme: the palette a subtree paints with.

use skene_core::{AnyView, Color, Composable, IntoView, NoChild, NodeId, View};

use crate::{
  SimplePalette,
  TextTheme,
  props::{Condition, Token},
};

/// The palette a subtree paints with, threaded through the reactive `Context`.
///
/// Wrap a subtree to dress every `Simple*` view beneath it; an app dresses its own
/// chrome from the same palette by reading [`SimplePalette::current`]. A nested theme
/// replaces the palette beneath it, and nesting follows the view tree rather than
/// anything the terminal does with it.
///
/// A theme carries exactly one palette. Light and dark are not two themes: they are one
/// condition among the many a token answers, so a palette that follows the terminal's
/// detected background carries both values per property and a subtree re-inks itself
/// when that background changes.
///
/// ```no_run
/// # use skene_tui::prelude::*;
/// # fn page() -> impl IntoView { Text::new("") }
/// SimpleTheme::new(SimplePalette::default()).set_child(page)
/// # ;
/// ```
///
/// To paint one scheme regardless of the detected background, tell the terminal which is
/// in force with [`set_scheme`](crate::set_scheme); that choice is the terminal's, not a
/// subtree's, exactly as the detected background is.
#[must_use]
pub struct SimpleTheme<C = NoChild> {
  child: C,
  palette: SimplePalette,
}

impl SimpleTheme {
  /// Creates a theme painting `palette`.
  #[inline(always)]
  pub const fn new(palette: SimplePalette) -> Self {
    Self {
      child: NoChild,
      palette,
    }
  }
}

impl<C> SimpleTheme<C> {
  /// Sets the child, which the view requires before it can build.
  ///
  /// The child comes as a closure because the palette has to be in scope while it is
  /// built: [`SimplePalette::current`] answers with the enclosing theme's, and an
  /// argument is evaluated before the call it is an argument to.
  #[inline(always)]
  pub fn set_child<V: IntoView>(self, child: impl FnOnce() -> V + 'static) -> SimpleTheme<AnyView> {
    SimpleTheme {
      child: AnyView::new(move || child().into_view().build()),
      palette: self.palette,
    }
  }
}

impl View for SimpleTheme<AnyView> {
  fn build(self) -> NodeId {
    let Self { child, palette } = self;
    let text = ambient_text(&palette);

    palette.provide(move || text.set_child(child).build())
  }
}

/// One token's color under the scheme in force.
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
