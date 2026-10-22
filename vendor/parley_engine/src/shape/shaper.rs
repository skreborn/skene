// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shaping of text.

use alloc::vec::Vec;
use core::mem;
use harfrust::ShapeOptions as HarfShapeOptions;
use linebender_resource_handle::{Blob, FontData};
use parlance::{FontFeature, FontVariation, Language};

use crate::{
    Analysis, CharInfo, ShapedText,
    itemize::{Item, Segment, TextRange},
    lru_cache::LruCache,
    shape::{
        CharCluster,
        cache,
        // Skene: the per-word shape cache.
        words::{Context, RunKey, WordCache, WordShape},
    },
};
use core::ops::Range;

/// Shaping options for one item.
///
/// These are styling options relevant for shaping. They're styling, in that they're not derived
/// from the underlying text. When you [shape the text][`Shaper::shape_text`], you should split the
/// text into [`Item`]s at the points where these options change.
#[derive(Debug)]
pub struct ShapeOptions<'a> {
    /// The font size to shape the item with.
    pub font_size: f32,
    /// The language to shape the item with.
    pub language: Option<Language>,
    /// The font features to shape the item with.
    ///
    /// If there are font features with duplicate [tags][`FontFeature::tag`], later values override
    /// earlier values.
    pub features: &'a [FontFeature],
    /// The font variations that are constant over an item.
    pub variations: &'a [FontVariation],
}

/// The font instance to shape an item with.
#[derive(Clone, Debug, PartialEq)]
pub struct FontInstance {
    /// The font.
    pub font: FontData,
    /// Font synthesis suggestions.
    // TODO: Synthesis carries more than we need, and ties us to `fontique`. We can likely change
    // this to opaque user data.
    pub synthesis: fontique::Synthesis,
}

impl FontInstance {
    /// Borrow this font instance.
    #[inline]
    pub fn as_ref(&self) -> FontInstanceRef<'_> {
        FontInstanceRef {
            data: &self.font.data,
            index: self.font.index,
            synthesis: &self.synthesis,
        }
    }
}

/// A borrowed [`FontInstance`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontInstanceRef<'a> {
    /// Blob containing the content of the font file.
    pub data: &'a Blob<u8>,
    /// Index of the font in a collection, or 0 for a single font.
    pub index: u32,
    /// Font synthesis suggestions.
    pub synthesis: &'a fontique::Synthesis,
}

impl From<FontInstanceRef<'_>> for FontInstance {
    #[inline]
    fn from(font: FontInstanceRef<'_>) -> Self {
        Self {
            font: FontData {
                data: font.data.clone(),
                index: font.index,
            },
            synthesis: *font.synthesis,
        }
    }
}

/// Reusable scratch to shape text using [`Self::shape_text`].
pub struct Shaper {
    shape_data_cache: LruCache<cache::ShapeDataKey, harfrust::ShaperData>,
    shape_instance_cache: LruCache<cache::ShapeInstanceId, harfrust::ShaperInstance>,
    shape_plan_cache: LruCache<cache::ShapePlanId, harfrust::ShapePlan>,
    unicode_buffer: Option<harfrust::UnicodeBuffer>,
    features: Vec<harfrust::Feature>,
    char_cluster: CharCluster,
    // Skene: the per-word shape cache, and the glyphs of the run being assembled from its
    // words.
    words: WordCache,
    word_ranges: Vec<(Range<usize>, u32)>,
    run_infos: Vec<harfrust::GlyphInfo>,
    run_positions: Vec<harfrust::GlyphPosition>,
}

impl Default for Shaper {
    fn default() -> Self {
        const MAX_ENTRIES: usize = 16;
        Self {
            shape_data_cache: LruCache::new(MAX_ENTRIES),
            shape_instance_cache: LruCache::new(MAX_ENTRIES),
            shape_plan_cache: LruCache::new(MAX_ENTRIES),
            unicode_buffer: Some(harfrust::UnicodeBuffer::new()),
            features: Vec::new(),
            char_cluster: CharCluster::default(),
            // Skene: the per-word shape cache, and the glyphs of the run being assembled.
            words: WordCache::default(),
            word_ranges: Vec::new(),
            run_infos: Vec::new(),
            run_positions: Vec::new(),
        }
    }
}

impl core::fmt::Debug for Shaper {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Shaper").finish_non_exhaustive()
    }
}

impl Shaper {
    /// Skene: keeps at most `words` shaped words for later runs to reuse (4096 unless set): none
    /// where it is zero, which shapes every run whole.
    pub fn set_word_cache_capacity(&mut self, words: usize) {
        self.words.set_capacity(words);
    }

    /// Skene: how many words were found in the shape cache and how many were shaped since the
    /// shaper was made.
    pub fn word_cache_stats(&self) -> (u64, u64) {
        self.words.stats()
    }

