//! The platform's colors, as CSS's system colors are: what the built-in controls' defaults
//! name, defined at the root by the realization from its environment.

use crate::{Color, Definitions, Token, TokenRef, TokenSet};

/// The platform's colors, as CSS's system colors are: what the built-ins' defaults name,
/// defined at the root by the realization from its environment
/// ([`crate::seam::Realization::system_colors`]) and redefined by any theme beneath.
///
/// A palette that redefines them restyles every built-in control beneath its theme at once:
/// `Token::new(SystemColors::ACCENT, Color::rgb(0xff9500))`.
#[derive(Clone, PartialEq, Debug)]
pub struct SystemColors {
  /// [`SystemColors::INK`].
  pub ink: Token<Color>,
  /// [`SystemColors::MUTED_INK`].
  pub muted_ink: Token<Color>,
  /// [`SystemColors::WINDOW`].
  pub window: Token<Color>,
  /// [`SystemColors::CONTROL`].
  pub control: Token<Color>,
  /// [`SystemColors::CONTROL_HOVER`].
  pub control_hover: Token<Color>,
  /// [`SystemColors::CONTROL_PRESSED`].
  pub control_pressed: Token<Color>,
  /// [`SystemColors::BORDER`].
  pub border: Token<Color>,
  /// [`SystemColors::ACCENT`].
  pub accent: Token<Color>,
  /// [`SystemColors::ACCENT_HOVER`].
  pub accent_hover: Token<Color>,
  /// [`SystemColors::ACCENT_PRESSED`].
  pub accent_pressed: Token<Color>,
  /// [`SystemColors::ON_ACCENT`].
  pub on_accent: Token<Color>,
  /// [`SystemColors::TRACK`].
  pub track: Token<Color>,
  /// [`SystemColors::KNOB`].
  pub knob: Token<Color>,
  /// [`SystemColors::FIELD`].
  pub field: Token<Color>,
  /// [`SystemColors::SELECTION`].
  pub selection: Token<Color>,
  /// [`SystemColors::SELECTION_INACTIVE`].
  pub selection_inactive: Token<Color>,
}

impl SystemColors {
  /// Text on the window and on a control.
  pub const INK: TokenRef<Color> = TokenRef::new("system-ink");
  /// Text that says less: a placeholder.
  pub const MUTED_INK: TokenRef<Color> = TokenRef::new("system-muted-ink");
  /// The window behind every view.
  pub const WINDOW: TokenRef<Color> = TokenRef::new("system-window");
  /// A control's fill.
  pub const CONTROL: TokenRef<Color> = TokenRef::new("system-control");
  /// A control's fill under the pointer.
  pub const CONTROL_HOVER: TokenRef<Color> = TokenRef::new("system-control-hover");
  /// A control's fill while pressed.
  pub const CONTROL_PRESSED: TokenRef<Color> = TokenRef::new("system-control-pressed");
  /// A control's border.
  pub const BORDER: TokenRef<Color> = TokenRef::new("system-border");
  /// The accent: a prominent button, an on switch, a checked mark's face, a slider's fill, a
  /// focus ring.
  pub const ACCENT: TokenRef<Color> = TokenRef::new("system-accent");
  /// The accent under the pointer.
  pub const ACCENT_HOVER: TokenRef<Color> = TokenRef::new("system-accent-hover");
  /// The accent while pressed.
  pub const ACCENT_PRESSED: TokenRef<Color> = TokenRef::new("system-accent-pressed");
  /// What sits on the accent.
  pub const ON_ACCENT: TokenRef<Color> = TokenRef::new("system-on-accent");
  /// A switch's track while off, a slider's track.
  pub const TRACK: TokenRef<Color> = TokenRef::new("system-track");
  /// A switch's and a slider's thumb.
  pub const KNOB: TokenRef<Color> = TokenRef::new("system-knob");
  /// A text field's fill, a checkbox's face, an open list.
  pub const FIELD: TokenRef<Color> = TokenRef::new("system-field");
  /// A text entry's selection while it has focus.
  pub const SELECTION: TokenRef<Color> = TokenRef::new("system-selection");
  /// A text entry's selection while it has none.
  pub const SELECTION_INACTIVE: TokenRef<Color> = TokenRef::new("system-selection-inactive");
}

impl TokenSet for SystemColors {
  fn define(&self, definitions: &mut Definitions) {
    definitions
      .add(&self.ink)
      .add(&self.muted_ink)
      .add(&self.window)
      .add(&self.control)
      .add(&self.control_hover)
      .add(&self.control_pressed)
      .add(&self.border)
      .add(&self.accent)
      .add(&self.accent_hover)
      .add(&self.accent_pressed)
      .add(&self.on_accent)
      .add(&self.track)
      .add(&self.knob)
      .add(&self.field)
      .add(&self.selection)
      .add(&self.selection_inactive);
  }
}
