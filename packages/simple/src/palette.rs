//! The Simple design language's color data: the token set, its light and dark
//! instances, and the emphasis variants resolved against it.

use std::rc::Rc;

use skene_core::{Adaptive, Color, Composable, Condition};
use skene_reactive::{Owner, provide_context, use_context};

/// One design token: a color, and the conditions that override it.
///
/// Generic over the condition vocabulary so a terminal palette is expressed in the
/// terminal's conditions and a GUI palette in the GUI's; each facade aliases the
/// parameter away.
pub type Token<C = Condition> = Adaptive<Color, C>;

/// One emphasis variant's colors: the surface it paints, the ink on that surface, and
/// the surfaces its interaction states paint instead.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Look<C = Condition> {
  /// The action's surface.
  pub surface: Token<C>,
  /// The ink on that surface.
  pub ink: Token<C>,
  /// The hairline border, on the variants that carry one.
  pub border: Option<Token<C>>,
  /// The surface shown under the pointer.
  pub hover: Token<C>,
  /// The surface shown while focused, on platforms without a focus ring of their own.
  pub focus: Token<C>,
}

/// The Simple components' design tokens: the chrome colors and one [`Look`] per
/// emphasis variant.
///
/// A palette is a **value**: build one, hand it around, hand it to a
/// `SimpleTheme`. Every field is a [`Token`], so a token can carry conditions of its
/// own—`palette.card.when(Condition::MinWidth(1200.), ink)` is as ordinary as the
/// light and dark pair a theme builds. Use the tokens for the chrome around the
/// components too, so an app reads as one surface.
///
/// This type is the Simple library's, not the framework's. A third-party component
/// library defines its own struct of [`Token`]s and is on exactly the same footing:
/// nothing below this crate knows what a palette is.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Palette<C = Condition> {
  /// The page background.
  pub background: Token<C>,
  /// The default text ink.
  pub foreground: Token<C>,
  /// De-emphasized text: captions, hints, section titles.
  pub muted_foreground: Token<C>,
  /// A raised surface: cards, popovers, modals.
  pub card: Token<C>,
  /// Hairline borders and separators.
  pub border: Token<C>,
  /// The focused or hovered control's border ink.
  pub ring: Token<C>,
  /// The main action: a high-contrast surface.
  pub primary: Look<C>,
  /// A supporting action: a subdued surface.
  pub secondary: Look<C>,
  /// A hairline-bordered action on the ambient background.
  pub outline: Look<C>,
  /// A bare action, surfacing only under the pointer.
  pub ghost: Look<C>,
  /// A dangerous action.
  pub destructive: Look<C>,
}

impl<C> Palette<C> {
  /// The light palette: dark ink on a near-white page.
  pub fn light() -> Self {
    let background = Color::srgb(0.98, 0.98, 0.98);
    let foreground = Color::srgb(0.035, 0.035, 0.043);
    let border = Color::srgb(0.894, 0.894, 0.906);
    let subdued = Color::srgb(0.957, 0.957, 0.961);
    let muted_foreground = Color::srgb(0.443, 0.443, 0.478);

    Self {
      background: background.into(),
      foreground: foreground.into(),
      muted_foreground: muted_foreground.into(),
      card: Color::srgb(1., 1., 1.).into(),
      border: border.into(),
      ring: muted_foreground.into(),
      primary: Look {
        surface: Color::srgb(0.094, 0.094, 0.106).into(),
        ink: Color::srgb(0.98, 0.98, 0.98).into(),
        border: None,
        hover: Color::srgb(0.153, 0.153, 0.165).into(),
        focus: Color::srgb(0.247, 0.247, 0.275).into(),
      },
      secondary: Look {
        surface: subdued.into(),
        ink: foreground.into(),
        border: None,
        hover: border.into(),
        focus: Color::srgb(0.831, 0.831, 0.847).into(),
      },
      outline: Look {
        surface: Color::TRANSPARENT.into(),
        ink: foreground.into(),
        border: Some(border.into()),
        hover: subdued.into(),
        focus: border.into(),
      },
      ghost: Look {
        surface: Color::TRANSPARENT.into(),
        ink: foreground.into(),
        border: None,
        hover: subdued.into(),
        focus: border.into(),
      },
      destructive: destructive(),
    }
  }