    /// Shape text into glyphs, overwriting `shaped_text`.
    ///
    /// The `text` passed in must be the same as used for producing `analysis`.
    ///
    /// This uses the items returned by `items`, and further itemizes the text into
    /// individually-shapeable segments of constant bidi level and script. `items` should be used to
    /// split on properties like shaping-relevant style changes (e.g., font size) or properties like
    /// language.
    ///
    /// `char_style_indices` holds per-character style indices; these are copied onto
    /// [`Character`][super::Character`] and [`ShapedCluster`][super::ShapedCluster]. A shaped
    /// cluster's style index is that of its logically first constituent character.
    ///
    /// Characters that don't have a particular script have their script resolved based on
    /// surrounding context (see [`Segment::script`]).
    ///
    /// Each segment is then broken into runs of maximal sequences of character clusters for which
    /// `select_font` returns the same font.
    ///
    /// # Panics
    ///
    /// Panics if `items` does not cover the entire source text, or the font returned by
    /// `select_font` is malformed.
    pub fn shape_text<'options>(
        &mut self,
        text: &str,
        analysis: &Analysis,
        // TODO: rename to something like `user_data` (s.t. we don't assume it's a style per se).
        char_style_indices: &[u16],
        items: impl IntoIterator<Item = Item<'options>>,
        mut select_font: impl FontSelector,
        shaped_text: &mut ShapedText,
    ) {
        shaped_text.clear();
        shaped_text.reserve(text.len());

        let char_count = analysis.char_info().len();
        debug_assert_eq!(
            char_style_indices.len(),
            char_count,
            "The number of character style indices must be equal to the character count"
        );

        let mut previous_item_end = 0;
        let mut itemizer = analysis.itemize(text);

        for item in items {
            assert!(
                item.char_end > previous_item_end,
                "item ends must be strictly increasing"
            );
            assert!(
                item.char_end as usize <= char_count,
                "items must not span past the text"
            );

            loop {
                let segment = itemizer
                    .next(
                        #[inline(always)]
                        |text_range| text_range.char_range.end == item.char_end as usize,
                    )
                    .expect("A segment must be yielded, given items tile the full text exactly");

                if shape_segment(
                    self,
                    text,
                    &segment,
                    &item.options,
                    &mut select_font,
                    analysis.char_info(),
                    char_style_indices,
                    shaped_text,
                )
                .is_err()
                {
                    // Abort on error. This happens iff `FontSelector::select_font` failed to return a
                    // font. By aborting we ensure `ShapedText` covers the source text contiguously (as
                    // we need a font to construct `ShapedRun`).
                    return;
                };

                debug_assert!(
                    segment.range.char_range.end <= item.char_end as usize,
                    "Segments must not span past the item."
                );
                if segment.range.char_range.end == item.char_end as usize {
                    break;
                }
            }

            previous_item_end = item.char_end;
        }

        assert_eq!(
            previous_item_end as usize, char_count,
            "`items` does not cover the entire source text"
        );
        debug_assert_eq!(
            shaped_text.characters().len(),
            char_count,
            "All characters must have been processed"
        );
    }
}

