//! The Simple design language's color data: the token set, the default palette, and the
//! emphasis variants resolved against it.

use std::rc::Rc;

use skene_core::{Color, Composable, Condition, Token};
use skene_reactive::{Owner, provide_context, use_context};

/// One emphasis variant's colors: the surface it paints, the ink on that surface, and
/// the surfaces its interaction states paint instead.
///
/// A `None` border says the variant is filled rather than bordered, which the terminal
/// reads structurally: a border there is a cell rather than a hairline, so an invisible
/// one would still cost the room. Assign the field to drop a border a setter added.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct SimpleLook<C = Condition> {
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

impl<C: Clone> SimpleLook<C> {
  /// Creates a look painting `ink` on `surface`, with no border and neither a hover nor
  /// a focus surface of its own.
  pub fn new(surface: impl Into<Token<C>>, ink: impl Into<Token<C>>) -> Self {
    let surface = surface.into();

    Self {
      hover: surface.clone(),
      focus: surface.clone(),
      surface,
      ink: ink.into(),
      border: None,
    }
  }
}

impl<C> SimpleLook<C> {
  /// Sets the action's surface.
  #[inline(always)]
  pub fn set_surface(mut self, surface: impl Into<Token<C>>) -> Self {
    self.surface = surface.into();

    self
  }

  /// Overrides the action's surface while `condition` holds.
  #[inline(always)]
  pub fn set_surface_when(mut self, condition: C, surface: Color) -> Self {
    self.surface = self.surface.when(condition, surface);

    self
  }

  /// Sets the ink on the action's surface.
  #[inline(always)]
  pub fn set_ink(mut self, ink: impl Into<Token<C>>) -> Self {
    self.ink = ink.into();

    self
  }

  /// Overrides the ink on the action's surface while `condition` holds.
  #[inline(always)]
  pub fn set_ink_when(mut self, condition: C, ink: Color) -> Self {
    self.ink = self.ink.when(condition, ink);

    self
  }

  /// Gives the action a hairline border, making it a bordered variant.
  #[inline(always)]
  pub fn set_border(mut self, border: impl Into<Token<C>>) -> Self {
    self.border = Some(border.into());

    self
  }

  /// Overrides the hairline border while `condition` holds, making it a bordered
  /// variant—transparent outside `condition`—if it was not one already.
  #[inline(always)]
  pub fn set_border_when(mut self, condition: C, border: Color) -> Self {
    let base = self
      .border
      .unwrap_or_else(|| Token::from(Color::TRANSPARENT));

    self.border = Some(base.when(condition, border));

    self
  }

  /// Sets the surface shown under the pointer.
  #[inline(always)]
  pub fn set_hover(mut self, hover: impl Into<Token<C>>) -> Self {
    self.hover = hover.into();

    self
  }

  /// Overrides the surface shown under the pointer while `condition` holds.
  #[inline(always)]
  pub fn set_hover_when(mut self, condition: C, hover: Color) -> Self {
    self.hover = self.hover.when(condition, hover);

    self
  }

  /// Sets the surface shown while focused.
  #[inline(always)]
  pub fn set_focus(mut self, focus: impl Into<Token<C>>) -> Self {
    self.focus = focus.into();

    self
  }

  /// Overrides the surface shown while focused while `condition` holds.
  #[inline(always)]
  pub fn set_focus_when(mut self, condition: C, focus: Color) -> Self {
    self.focus = self.focus.when(condition, focus);

    self
  }
}