  /// The dark palette: light ink on a near-black page.
  pub fn dark() -> Self {
    let foreground = Color::srgb(0.98, 0.98, 0.98);
    let border = Color::srgb(0.153, 0.153, 0.165);
    let raised = Color::srgb(0.247, 0.247, 0.275);
    let muted_foreground = Color::srgb(0.631, 0.631, 0.667);

    Self {
      background: Color::srgb(0.035, 0.035, 0.043).into(),
      foreground: foreground.into(),
      muted_foreground: muted_foreground.into(),
      card: Color::srgb(0.094, 0.094, 0.106).into(),
      border: border.into(),
      ring: muted_foreground.into(),
      primary: Look {
        surface: foreground.into(),
        ink: Color::srgb(0.094, 0.094, 0.106).into(),
        border: None,
        hover: Color::srgb(0.894, 0.894, 0.906).into(),
        focus: Color::srgb(0.831, 0.831, 0.855).into(),
      },
      secondary: Look {
        surface: border.into(),
        ink: foreground.into(),
        border: None,
        hover: raised.into(),
        focus: Color::srgb(0.322, 0.322, 0.357).into(),
      },
      outline: Look {
        surface: Color::TRANSPARENT.into(),
        ink: foreground.into(),
        border: Some(border.into()),
        hover: border.into(),
        focus: raised.into(),
      },
      ghost: Look {
        surface: Color::TRANSPARENT.into(),
        ink: foreground.into(),
        border: None,
        hover: border.into(),
        focus: raised.into(),
      },
      destructive: destructive(),
    }
  }
}

impl<C: Composable> Palette<C> {
  /// One palette whose every token carries `dark`'s value as a scheme variant: the
  /// plumbing a facade's adaptive theme runs on, rather than a way to express a design.
  ///
  /// Lossless in both directions—a token that already carries conditions of its own
  /// keeps them, and `dark`'s are narrowed to the dark scheme rather than dropped.
  pub fn adapt(light: Self, dark: Self) -> Self {
    Self {
      background: adapt_token(light.background, dark.background),
      foreground: adapt_token(light.foreground, dark.foreground),
      muted_foreground: adapt_token(light.muted_foreground, dark.muted_foreground),
      card: adapt_token(light.card, dark.card),
      border: adapt_token(light.border, dark.border),
      ring: adapt_token(light.ring, dark.ring),
      primary: adapt_look(light.primary, dark.primary),
      secondary: adapt_look(light.secondary, dark.secondary),
      outline: adapt_look(light.outline, dark.outline),
      ghost: adapt_look(light.ghost, dark.ghost),
      destructive: adapt_look(light.destructive, dark.destructive),
    }
  }
}

/// One token carrying `dark`'s values under the dark scheme.
fn adapt_token<C: Composable>(light: Token<C>, dark: Token<C>) -> Token<C> {
  dark
    .narrowed(C::dark_mode())
    .into_iter()
    .fold(light, |token, (condition, color)| {
      token.when(condition, color)
    })
}

/// One emphasis variant's colors, adapted token by token.
fn adapt_look<C: Composable>(light: Look<C>, dark: Look<C>) -> Look<C> {
  Look {
    surface: adapt_token(light.surface, dark.surface),
    ink: adapt_token(light.ink, dark.ink),
    border: light
      .border
      .zip(dark.border)
      .map(|(light, dark)| adapt_token(light, dark)),
    hover: adapt_token(light.hover, dark.hover),
    focus: adapt_token(light.focus, dark.focus),
  }
}

impl<C: Composable + 'static> Palette<C> {
  /// The palette the current subtree inherits: the nearest enclosing theme's, or the
  /// dark one.
  ///
  /// Read it where the views that use it are built—inside a `build`, or inside the
  /// closure a theme takes its child as—since that is where the enclosing theme's
  /// palette is in scope.
  pub fn current() -> Self {
    use_context::<Rc<Self>>().map_or_else(Self::dark, |palette| (*palette).clone())
  }

  /// Builds `child` with this palette inherited by its subtree.
  pub fn provide<R>(self, child: impl FnOnce() -> R) -> R {
    let scope = Owner::current().child();

    scope.enter(|| {
      provide_context(Rc::new(self));

      child()
    })
  }
}

/// The destructive action's colors, which carry their own contrast and so read the
/// same in both schemes.
fn destructive<C>() -> Look<C> {
  Look {
    surface: Color::srgb(0.863, 0.149, 0.149).into(),
    ink: Color::srgb(0.98, 0.98, 0.98).into(),
    border: None,
    hover: Color::srgb(0.725, 0.11, 0.11).into(),
    focus: Color::srgb(0.6, 0.106, 0.106).into(),
  }
}

/// A Simple action's emphasis, selecting its surface and ink.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[must_use]
pub enum SimpleVariant {
  /// The main action: a high-contrast surface.
  #[default]
  Primary,
  /// A supporting action: a subdued surface.
  Secondary,
  /// A hairline-bordered action on the ambient background.
  Outline,
  /// A bare action, surfacing only under the pointer.
  Ghost,
  /// A dangerous action: the destructive surface.
  Destructive,
}

impl SimpleVariant {
  /// The variant's colors, taken from `palette`.
  #[inline(always)]
  pub fn look<C>(self, palette: Palette<C>) -> Look<C> {
    match self {
      Self::Primary => palette.primary,
      Self::Secondary => palette.secondary,
      Self::Outline => palette.outline,
      Self::Ghost => palette.ghost,
      Self::Destructive => palette.destructive,
    }
  }
}
