//! Resolving a design token against the scheme in force, once for every facade.
//!
//! A theme holds both palettes and resolves a token one of two ways: under a fixed
//! scheme it is a single value, and while the theme follows the platform it is the
//! light value plus a dark-scheme variant—ordinary conditional styling, so the web
//! compiles it to a media query and a scriptless page still answers the user's
//! preference.

use std::rc::Rc;

use skene_core::{Color, ColorScheme};
use skene_reactive::{Owner, provide_context, use_context};

use crate::Palette;

thread_local! {
  /// The palettes an unthemed subtree paints with, built once per thread.
  static DEFAULT: Rc<Palettes> = Rc::new(Palettes::default());
}

/// A facade's condition vocabulary, as far as a theme needs it.
#[diagnostic::on_unimplemented(
  message = "`{Self}` cannot name the dark color scheme, so a theme cannot follow the platform with it",
  label = "not a scheme-aware condition"
)]
pub trait SchemeCondition: Clone {
  /// The condition holding while the dark scheme is in force.
  fn dark_mode() -> Self;

  /// This condition narrowed to the dark scheme.
  #[must_use]
  fn with_dark_mode(self) -> Self;
}

impl SchemeCondition for skene_core::Condition {
  #[inline(always)]
  fn dark_mode() -> Self {
    Self::DarkMode
  }

  #[inline(always)]
  fn with_dark_mode(self) -> Self {
    Self::and([self, Self::DarkMode])
  }
}

/// The palettes a subtree paints with, and which of them is in force.
///
/// A facade's theme view provides these through the reactive context; the themed
/// setters read them. Constructed with both palettes and no scheme, a theme follows
/// the platform.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct Palettes {
  /// The palette painted under the light scheme.
  pub light: Palette,
  /// The palette painted under the dark scheme.
  pub dark: Palette,
  /// The scheme the subtree is fixed to; `None` follows the platform's.
  pub scheme: Option<ColorScheme>,
}

impl Default for Palettes {
  #[inline(always)]
  fn default() -> Self {
    Self {
      light: Palette::light(),
      dark: Palette::dark(),
      scheme: None,
    }
  }
}

impl Palettes {
  /// The palettes the current subtree inherits: the nearest enclosing theme's, or the
  /// defaults, which follow the platform's scheme.
  #[inline(always)]
  #[must_use]
  pub fn current() -> Rc<Self> {
    use_context::<Rc<Self>>().unwrap_or_else(|| DEFAULT.with(Rc::clone))
  }

  /// Builds `child` with these palettes inherited by its subtree.
  pub fn provide<R>(self, child: impl FnOnce() -> R) -> R {
    let scope = Owner::current().child();

    scope.enter(|| {
      provide_context(Rc::new(self));

      child()
    })
  }

  /// The palette in force, while the scheme is fixed.
  #[inline(always)]
  #[must_use]
  pub fn fixed(&self) -> Option<&Palette> {
    match self.scheme? {
      ColorScheme::Light => Some(&self.light),
      ColorScheme::Dark => Some(&self.dark),
    }
  }

  /// Sets `token` on `target` through the property's own setters: one value under a
  /// fixed scheme, the light value plus a dark-scheme variant while following the
  /// platform.
  pub fn apply<T, C: SchemeCondition>(
    &self,
    target: T,
    token: impl Fn(&Palette) -> Color,
    set: impl FnOnce(T, Color) -> T,
    set_when: impl FnOnce(T, C, Color) -> T,
  ) -> T {
    match self.fixed() {
      Some(palette) => set(target, token(palette)),
      None => set_when(
        set(target, token(&self.light)),
        C::dark_mode(),
        token(&self.dark),
      ),
    }
  }

  /// Sets `token` on `target` for as long as `condition` holds, narrowing to the
  /// scheme the same way [`apply`](Self::apply) does.
  pub fn apply_when<T, C: SchemeCondition>(
    &self,
    target: T,
    condition: C,
    token: impl Fn(&Palette) -> Color,
    set_when: impl Fn(T, C, Color) -> T,
  ) -> T {
    if let Some(palette) = self.fixed() {
      return set_when(target, condition, token(palette));
    }

    let target = set_when(target, condition.clone(), token(&self.light));

    set_when(target, condition.with_dark_mode(), token(&self.dark))
  }
}
