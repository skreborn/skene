// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Skene: combining marks the font's mark positioning leaves colliding, stacked clear of one
//! another.
//!
//! `HarfRust` positions marks by the font's `mark` and `mkmk` features, and falls back to stacking
//! them by their combining classes only where the font has no mark positioning at all. A font that
//! positions some marks but not a sequence of them (two marks above a base, each anchored to the
//! base and not to each other) leaves them drawn over one another. This pass finds such marks by
//! their ink and moves the later one away from the base until it clears, as the fallback stacks
//! them, and leaves every mark the font keeps apart where it put it.

use alloc::vec::Vec;

use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::raw::TableProvider as _;
use skrifa::raw::tables::layout::ClassDef;
use skrifa::{FontRef, GlyphId, GlyphId16, MetadataProvider as _};

use super::ShapedCluster;
use crate::Glyph;

/// The class `GDEF` gives mark glyphs.
const MARK_CLASS: u16 = 3;

/// A glyph's ink in layout units, `y` down, relative to its grapheme's visual start.
#[derive(Clone, Copy, Debug)]
struct Ink {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Ink {
    /// Whether `self` and `other` overlap enough to read as a collision: by a quarter of the
    /// narrower's width across and a tenth of the shorter's height down, so marks the font sets
    /// touching are left alone.
    fn collides(&self, other: &Self) -> bool {
        let across = self.right.min(other.right) - self.left.max(other.left);
        let down = self.bottom.min(other.bottom) - self.top.max(other.top);
        let width = (self.right - self.left).min(other.right - other.left);
        let height = (self.bottom - self.top).min(other.bottom - other.top);

        across > 0.25 * width && down > 0.1 * height
    }