/// Shape one segment.
///
/// Returns `Err(())` if shaping should be aborted, which happens iff [`FontSelector::select_font`]
/// returned `None`.
fn shape_segment(
    scx: &mut Shaper,
    text: &str,
    segment: &Segment,
    options: &ShapeOptions<'_>,
    select_font: &mut impl FontSelector,
    char_info: &[CharInfo],
    char_style_indices: &[u16],
    shaped_text: &mut ShapedText,
) -> Result<(), ()> {
    select_font.begin_segment(segment, options);

    let text_range = &segment.range.byte_range;
    let char_range = &segment.range.char_range;

    let segment_text = &text[text_range.clone()];

    // Only process current segment
    let segment_char_info = &char_info[char_range.start..char_range.end];
    let segment_char_style_indices = &char_style_indices[char_range.start..char_range.end];

    if segment_text.is_empty() {
        return Ok(()); // No clusters
    }

    let char_cluster = &mut scx.char_cluster;

    // Shaping inputs that are constant over the segment.
    let direction = if segment.bidi_level.is_rtl() {
        harfrust::Direction::RightToLeft
    } else {
        harfrust::Direction::LeftToRight
    };
    let hb_script = script_to_harfrust(segment.script);
    let language = options
        .language
        .as_ref()
        .and_then(|lang| lang.as_str().parse::<harfrust::Language>().ok());
    scx.features.clear();
    scx.features.extend(options.features.iter().map(|feature| {
        harfrust::Feature::new(
            harfrust::Tag::new(&feature.tag.to_bytes()),
            feature.value as u32,
            ..,
        )
    }));

    // Shape a run of the segment with a single font, appending it to `shaped_text`.
    let mut shape_run = |font: &FontInstance, range: TextRange| {
        // TODO: How do we want to handle errors like this?
        let font_ref =
            harfrust::FontRef::from_index(font.font.data.as_ref(), font.font.index).unwrap();

        // Create harfrust shaper
        let shaper_data = scx.shape_data_cache.entry(
            cache::ShapeDataKey::new(font.font.data.id(), font.font.index),
            || harfrust::ShaperData::new(&font_ref),
        );
        let instance = scx.shape_instance_cache.entry(
            cache::ShapeInstanceKey::new(
                font.font.data.id(),
                font.font.index,
                &font.synthesis,
                Some(options.variations),
            ),
            || {
                harfrust::ShaperInstance::from_variations(
                    &font_ref,
                    variations_iter(&font.synthesis, options.variations),
                )
            },
        );

        let harf_shaper = shaper_data
            .shaper(&font_ref)
            .instance(Some(instance))
            .build();
        let shaper_plan = scx.shape_plan_cache.entry(
            cache::ShapePlanKey::new(
                font.font.data.id(),
                font.font.index,
                &font.synthesis,
                direction,
                hb_script,
                language.clone(),
                &scx.features,
                Some(options.variations),
            ),
            || {
                harfrust::ShapePlan::new(
                    &harf_shaper,
                    direction,
                    Some(hb_script),
                    language.as_ref(),
                    &scx.features,
                )
            },
        );

        // Skene: the run is shaped word by word through the shape cache (see `words`), split
        // after its spaces where nothing the font does crosses them for the run's text, and its
        // glyphs assembled from its words'.
        let run_text = &text[range.byte_range.clone()];
        let run_chars = &char_info[range.char_range.clone()];
        let split = scx.words.is_enabled()
            && run_text.contains(' ')
            && scx.words.splits(
                font.font.data.id(),
                font.font.index,
                &font_ref,
                run_text,
                hb_script,
                direction,
            );
        scx.word_ranges.clear();
        let mut word_start = (0, 0);
        let mut previous = '\0';

        for (index, (offset, ch)) in run_text.char_indices().enumerate() {
            let info = run_chars[index];

            if split
                && previous == ' '
                && ch != ' '
                && info.is_grapheme_start()
                && !info.continues_cluster()
            {
                scx.word_ranges.push((word_start.0..offset, word_start.1));
                word_start = (offset, index as u32);
            }

            previous = ch;
        }

        scx.word_ranges
            .push((word_start.0..run_text.len(), word_start.1));

        let run_key = scx.words.is_enabled().then(|| {
            scx.words.run_key(RunKey {
                font_blob_id: font.font.data.id(),
                font_index: font.font.index,
                variations: variations_iter(&font.synthesis, options.variations)
                    .map(|variation| {
                        (
                            u32::from_be_bytes(variation.tag.to_be_bytes()),
                            variation.value.to_bits(),
                        )
                    })
                    .collect(),
                size: options.font_size.to_bits(),
                features: scx.features.clone(),
                language: language.clone(),
                script: hb_script,
                direction,
            })
        });

        scx.run_infos.clear();
        scx.run_positions.clear();

        // `HarfRust` gives a right-to-left run's glyphs in visual order: its words come last first.
        let count = scx.word_ranges.len();

        for at in 0..count {
            let index = if direction == harfrust::Direction::RightToLeft {
                count - 1 - at
            } else {
                at
            };
            let (bytes, first_char) = scx.word_ranges[index].clone();
            let start = range.byte_range.start + bytes.start;
            let end = range.byte_range.start + bytes.end;
            let mut shape = || {
                shape_word(
                    &mut scx.unicode_buffer,
                    &harf_shaper,
                    shaper_plan,
                    &scx.features,
                    options.font_size,
                    text,
                    start..end,
                    direction,
                    hb_script,
                    language.as_ref(),
                )
            };
            let shaped;
            let word = match &run_key {
                Some(run_key) => {
                    let context = (
                        Context::of(text[..start].chars().rev()),
                        Context::of(text[end..].chars()),
                    );

                    scx.words
                        .get_or_shape(run_key, &text[start..end], context, shape)
                }
                None => {
                    shaped = shape();
                    &shaped
                }
            };

            scx.run_infos.extend(word.infos.iter().map(|info| {
                let mut info = *info;

                info.cluster += first_char;
                info
            }));
            scx.run_positions.extend_from_slice(&word.positions);
        }

        shaped_text.push_run(
            text,
            range,
            segment,
            options,
            char_info,
            char_style_indices,
            font,
            // Skene: the run's glyphs, assembled from its words'; each word returned the buffer.
            &scx.run_infos,
            &scx.run_positions,
            harf_shaper.coords(),
        );
    };

    // This is the run being accumulated, i.e., consecutive graphemes for which `select_font`
    // returns the same font.
    let mut run: Option<(FontInstance, TextRange)> = None;

    // The segment's characters with their analysis data and style indices.
    let mut chars = segment_text
        .chars()
        .zip(segment_char_info)
        .zip(segment_char_style_indices)
        .map(|((ch, &info), &style_index)| (ch, info, style_index));

    // The lengths of the segment's graphemes in characters.
    let grapheme_lengths = segment_char_info
        .chunk_by(|_, info| !info.is_grapheme_start())
        .map(<[_]>::len);

    // The end of the previous grapheme.
    let mut byte_end = text_range.start;
    let mut char_end = char_range.start;

    for grapheme_length in grapheme_lengths {
        let (byte_start, char_start) = (byte_end, char_end);
        char_cluster.fill(chars.by_ref().take(grapheme_length), &mut byte_end);
        char_end += grapheme_length;
        let grapheme = TextRange {
            byte_range: byte_start..byte_end,
            char_range: char_start..char_end,
        };

        let font = select_font
            .select_font(segment, options, char_cluster)
            .ok_or(())?;

        if let Some((run_font, run_range)) = &mut run
            && run_font.as_ref() == font
        {
            run_range.byte_range.end = byte_end;
            run_range.char_range.end = char_end;
        } else {
            if let Some((run_font, run_range)) = run.take() {
                // The font changed: shape the previous run.
                shape_run(&run_font, run_range);
            }
            run = Some((font.into(), grapheme));
        }
    }
    if let Some((run_font, run_range)) = run {
        shape_run(&run_font, run_range);
    }

    Ok(())
}

