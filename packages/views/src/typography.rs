//! The names of the tokens every run inherits its text properties through.
//!
//! A facade's `Typography` token set defines them, and a run references them where it
//! sets nothing of its own: a [`Text`](crate::Text) or [`Span`](crate::Span) with no
//! color is inked in [`INK`], or [`LINK_INK`] when it is a link, and selected in
//! [`SELECTION_INK`] on [`SELECTION_SURFACE`]. [`SIZE`], [`WEIGHT`], [`LINE_HEIGHT`] and
//! [`LETTER_SPACING`] are for the text properties a medium's style extension carries.

use skene_core::{Color, next::TokenRef};

/// The text color.
pub const INK: TokenRef<Color> = TokenRef::new("typography-ink");

/// The text color of a link.
pub const LINK_INK: TokenRef<Color> = TokenRef::new("typography-link-ink");

/// The text color while selected.
pub const SELECTION_INK: TokenRef<Color> = TokenRef::new("typography-selection-ink");

/// The background behind selected text.
pub const SELECTION_SURFACE: TokenRef<Color> = TokenRef::new("typography-selection-surface");

/// The font size, in the medium's unit.
pub const SIZE: TokenRef<f32> = TokenRef::new("typography-size");

/// The font weight.
pub const WEIGHT: TokenRef<f32> = TokenRef::new("typography-weight");

/// The line height, as a multiple of the font size.
pub const LINE_HEIGHT: TokenRef<f32> = TokenRef::new("typography-line-height");

/// The extra space between letters, in the medium's unit.
pub const LETTER_SPACING: TokenRef<f32> = TokenRef::new("typography-letter-spacing");
