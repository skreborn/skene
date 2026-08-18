//! The Simple design language's theme: the palette a subtree paints with.

use crate::{
  AnyView,
  IntoView,
  NoChild,
  NodeId,
  RunStyle,
  SimplePalette,
  TextStyle,
  TextTheme,
  View,
};

/// The palette a subtree paints with, threaded through the reactive `Context`.
///
/// Wrap a subtree to dress every `Simple*` view beneath it; an app dresses its own
/// chrome from the same palette by reading [`SimplePalette::current`]. A nested theme
/// replaces the palette beneath it, and nesting follows the view tree rather than
/// anything a platform does with it.
///
/// A theme carries exactly one palette. Light and dark are not two themes: they are one
/// condition among the many a token answers, so a palette that follows the reader's
/// preference carries both values per property—which costs no script, because the web
/// compiles a conditional token to a `prefers-color-scheme` media query. A
/// server-rendered page therefore answers the reader before any bundle arrives.
///
/// ```no_run
/// # use skene_gui::prelude::*;
/// # fn page() -> impl IntoView { Text::new("") }
/// SimpleTheme::new(SimplePalette::default()).set_child(page)
/// # ;
/// ```
///
/// To paint one scheme regardless of the reader's preference, tell the platform which
/// is in force with [`set_scheme`](crate::set_scheme); that choice is the document's,
/// not a subtree's, exactly as the platform's own preference is.
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
