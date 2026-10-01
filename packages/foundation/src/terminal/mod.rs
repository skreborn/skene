//! What every terminal platform shares above the spine.
//!
//! The terminal style extension ([`TerminalStyle`]: border glyphs, a shadow, scrollbars, a
//! corner radius that picks rounded glyphs, boldness, weight and a size in cells), the
//! [`BorderGlyphs`] and [`ScrollbarGlyphs`] vocabulary, and the terminal [`Typography`]
//! token set. The terminal
//! platform implements against it; a facade for the terminal aliases from it.

mod ext;
mod typography;

pub use self::{
  ext::{
    BOLD_WEIGHT,
    BorderGlyphs,
    ScrollbarGlyphs,
    TerminalBoxFields,
    TerminalProperty,
    TerminalRunFields,
    TerminalStyle,
    TerminalValue,
    terminal_fields,
  },
  typography::Typography,
};
