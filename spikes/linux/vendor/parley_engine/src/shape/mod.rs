// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping utilities.

pub(crate) mod atom;
mod cache;
mod cluster;
mod data;
pub(crate) mod shaped_text;
pub(crate) mod shaper;
// Skene: the mark-stacking pass and the per-word shape cache.
mod stack;
mod words;

pub use cluster::{Char, CharCluster, Coverage, SourceRange, Whitespace};
pub use data::{Character, CharacterFlags, ShapedCluster};

pub(crate) use data::ShapedClusterFlags;

/// Skene: the glyph `HarfRust` gives `ch` in `font` where nothing substitutes it: by the
/// character map's subtable it reads, every format it reads, and a symbol font's private range
/// for a Latin-1 character; `None` where the subtable is Mac Roman and `ch` is not ASCII.
pub(crate) fn nominal_glyph(font: &skrifa::FontRef<'_>, ch: char) -> Option<skrifa::GlyphId> {
    use skrifa::raw::TableProvider as _;

    let cmap = font.cmap().ok()?;
    let (_, record, subtable) = cmap.best_subtable()?;
    let code = u32::from(ch);

    if record.is_mac_roman() && code > 0x7f {
        return None;
    }

    subtable.map_codepoint(code).or_else(|| {
        (record.is_symbol() && code <= 0xff)
            .then(|| subtable.map_codepoint(0xf000 + code))
            .flatten()
    })
}
