//! # skene-tui-core
//!
//! What every terminal driver shares above the spine: the terminal style extension
//! ([`TerminalExt`]: border glyphs, a shadow, a corner radius that picks rounded glyphs,
//! boldness and weight), the [`BorderGlyphs`] vocabulary, and the terminal
//! [`Typography`] token set. The terminal driver implements against it; a facade for
//! the terminal aliases from it.

mod ext;
mod typography;

pub use crate::{
  ext::{
    BOLD_WEIGHT,
    BorderGlyphs,
    TerminalBoxFields,
    TerminalExt,
    TerminalProperty,
    TerminalRunFields,
    TerminalValue,
    terminal_fields,
  },
  typography::Typography,
};
