// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Spike patch (skene `spikes/android`): a process-wide rule that sets each shaped glyph's
//! advance, so a platform whose text engine advances by hinted, whole-pixel widths (Android's
//! Minikin over Skia and FreeType) can be matched before line breaking.

use alloc::boxed::Box;
use alloc::sync::Arc;
use std::sync::RwLock;

use linebender_resource_handle::FontData;
use parlance::NormalizedCoord;

/// The advances of one run's glyphs.
pub type RunAdvances<'a> = Box<dyn Fn(u32, i32) -> Option<f32> + 'a>;

/// Sets glyph advances in place of the shaper's.
pub trait AdvanceRule: Send + Sync {
    /// The advances of a run in `font` at `coords` and `font_size` (layout units): given a
    /// glyph and its shaped advance in font units (positioning included), the advance in layout
    /// units, or `None` to keep the shaper's. `None` leaves the whole run alone.
    fn run<'a>(
        &'a self,
        font: &'a FontData,
        coords: &'a [NormalizedCoord],
        font_size: f32,
    ) -> Option<RunAdvances<'a>>;
}

static RULE: RwLock<Option<Arc<dyn AdvanceRule>>> = RwLock::new(None);

/// Sets the process's advance rule; `None` restores the shaper's advances.
pub fn set_advance_rule(rule: Option<Arc<dyn AdvanceRule>>) {
    if let Ok(mut slot) = RULE.write() {
        *slot = rule;
    }
}

/// The process's advance rule.
pub(crate) fn advance_rule() -> Option<Arc<dyn AdvanceRule>> {
    RULE.read().ok().and_then(|slot| slot.clone())
}
