//! The names of the tokens every run inherits its text properties through.
//!
//! A facade's `Typography` token set defines them, and a run references them where it
//! sets nothing of its own: a [`Text`](crate::Text) or [`Span`](crate::Span) with no
//! color is inked in [`INK`], or [`LINK_INK`] when it is a link, and selected in
//! [`SELECTION_INK`] on [`SELECTION_SURFACE`]. [`LINK_FOCUS_INK`] and
//! [`LINK_FOCUS_SURFACE`] are for a driver that paints a focused link's emphasis itself.
//! [`SIZE`], [`WEIGHT`], [`LINE_HEIGHT`] and [`LETTER_SPACING`] are for the text
//! properties a medium's style extension carries.

use skene_core::{Color, TokenRef};

/// The text color.
pub const INK: TokenRef<Color> = TokenRef::new("typography-ink");

/// The text color of a link.
pub const LINK_INK: TokenRef<Color> = TokenRef::new("typography-link-ink");

/// The text color while selected.
pub const SELECTION_INK: TokenRef<Color> = TokenRef::new("typography-selection-ink");

/// The background behind selected text.
pub const SELECTION_SURFACE: TokenRef<Color> = TokenRef::new("typography-selection-surface");

/// The text color of a link while its focus shows.
pub const LINK_FOCUS_INK: TokenRef<Color> = TokenRef::new("typography-link-focus-ink");

/// The background behind a link while its focus shows.
pub const LINK_FOCUS_SURFACE: TokenRef<Color> = TokenRef::new("typography-link-focus-surface");

/// The font size, in the medium's unit.
pub const SIZE: TokenRef<f32> = TokenRef::new("typography-size");

/// The font weight.
pub const WEIGHT: TokenRef<f32> = TokenRef::new("typography-weight");

/// The line height, as a multiple of the font size.
pub const LINE_HEIGHT: TokenRef<f32> = TokenRef::new("typography-line-height");

/// The extra space between letters, in the medium's unit.
pub const LETTER_SPACING: TokenRef<f32> = TokenRef::new("typography-letter-spacing");