    /// `self` moved down by `dy`.
    fn moved(self, dy: f32) -> Self {
        Self {
            top: self.top + dy,
            bottom: self.bottom + dy,
            ..self
        }
    }
}

/// A mark placed so far in its grapheme: its ink and whether it sits above its base.
struct Placed {
    ink: Ink,
    above: bool,
}

/// Stacks the marks of each grapheme of a run (`clusters`, in logical order, with the `glyphs`
/// they index) set in `font` at `font_size` and the location `coords`, that the font leaves
/// colliding: a mark whose ink collides with an earlier mark's on the same side of the base moves
/// away from the base until it clears every earlier one there by a sixteenth of the font size,
/// the gap `HarfRust`'s fallback stacks marks with.
///
/// A mark is a glyph `GDEF` classes as one, or a glyph after the grapheme's first with no
/// advance.
pub(crate) fn stack_marks(
    font: &FontRef<'_>,
    coords: &[NormalizedCoord],
    font_size: f32,
    is_rtl: bool,
    clusters: &mut [ShapedCluster],
    glyphs: &mut Vec<Glyph>,
) {
    // A grapheme with marks to collide has three glyphs or more, over several clusters or in
    // one; a run of one glyph a grapheme pays nothing more than this scan.
    if clusters
        .iter()
        .all(|cluster| cluster.is_grapheme_start() && cluster.glyph_len() <= 1)
    {
        return;
    }

    let mut start = 0;
    let mut classes: Option<Option<ClassDef<'_>>> = None;
    let mut metrics: Option<GlyphMetrics<'_>> = None;

    while start < clusters.len() {
        let mut end = start + 1;

        while end < clusters.len() && !clusters[end].is_grapheme_start() {
            end += 1;
        }

        let grapheme = &mut clusters[start..end];
        let glyph_count: usize = grapheme
            .iter()
            .map(|cluster| usize::from(cluster.glyph_len()))
            .sum();

        if glyph_count >= 3 {
            let classes = classes.get_or_insert_with(|| {
                font.gdef()
                    .ok()
                    .and_then(|gdef| gdef.glyph_class_def())
                    .and_then(Result::ok)
            });
            let metrics = metrics.get_or_insert_with(|| {
                font.glyph_metrics(Size::new(font_size), LocationRef::new(coords))
            });

            stack_grapheme(
                metrics,
                font_size,
                is_rtl,
                classes.as_ref(),
                grapheme,
                glyphs,
            );
        }

        start = end;
    }
}

/// [`stack_marks`] for one grapheme's clusters.
fn stack_grapheme(
    metrics: &GlyphMetrics<'_>,
    font_size: f32,
    is_rtl: bool,
    classes: Option<&ClassDef<'_>>,
    grapheme: &mut [ShapedCluster],
    glyphs: &mut Vec<Glyph>,
) {
    // A glyph kept in its cluster moves into the glyphs, so it can move.
    for cluster in grapheme.iter_mut() {
        if cluster.has_inline_glyph() {
            let offset = glyphs.len() as u32;

            glyphs.push(Glyph {
                id: cluster.glyph_offset,
                x: 0.,
                y: 0.,
                advance: cluster.advance,
            });
            cluster.glyph_offset = offset;
            cluster.flags = cluster.flags.with_inline_glyph(false);
        }
    }

    // The glyphs in logical order of their clusters, each with its pen position from the
    // grapheme's visual start.
    let mut stored: Vec<(usize, f32)> = Vec::new();
    let mut pen = 0.;
    let visual: Vec<&ShapedCluster> = if is_rtl {
        grapheme.iter().rev().collect()
    } else {
        grapheme.iter().collect()
    };

    for cluster in &visual {
        let start = cluster.glyph_offset as usize;

        for at in start..start + usize::from(cluster.glyph_len()) {
            stored.push((at, pen));
            pen += glyphs[at].advance;
        }
    }

    if stored.len() < 2 {
        return;
    }

    // Back to logical order: clusters' glyphs in the order the text stacks them.
    if is_rtl {
        stored.reverse();
    }

    // Fonts class some combining marks as bases (Liberation's), so a glyph after the
    // grapheme's first with no advance is a mark too.
    let is_mark = |glyph: &Glyph, first: bool| {
        (!first && glyph.advance == 0.)
            || classes.is_some_and(|classes| {
                GlyphId16::try_from(GlyphId::new(glyph.id))
                    .is_ok_and(|id| classes.get(id) == MARK_CLASS)
            })
    };
    let ink_of = |glyph: &Glyph, pen: f32| {
        let bounds = metrics.bounds(GlyphId::new(glyph.id))?;

        (bounds.x_max > bounds.x_min && bounds.y_max > bounds.y_min).then(|| Ink {
            left: pen + glyph.x + bounds.x_min,
            right: pen + glyph.x + bounds.x_max,
            top: glyph.y - bounds.y_max,
            bottom: glyph.y - bounds.y_min,
        })
    };
    let gap = font_size / 16.;
    let mut base: Option<Ink> = None;
    let mut placed: Vec<Placed> = Vec::new();

    for (index, &(at, pen)) in stored.iter().enumerate() {
        let glyph = glyphs[at];
        let Some(mut ink) = ink_of(&glyph, pen) else {
            continue;
        };

        if !is_mark(&glyph, index == 0) {
            base.get_or_insert(ink);

            continue;
        }

        let Some(base) = base else {
            continue;
        };
        let above = (ink.top + ink.bottom) < (base.top + base.bottom);
        let mut dy = 0.;

        // Each move clears one earlier mark and may meet another: at most one move per mark.
        for _ in 0..=placed.len() {
            let Some(hit) = placed
                .iter()
                .filter(|other| other.above == above)
                .find(|other| ink.collides(&other.ink))
            else {
                break;
            };
            let step = if above {
                hit.ink.top - gap - ink.bottom
            } else {
                hit.ink.bottom + gap - ink.top
            };

            dy += step;
            ink = ink.moved(step);
        }

        if dy != 0. {
            glyphs[at].y += dy;
        }

        placed.push(Placed { ink, above });
    }
}