/// The Simple components' design tokens: the chrome colors and one [`SimpleLook`] per
/// emphasis variant.
///
/// A palette is a **value**: build one, hand it around, hand it to a `SimpleTheme`.
/// Every field is a [`Token`], so every color is set two ways—`set_card` for the value
/// it takes by default, `set_card_when` for the value it takes while a condition holds.
/// The color scheme is one such condition and nothing more: a palette that answers the
/// reader's preference spells `set_card_when(Condition::DarkMode, ink)` beside
/// `set_card_when(Condition::MinWidth(1200.), wide)`, and neither is privileged. A
/// look's own conditions live on the look, since a look is five colors rather than one.
///
/// Use the tokens for the chrome around the components too, so an app reads as one
/// surface.
///
/// This type is the Simple library's, not the framework's. A third-party component
/// library defines its own struct of [`Token`]s and is on exactly the same footing:
/// nothing below this crate knows what a palette is, and [`default`](Self::default) is
/// built from the same public setters a third party would use.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct SimplePalette<C = Condition> {
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
  pub primary: SimpleLook<C>,
  /// A supporting action: a subdued surface.
  pub secondary: SimpleLook<C>,
  /// A hairline-bordered action on the ambient background.
  pub outline: SimpleLook<C>,
  /// A bare action, surfacing only under the pointer.
  pub ghost: SimpleLook<C>,
  /// A dangerous action.
  pub destructive: SimpleLook<C>,
}

impl<C> SimplePalette<C> {
  /// Sets the page background.
  #[inline(always)]
  pub fn set_background(mut self, background: impl Into<Token<C>>) -> Self {
    self.background = background.into();

    self
  }

  /// Overrides the page background while `condition` holds.
  #[inline(always)]
  pub fn set_background_when(mut self, condition: C, background: Color) -> Self {
    self.background = self.background.when(condition, background);

    self
  }

  /// Sets the default text ink.
  #[inline(always)]
  pub fn set_foreground(mut self, foreground: impl Into<Token<C>>) -> Self {
    self.foreground = foreground.into();

    self
  }

  /// Overrides the default text ink while `condition` holds.
  #[inline(always)]
  pub fn set_foreground_when(mut self, condition: C, foreground: Color) -> Self {
    self.foreground = self.foreground.when(condition, foreground);

    self
  }

  /// Sets the de-emphasized text ink.
  #[inline(always)]
  pub fn set_muted_foreground(mut self, muted_foreground: impl Into<Token<C>>) -> Self {
    self.muted_foreground = muted_foreground.into();

    self
  }

  /// Overrides the de-emphasized text ink while `condition` holds.
  #[inline(always)]
  pub fn set_muted_foreground_when(mut self, condition: C, muted_foreground: Color) -> Self {
    self.muted_foreground = self.muted_foreground.when(condition, muted_foreground);

    self
  }

  /// Sets the raised surface.
  #[inline(always)]
  pub fn set_card(mut self, card: impl Into<Token<C>>) -> Self {
    self.card = card.into();

    self
  }

  /// Overrides the raised surface while `condition` holds.
  #[inline(always)]
  pub fn set_card_when(mut self, condition: C, card: Color) -> Self {
    self.card = self.card.when(condition, card);

    self
  }

  /// Sets the hairline border and separator ink.
  #[inline(always)]
  pub fn set_border(mut self, border: impl Into<Token<C>>) -> Self {
    self.border = border.into();

    self
  }

  /// Overrides the hairline border and separator ink while `condition` holds.
  #[inline(always)]
  pub fn set_border_when(mut self, condition: C, border: Color) -> Self {
    self.border = self.border.when(condition, border);

    self
  }

  /// Sets the focused or hovered control's border ink.
  #[inline(always)]
  pub fn set_ring(mut self, ring: impl Into<Token<C>>) -> Self {
    self.ring = ring.into();

    self
  }

  /// Overrides the focused or hovered control's border ink while `condition` holds.
  #[inline(always)]
  pub fn set_ring_when(mut self, condition: C, ring: Color) -> Self {
    self.ring = self.ring.when(condition, ring);

    self
  }

  /// Sets the main action's colors.
  #[inline(always)]
  pub fn set_primary(mut self, primary: SimpleLook<C>) -> Self {
    self.primary = primary;

    self
  }

  /// Sets the supporting action's colors.
  #[inline(always)]
  pub fn set_secondary(mut self, secondary: SimpleLook<C>) -> Self {
    self.secondary = secondary;

    self
  }

  /// Sets the hairline-bordered action's colors.
  #[inline(always)]
  pub fn set_outline(mut self, outline: SimpleLook<C>) -> Self {
    self.outline = outline;

    self
  }

  /// Sets the bare action's colors.
  #[inline(always)]
  pub fn set_ghost(mut self, ghost: SimpleLook<C>) -> Self {
    self.ghost = ghost;

    self
  }

