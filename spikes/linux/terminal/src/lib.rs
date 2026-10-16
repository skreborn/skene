//! # skene-terminal
//!
//! The terminal realization's text, the editing core's first client:
//!
//! - [`cells::Cells`], the terminal's [`TextEngine`](skene_foundation::text::TextEngine):
//!   Parley over a typeface generated at build time whose every code point is a glyph as many
//!   cells wide as the terminal draws it, so text is laid out, broken and moved through by
//!   the same engine and rules as on a pixel platform, in cells;
//! - [`field::TextField`], a text entry the shared editing core drives (keys, the pointer,
//!   the clipboard, undo, composition), rendered to a [`frame::Frame`] of cells whose styled
//!   snapshot a test compares.

pub mod cells;
pub mod field;
pub mod frame;