/// Skene: the text `range` of `text` shaped alone by `shaper` under `plan`, the text around it
/// its context, its clusters counted in characters from its start.
fn shape_word(
    unicode_buffer: &mut Option<harfrust::UnicodeBuffer>,
    shaper: &harfrust::Shaper<'_>,
    plan: &harfrust::ShapePlan,
    features: &[harfrust::Feature],
    font_size: f32,
    text: &str,
    range: Range<usize>,
    direction: harfrust::Direction,
    script: harfrust::Script,
    language: Option<&harfrust::Language>,
) -> WordShape {
    let mut buffer = mem::take(unicode_buffer).unwrap();
    buffer.clear();
    buffer.set_cluster_level(harfrust::BufferClusterLevel::MonotoneCharacters);

    let word = &text[range.clone()];
    buffer.reserve(word.len());
    #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
    for (i, ch) in word.chars().enumerate() {
        // Each cluster's index is its character's within the word, so `data.rs` finds the
        // characters of a cluster by it once the word's place in its run is added.
        buffer.add(ch, i as u32);
    }

    buffer.set_pre_context(&text[..range.start]);
    buffer.set_post_context(&text[range.end..]);
    buffer.set_direction(direction);
    buffer.set_script(script);

    if let Some(language) = language {
        buffer.set_language(language.clone());
    }

    let glyph_buffer = shaper.shape(
        buffer,
        HarfShapeOptions::new()
            .plan(Some(plan))
            .features(features)
            .point_size(Some(font_size)),
    );
    let shaped = WordShape {
        infos: glyph_buffer.glyph_infos().to_vec(),
        positions: glyph_buffer.glyph_positions().to_vec(),
    };

    // Replace buffer to reuse allocation in the next word.
    *unicode_buffer = Some(glyph_buffer.clear());

    shaped
}

#[inline]
fn variations_iter<'a>(
    synthesis: &'a fontique::Synthesis,
    item: &'a [FontVariation],
) -> impl Iterator<Item = harfrust::Variation> + 'a {
    synthesis
        .variation_settings()
        .iter()
        .map(|(tag, value)| harfrust::Variation {
            tag: *tag,
            value: *value,
        })
        .chain(item.iter().map(|variation| harfrust::Variation {
            tag: harfrust::Tag::new(&variation.tag.to_bytes()),
            value: variation.value,
        }))
}

pub(crate) fn script_to_harfrust(script: fontique::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&script.to_bytes()))
        .unwrap_or(harfrust::script::UNKNOWN)
}

/// Implements font selection for shaping.
pub trait FontSelector {
    /// Called when a new segment starts.
    ///
    /// This can be useful to inspect, e.g., the segment's script.
    fn begin_segment(&mut self, segment: &Segment, options: &ShapeOptions<'_>) {
        let _ = (segment, options);
    }

    /// Called once per character cluster within the current segment.
    ///
    /// A character cluster will usually be a grapheme, though if text direction or script changes
    /// mid-grapheme, it will be split over segments.
    ///
    /// Shaping is aborted if this returns `None`; [`ShapedText`] then contains a partial result.
    fn select_font(
        &mut self,
        segment: &Segment,
        options: &ShapeOptions<'_>,
        cluster: &mut CharCluster,
    ) -> Option<FontInstanceRef<'_>>;
}
