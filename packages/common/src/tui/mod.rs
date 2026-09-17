//! What every terminal platform shares above the spine: the terminal style extension
//! ([`TuiExt`]: border glyphs, a shadow, a corner radius that picks rounded glyphs,
//! boldness, weight and a size in cells), the [`BorderGlyphs`] vocabulary, and the terminal
//! [`Typography`] token set. The terminal platform implements against it; a facade for
//! the terminal aliases from it.

mod ext;
mod typography;

pub use self::{
  ext::{
    BOLD_WEIGHT,
    BorderGlyphs,
    TuiBoxFields,
    TuiExt,
    TuiProperty,
    TuiRunFields,
    TuiValue,
    tui_fields,
  },
  typography::Typography,
};