  /// Sets the dangerous action's colors.
  #[inline(always)]
  pub fn set_destructive(mut self, destructive: SimpleLook<C>) -> Self {
    self.destructive = destructive;

    self
  }
}

impl<C: Composable> Default for SimplePalette<C> {
  /// The palette the Simple components paint with unless an app hands over its own:
  /// dark ink on a near-white page, each token carrying the value it takes under the
  /// reader's dark preference.
  fn default() -> Self {
    let page = Color::srgb(0.98, 0.98, 0.98);
    let page_dark = Color::srgb(0.035, 0.035, 0.043);
    let ink = Color::srgb(0.035, 0.035, 0.043);
    let ink_dark = Color::srgb(0.98, 0.98, 0.98);
    let muted = Color::srgb(0.443, 0.443, 0.478);
    let muted_dark = Color::srgb(0.631, 0.631, 0.667);
    let raised = Color::srgb(1., 1., 1.);
    let raised_dark = Color::srgb(0.094, 0.094, 0.106);
    let edge = Color::srgb(0.894, 0.894, 0.906);
    let edge_dark = Color::srgb(0.153, 0.153, 0.165);
    let subdued = Color::srgb(0.957, 0.957, 0.961);
    let subdued_dark = Color::srgb(0.247, 0.247, 0.275);

    Self {
      background: pair(page, page_dark),
      foreground: pair(ink, ink_dark),
      muted_foreground: pair(muted, muted_dark),
      card: pair(raised, raised_dark),
      border: pair(edge, edge_dark),
      ring: pair(muted, muted_dark),
      primary: SimpleLook::new(pair(raised_dark, ink_dark), pair(ink_dark, raised_dark))
        .set_hover(pair(edge_dark, edge))
        .set_focus(pair(subdued_dark, Color::srgb(0.831, 0.831, 0.855))),
      secondary: SimpleLook::new(pair(subdued, edge_dark), pair(ink, ink_dark))
        .set_hover(pair(edge, subdued_dark))
        .set_focus(pair(
          Color::srgb(0.831, 0.831, 0.847),
          Color::srgb(0.322, 0.322, 0.357),
        )),
      outline: SimpleLook::new(Color::TRANSPARENT, pair(ink, ink_dark))
        .set_border(pair(edge, edge_dark))
        .set_hover(pair(subdued, edge_dark))
        .set_focus(pair(edge, subdued_dark)),
      ghost: SimpleLook::new(Color::TRANSPARENT, pair(ink, ink_dark))
        .set_hover(pair(subdued, edge_dark))
        .set_focus(pair(edge, subdued_dark)),
      destructive: SimpleLook::new(
        Color::srgb(0.863, 0.149, 0.149),
        Color::srgb(0.98, 0.98, 0.98),
      )
      .set_hover(Color::srgb(0.725, 0.11, 0.11))
      .set_focus(Color::srgb(0.6, 0.106, 0.106)),
    }
  }
}

/// One token taking `dark`'s value under the reader's dark preference: how the default
/// palette spells a color that answers the scheme, using the same
/// [`when`](Token::when) any palette does.
fn pair<C: Composable>(light: Color, dark: Color) -> Token<C> {
  Token::from(light).when(C::dark_mode(), dark)
}

impl<C: Composable + 'static> SimplePalette<C> {
  /// The palette the current subtree inherits: the nearest enclosing theme's, or the
  /// [default](Self::default) one.
  ///
  /// Read it where the views that use it are built—inside a `build`, or inside the
  /// closure a theme takes its child as—since that is where the enclosing theme's
  /// palette is in scope.
  pub fn current() -> Self {
    use_context::<Rc<Self>>().map_or_else(Self::default, |palette| (*palette).clone())
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
  pub fn look<C>(self, palette: SimplePalette<C>) -> SimpleLook<C> {
    match self {
      Self::Primary => palette.primary,
      Self::Secondary => palette.secondary,
      Self::Outline => palette.outline,
      Self::Ghost => palette.ghost,
      Self::Destructive => palette.destructive,
    }
  }
}
