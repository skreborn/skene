// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Skene: shaping a run word by word through a cache, as Minikin's `LayoutCache` and Blink's
//! `ShapeCache` do.
//!
//! A run is split after each space where nothing its font does crosses that space for the run's
//! text ([`Hazard`]), each word is shaped once for everything its shaping depends on (the
//! [`RunKey`], the word's text and what `HarfRust` reads of the text around it, its
//! [`Context`]), and the run's glyphs are assembled from its words'. Where the font's lookups may
//! cross its space for the run's text, the whole run is the one word, still cached by its text.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::hash::{BuildHasher, Hash, Hasher};

use hashbrown::{DefaultHashBuilder, HashMap, HashSet};
use icu_properties::CodePointMapData;
use icu_properties::props::{GeneralCategory, GeneralCategoryGroup, JoiningType};
use skrifa::raw::tables::gpos::{PairPos, PositionSubtables, ValueRecord};
use skrifa::raw::tables::gsub::{SingleSubst, SubstitutionSubtables};
use skrifa::raw::tables::kern::SubtableKind;
use skrifa::raw::tables::layout::{
    ChainedSequenceContext, ClassDef, CoverageTable, LookupFlag, SequenceContext,
};
use skrifa::raw::types::BigEndian;
use skrifa::raw::{ReadError, TableProvider as _};
use skrifa::{FontRef, GlyphId16, Tag};

/// How many characters of context `HarfRust` keeps a side of the text it shapes
/// (`CONTEXT_LENGTH`).
const CONTEXT_LENGTH: usize = 5;

/// What `HarfRust` can read of the text on one side of what it shapes.
///
/// It reads the context only to join cursive scripts: the joining type of the nearest character
/// (within [`CONTEXT_LENGTH`]) that is not transparent to joining. A non-joining one reads as
/// [`Self::NonJoining`] whatever it is; any other is kept itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Context {
    /// No character, or only characters transparent to joining.
    None,
    /// A character that joins nothing.
    NonJoining,
    /// A character that may join, kept itself.
    Joining(char),
}

impl Context {
    /// The context `chars` (nearest first) give.
    pub(crate) fn of(chars: impl Iterator<Item = char>) -> Self {
        let joining = const { CodePointMapData::<JoiningType>::new() };

        for ch in chars.take(CONTEXT_LENGTH) {
            match joining.get(ch) {
                JoiningType::Transparent => continue,
                JoiningType::NonJoining => return Self::NonJoining,
                _ => return Self::Joining(ch),
            }
        }

        Self::None
    }
}

/// Everything a run's shaping depends on beyond its text and its context: its font and the
/// instance of it, its size, and the options it is shaped with.
#[derive(Clone, Debug, PartialEq, Hash)]
pub(crate) struct RunKey {
    /// The font's blob.
    pub(crate) font_blob_id: u64,
    /// The font's index in its blob.
    pub(crate) font_index: u32,
    /// The variation settings, the synthesis's then the item's, as tags and value bits.
    pub(crate) variations: Vec<(u32, u32)>,
    /// The font size's bits.
    pub(crate) size: u32,
    /// The features.
    pub(crate) features: Vec<harfrust::Feature>,
    /// The language.
    pub(crate) language: Option<harfrust::Language>,
    /// The script.
    pub(crate) script: harfrust::Script,
    /// The direction.
    pub(crate) direction: harfrust::Direction,
}

/// A word cached: what it was shaped under, its text and its context on each side.
struct WordKey {
    run: Arc<RunKey>,
    text: alloc::boxed::Box<str>,
    context: (Context, Context),
}

impl Hash for WordKey {
    /// Hashes as [`WordCache::hash`] does a word's parts.
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self.run).hash(state);
        (*self.text).hash(state);
        self.context.hash(state);
    }
}

/// A word's glyphs as `HarfRust` shaped it, their clusters counted in characters from the word's
/// start.
pub(crate) struct WordShape {
    /// The glyphs, in the order `HarfRust` gave them.
    pub(crate) infos: Vec<harfrust::GlyphInfo>,
    /// Their positions.
    pub(crate) positions: Vec<harfrust::GlyphPosition>,
}

/// The cache of shaped words, in two generations: a word found in the older moves to the newer,
/// and once the newer holds half the capacity, it becomes the older and the older is dropped, so
/// the words in use stay and a run of new ones never holds more than the capacity.
pub(crate) struct WordCache {
    newer: HashMap<WordKey, WordShape>,
    older: HashMap<WordKey, WordShape>,
    hasher: DefaultHashBuilder,
    capacity: usize,
    /// The run key last asked about, shared by the words cached under it.
    run: Option<Arc<RunKey>>,
    hits: u64,
    misses: u64,
    /// What each font's lookups cross its space for, by its blob and index.
    hazards: HashMap<(u64, u32), Hazard>,
}

impl Default for WordCache {
    fn default() -> Self {
        let hasher = DefaultHashBuilder::default();

        Self {
            newer: HashMap::with_hasher(hasher.clone()),
            older: HashMap::with_hasher(hasher.clone()),
            hasher,
            capacity: Self::DEFAULT_CAPACITY,
            run: None,
            hits: 0,
            misses: 0,
            hazards: HashMap::default(),
        }
    }
}

impl WordCache {
    /// The words a cache keeps unless told otherwise: Minikin's `LayoutCache` keeps 5000.
    pub(crate) const DEFAULT_CAPACITY: usize = 4096;

    /// Keeps at most `capacity` words: none where it is zero, which shapes every run whole.
    pub(crate) fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
        self.newer.clear();
        self.older.clear();
    }

    /// Whether the cache keeps any word.
    pub(crate) fn is_enabled(&self) -> bool {
        self.capacity > 0
    }

    /// The words found and the words shaped since the cache was made.
    pub(crate) fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }

    /// Whether a run of `text` in `font` (by its blob and index), `script` and `direction`
    /// shapes as its words shaped apart, the font's [`Hazard`] worked out once a font.
    pub(crate) fn splits(
        &mut self,
        blob: u64,
        index: u32,
        font: &FontRef<'_>,
        text: &str,
        script: harfrust::Script,
        direction: harfrust::Direction,
    ) -> bool {
        self.hazards
            .entry((blob, index))
            .or_insert_with(|| Hazard::of(font))
            .allows(font, text, script, direction)
    }

    /// The shared key equal to `run`.
    pub(crate) fn run_key(&mut self, run: RunKey) -> Arc<RunKey> {
        match &self.run {
            Some(last) if **last == run => Arc::clone(last),
            _ => {
                let run = Arc::new(run);

                self.run = Some(Arc::clone(&run));
                run
            }
        }
    }

    /// The hash a word is kept under, by the hasher both generations share.
    fn hash(&self, run: &RunKey, text: &str, context: (Context, Context)) -> u64 {
        let mut hasher = self.hasher.build_hasher();

        run.hash(&mut hasher);
        text.hash(&mut hasher);
        context.hash(&mut hasher);
        hasher.finish()
    }

    /// The word `text` shaped under `run` in `context`, shaped by `shape` where the cache does
    /// not hold it.
    pub(crate) fn get_or_shape(
        &mut self,
        run: &Arc<RunKey>,
        text: &str,
        context: (Context, Context),
        shape: impl FnOnce() -> WordShape,
    ) -> &WordShape {
        let hash = self.hash(run, text, context);
        let matches = |key: &WordKey| {
            (Arc::ptr_eq(&key.run, run) || *key.run == **run)
                && *key.text == *text
                && key.context == context
        };

        if self.newer.raw_entry().from_hash(hash, matches).is_some() {
            self.hits += 1;
        } else {
            let found = match self.older.raw_entry_mut().from_hash(hash, matches) {
                hashbrown::hash_map::RawEntryMut::Occupied(entry) => Some(entry.remove_entry()),
                hashbrown::hash_map::RawEntryMut::Vacant(_) => None,
            };
            let (key, shaped) = match found {
                Some(found) => {
                    self.hits += 1;
                    found
                }
                None => {
                    self.misses += 1;
                    (
                        WordKey {
                            run: Arc::clone(run),
                            text: text.into(),
                            context,
                        },
                        shape(),
                    )
                }
            };

            if self.newer.len() >= self.capacity.div_ceil(2) {
                self.older =
                    core::mem::replace(&mut self.newer, HashMap::with_hasher(self.hasher.clone()));
            }

            if let hashbrown::hash_map::RawEntryMut::Vacant(entry) =
                self.newer.raw_entry_mut().from_hash(hash, |_| false)
            {
                entry.insert_hashed_nocheck(hash, key, shaped);
            }
        }

        self.newer
            .raw_entry()
            .from_hash(hash, matches)
            .map(|(_, shaped)| shaped)
            .expect("the word was just kept")
    }
}

/// What a font's lookups cross its space for: what a run's text avoids to shape as its words
/// shaped apart, split after its spaces.
#[derive(Debug)]
pub(crate) enum Hazard {
    /// Nothing: every run splits.
    None,
    /// The glyphs a lookup crosses the space for, and every glyph a substitution may turn into
    /// one: a run splits where each of its characters maps to a glyph outside them.
    Glyphs {
        /// The glyphs.
        glyphs: HashSet<GlyphId16>,
        /// The ASCII characters that map to a glyph outside them, a bit each.
        ascii: u128,
    },
    /// Any glyph: no run splits.
    All,
}

impl Hazard {
    /// The hazard of `font`.
    ///
    /// A lookup crosses the space where it matches a sequence (a ligature, a context, a chained
    /// context, a pair, a cursive or mark attachment) holding the space at a position before its
    /// last, or a sequence of two glyphs or more where its lookup flags skip the space; so does a
    /// `kern` pair with the space on its left, and any AAT table that rewrites the text. A
    /// sequence matches only where each of its positions does, so one that crosses hangs on the
    /// glyphs of its narrowest position without the space, and on every glyph a substitution may
    /// turn into one of those. A lookup that acts on one glyph alone (a single, multiple or
    /// alternate substitution, a single positioning) cannot cross a space. Where `HarfRust` maps
    /// characters through a conversion (a symbol or Mac Roman character map), any crossing makes
    /// the hazard every glyph.
    ///
    /// Blink's `HarfBuzzFace::HasSpaceInLigaturesOrKerning` asks whether any lookup holds the
    /// space at all, and splits no run of a font where one does.
    pub(crate) fn of(font: &FontRef<'_>) -> Self {
        let Some(space) =
            super::nominal_glyph(font, ' ').and_then(|space| GlyphId16::try_from(space).ok())
        else {
            return Self::All;
        };

        if [b"morx", b"mort", b"kerx"]
            .iter()
            .any(|tag| font.table_data(Tag::new(tag)).is_some())
        {
            return Self::All;
        }

        let mut crossing = Crossing {
            space,
            glyphs: Some(HashSet::new()),
        };

        if crossing.gather(font).is_err() {
            return Self::All;
        }

        let glyphs = match crossing.glyphs {
            Some(glyphs) if glyphs.is_empty() => return Self::None,
            Some(glyphs) => substituted_into(font, glyphs),
            None => None,
        };
        let map = font.cmap().ok().and_then(|cmap| cmap.best_subtable());

        // A run is checked against the character map `HarfRust` reads, through no conversion.
        let (Some(glyphs), Some((_, record, map))) = (glyphs, map) else {
            return Self::All;
        };

        if record.is_symbol() || record.is_mac_roman() {
            return Self::All;
        }

        let ascii = (0..128_u8).fold(0_u128, |ascii, code| {
            let outside = map
                .map_codepoint(code)
                .and_then(|glyph| GlyphId16::try_from(glyph).ok())
                .is_some_and(|glyph| !glyphs.contains(&glyph));

            ascii | u128::from(outside) << code
        });

        Self::Glyphs { glyphs, ascii }
    }

    /// Whether a run of `text` in `font`, `script` and `direction` shapes as its words shaped
    /// apart.
    ///
    /// Where the hazard is glyphs, the run splits only where `HarfRust` gives each of its
    /// characters the glyph the font's character map does, and none is a hazard: left to right
    /// (so no character is mirrored), in a script the default shaper shapes (which keeps each
    /// character the font maps rather than decomposing it), without a mark (so nothing composes)
    /// and with every character in the character map.
    pub(crate) fn allows(
        &self,
        font: &FontRef<'_>,
        text: &str,
        script: harfrust::Script,
        direction: harfrust::Direction,
    ) -> bool {
        let (glyphs, ascii) = match self {
            Self::None => return true,
            Self::All => return false,
            Self::Glyphs { glyphs, ascii } => (glyphs, *ascii),
        };

        if direction != harfrust::Direction::LeftToRight
            || ![
                harfrust::script::LATIN,
                harfrust::script::GREEK,
                harfrust::script::CYRILLIC,
                harfrust::script::COMMON,
            ]
            .contains(&script)
        {
            return false;
        }

        let categories = const { CodePointMapData::<GeneralCategory>::new() };
        let mut map = None;

        text.chars().all(|ch| {
            if ch.is_ascii() {
                return (ascii >> u32::from(ch)) & 1 != 0;
            }

            let map = map.get_or_insert_with(|| {
                font.cmap()
                    .ok()
                    .and_then(|cmap| cmap.best_subtable())
                    .map(|(_, _, map)| map)
            });

            !GeneralCategoryGroup::Mark.contains(categories.get(ch))
                && map
                    .as_ref()
                    .and_then(|map| map.map_codepoint(ch))
                    .and_then(|glyph| GlyphId16::try_from(glyph).ok())
                    .is_some_and(|glyph| !glyphs.contains(&glyph))
        })
    }
}

/// One position of a sequence a lookup matches: the glyphs it takes.
enum Position<'a> {
    /// One glyph.
    Glyph(GlyphId16),
    /// The glyphs a coverage holds.
    Covered(CoverageTable<'a>),
    /// The glyphs a class definition gives a class: for class 0, every glyph it classes not.
    Class(ClassDef<'a>, u16),
    /// The glyphs a coverage holds that a class definition gives a class.
    CoveredClass(CoverageTable<'a>, ClassDef<'a>, u16),
}

impl Position<'_> {
    /// Whether it takes `glyph`.
    fn holds(&self, glyph: GlyphId16) -> bool {
        match self {
            Self::Glyph(own) => *own == glyph,
            Self::Covered(coverage) => coverage.get(glyph).is_some(),
            Self::Class(classes, class) => classes.get(glyph) == *class,
            Self::CoveredClass(coverage, classes, class) => {
                coverage.get(glyph).is_some() && classes.get(glyph) == *class
            }
        }
    }

    /// How many glyphs it takes: `None` for class 0, which takes every glyph not classed.
    fn population(&self) -> Option<usize> {
        match self {
            Self::Glyph(_) => Some(1),
            Self::Covered(coverage) => Some(coverage.population()),
            Self::Class(_, 0) => None,
            Self::Class(classes, class) => {
                Some(classes.iter().filter(|(_, own)| own == class).count())
            }
            Self::CoveredClass(coverage, classes, class) => Some(
                coverage
                    .iter()
                    .filter(|glyph| classes.get(*glyph) == *class)
                    .count(),
            ),
        }
    }

    /// Adds the glyphs it takes to `glyphs`, where it has a [`Self::population`].
    fn add_to(&self, glyphs: &mut HashSet<GlyphId16>) {
        match self {
            Self::Glyph(glyph) => {
                glyphs.insert(*glyph);
            }
            Self::Covered(coverage) => glyphs.extend(coverage.iter()),
            Self::Class(_, 0) => {}
            Self::Class(classes, class) => glyphs.extend(
                classes
                    .iter()
                    .filter(|(_, own)| own == class)
                    .map(|(glyph, _)| glyph),
            ),
            Self::CoveredClass(coverage, classes, class) => glyphs.extend(
                coverage
                    .iter()
                    .filter(|glyph| classes.get(*glyph) == *class),
            ),
        }
    }
}

/// The glyphs a font's lookups cross its space for, gathered sequence by sequence.
struct Crossing {
    /// The space's glyph.
    space: GlyphId16,
    /// The glyphs so far: `None` once any glyph may cross.
    glyphs: Option<HashSet<GlyphId16>>,
}

impl Crossing {
    /// Gathers every sequence `font` matches: an error where a table does not read.
    fn gather(&mut self, font: &FontRef<'_>) -> Result<(), ReadError> {
        if let Ok(kern) = font.kern() {
            for subtable in kern.subtables() {
                let SubtableKind::Format0(pairs) = subtable?.kind()? else {
                    self.glyphs = None;
                    return Ok(());
                };

                for pair in pairs.pairs() {
                    if pair.left() == self.space {
                        self.sequence(
                            &[Position::Glyph(pair.left()), Position::Glyph(pair.right())],
                            false,
                        );
                    }
                }
            }
        }

        // The class lookup flags skip the space by: its `GDEF` class, or a base where the font
        // classes no glyph, as `HarfRust` synthesizes it.
        let class = match font.gdef().ok().and_then(|gdef| gdef.glyph_class_def()) {
            Some(classes) => classes?.get(self.space),
            None => 1,
        };
        let skips = |flag: LookupFlag| match class {
            1 => flag.contains(LookupFlag::IGNORE_BASE_GLYPHS),
            2 => flag.contains(LookupFlag::IGNORE_LIGATURES),
            3 => {
                flag.contains(LookupFlag::IGNORE_MARKS)
                    || flag.contains(LookupFlag::USE_MARK_FILTERING_SET)
                    || flag.mark_attachment_class().is_some()
            }
            _ => false,
        };

        if let Ok(gsub) = font.gsub() {
            for lookup in gsub.lookup_list()?.lookups().iter() {
                let lookup = lookup?;
                let subtables = lookup.subtables()?;
                let sequence = !matches!(
                    subtables,
                    SubstitutionSubtables::Single(_)
                        | SubstitutionSubtables::Multiple(_)
                        | SubstitutionSubtables::Alternate(_)
                        | SubstitutionSubtables::EmptyExtension
                );

                let skipping = sequence && skips(lookup.lookup_flag());

                match subtables {
                    SubstitutionSubtables::Ligature(subtables) => {
                        for table in subtables.iter() {
                            let table = table?;

                            for (first, set) in
                                table.coverage()?.iter().zip(table.ligature_sets().iter())
                            {
                                for ligature in set?.ligatures().iter() {
                                    let positions: Vec<_> =
                                        core::iter::once(Position::Glyph(first))
                                            .chain(
                                                ligature?
                                                    .component_glyph_ids()
                                                    .iter()
                                                    .map(|glyph| Position::Glyph(glyph.get())),
                                            )
                                            .collect();

                                    self.sequence(&positions, skipping);
                                }
                            }
                        }
                    }
                    SubstitutionSubtables::Contextual(subtables) => {
                        for table in subtables.iter() {
                            self.context(&table?, skipping)?;
                        }
                    }
                    SubstitutionSubtables::ChainContextual(subtables) => {
                        for table in subtables.iter() {
                            self.chained(&table?, skipping)?;
                        }
                    }
                    SubstitutionSubtables::Reverse(subtables) => {
                        for table in subtables.iter() {
                            let table = table?;
                            let mut positions = Vec::new();

                            for coverage in table.backtrack_coverages().iter() {
                                positions.push(Position::Covered(coverage?));
                            }

                            positions.reverse();
                            positions.push(Position::Covered(table.coverage()?));

                            for coverage in table.lookahead_coverages().iter() {
                                positions.push(Position::Covered(coverage?));
                            }

                            self.sequence(&positions, skipping);
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Ok(gpos) = font.gpos() {
            for lookup in gpos.lookup_list()?.lookups().iter() {
                let lookup = lookup?;
                let subtables = lookup.subtables()?;
                // A mark attachment finds its base or mark by flags of its own.
                let sequence = matches!(
                    subtables,
                    PositionSubtables::Pair(_)
                        | PositionSubtables::Cursive(_)
                        | PositionSubtables::Contextual(_)
                        | PositionSubtables::ChainContextual(_)
                );

                let skipping = sequence && skips(lookup.lookup_flag());

                match subtables {
                    // Any pair may span a space its lookup skips.
                    PositionSubtables::Pair(_) | PositionSubtables::Cursive(_) if skipping => {
                        self.glyphs = None;
                    }
                    PositionSubtables::Pair(subtables) => {
                        for table in subtables.iter() {
                            self.pair(&table?)?;
                        }
                    }
                    PositionSubtables::Cursive(subtables) => {
                        for table in subtables.iter() {
                            let coverage = table?.coverage()?;

                            self.sequence(
                                &[
                                    Position::Covered(coverage.clone()),
                                    Position::Covered(coverage),
                                ],
                                false,
                            );
                        }
                    }
                    PositionSubtables::MarkToBase(subtables) => {
                        for table in subtables.iter() {
                            let table = table?;

                            self.sequence(
                                &[
                                    Position::Covered(table.base_coverage()?),
                                    Position::Covered(table.mark_coverage()?),
                                ],
                                false,
                            );
                        }
                    }
                    PositionSubtables::MarkToLig(subtables) => {
                        for table in subtables.iter() {
                            let table = table?;

                            self.sequence(
                                &[
                                    Position::Covered(table.ligature_coverage()?),
                                    Position::Covered(table.mark_coverage()?),
                                ],
                                false,
                            );
                        }
                    }
                    PositionSubtables::MarkToMark(subtables) => {
                        for table in subtables.iter() {
                            let table = table?;

                            self.sequence(
                                &[
                                    Position::Covered(table.mark2_coverage()?),
                                    Position::Covered(table.mark1_coverage()?),
                                ],
                                false,
                            );
                        }
                    }
                    PositionSubtables::Contextual(subtables) => {
                        for table in subtables.iter() {
                            self.context(&table?, skipping)?;
                        }
                    }
                    PositionSubtables::ChainContextual(subtables) => {
                        for table in subtables.iter() {
                            self.chained(&table?, skipping)?;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// A sequence matching `positions` in order, by a lookup that skips the space where
    /// `skipping`: where it may span the space, it hangs on its narrowest position that does not
    /// take the space.
    ///
    /// A sequence spans the space where a position before its last takes it, or, by a lookup that
    /// skips the space, wherever it has two positions or more; such a lookup never matches the
    /// space, so its positions hang on the glyphs they take besides it.
    fn sequence(&mut self, positions: &[Position<'_>], skipping: bool) {
        let space = self.space;
        let Some(glyphs) = &mut self.glyphs else {
            return;
        };
        let spans = if skipping {
            positions.len() > 1
        } else {
            positions
                .split_last()
                .is_some_and(|(_, leading)| leading.iter().any(|position| position.holds(space)))
        };

        if !spans {
            return;
        }

        let narrowest = positions
            .iter()
            .filter(|position| skipping || !position.holds(space))
            .filter_map(|position| Some((position.population()?, position)))
            .min_by_key(|(population, _)| *population);

        match narrowest {
            Some((_, position)) => {
                position.add_to(glyphs);
                glyphs.remove(&space);
            }
            None => self.glyphs = None,
        }
    }

    /// The sequences of a context, by a lookup that skips the space where `skipping`.
    fn context(&mut self, context: &SequenceContext<'_>, skipping: bool) -> Result<(), ReadError> {
        match context {
            SequenceContext::Format1(table) => {
                for (first, set) in table.coverage()?.iter().zip(table.seq_rule_sets().iter()) {
                    let Some(set) = set else {
                        continue;
                    };

                    for rule in set?.seq_rules().iter() {
                        let positions: Vec<_> = core::iter::once(Position::Glyph(first))
                            .chain(
                                rule?
                                    .input_sequence()
                                    .iter()
                                    .map(|glyph| Position::Glyph(glyph.get())),
                            )
                            .collect();

                        self.sequence(&positions, skipping);
                    }
                }
            }
            SequenceContext::Format2(table) => {
                let coverage = table.coverage()?;
                let classes = table.class_def()?;

                for (first, set) in table.class_seq_rule_sets().iter().enumerate() {
                    let Some(set) = set else {
                        continue;
                    };

                    for rule in set?.class_seq_rules().iter() {
                        let positions: Vec<_> = core::iter::once(Position::CoveredClass(
                            coverage.clone(),
                            classes.clone(),
                            first as u16,
                        ))
                        .chain(
                            rule?
                                .input_sequence()
                                .iter()
                                .map(|class| Position::Class(classes.clone(), class.get())),
                        )
                        .collect();

                        self.sequence(&positions, skipping);
                    }
                }
            }
            SequenceContext::Format3(table) => {
                let mut positions = Vec::new();

                for coverage in table.coverages().iter() {
                    positions.push(Position::Covered(coverage?));
                }

                self.sequence(&positions, skipping);
            }
        }

        Ok(())
    }

    /// The sequences of a chained context, its backtrack in reading order, by a lookup that skips
    /// the space where `skipping`.
    fn chained(
        &mut self,
        context: &ChainedSequenceContext<'_>,
        skipping: bool,
    ) -> Result<(), ReadError> {
        match context {
            ChainedSequenceContext::Format1(table) => {
                for (first, set) in table
                    .coverage()?
                    .iter()
                    .zip(table.chained_seq_rule_sets().iter())
                {
                    let Some(set) = set else {
                        continue;
                    };

                    for rule in set?.chained_seq_rules().iter() {
                        let rule = rule?;
                        let glyph = |glyph: &BigEndian<GlyphId16>| Position::Glyph(glyph.get());
                        let positions: Vec<_> = rule
                            .backtrack_sequence()
                            .iter()
                            .rev()
                            .map(glyph)
                            .chain(core::iter::once(Position::Glyph(first)))
                            .chain(rule.input_sequence().iter().map(glyph))
                            .chain(rule.lookahead_sequence().iter().map(glyph))
                            .collect();

                        self.sequence(&positions, skipping);
                    }
                }
            }
            ChainedSequenceContext::Format2(table) => {
                let coverage = table.coverage()?;
                let backtrack = table.backtrack_class_def()?;
                let input = table.input_class_def()?;
                let lookahead = table.lookahead_class_def()?;

                for (first, set) in table.chained_class_seq_rule_sets().iter().enumerate() {
                    let Some(set) = set else {
                        continue;
                    };

                    for rule in set?.chained_class_seq_rules().iter() {
                        let rule = rule?;
                        let positions: Vec<_> = rule
                            .backtrack_sequence()
                            .iter()
                            .rev()
                            .map(|class| Position::Class(backtrack.clone(), class.get()))
                            .chain(core::iter::once(Position::CoveredClass(
                                coverage.clone(),
                                input.clone(),
                                first as u16,
                            )))
                            .chain(
                                rule.input_sequence()
                                    .iter()
                                    .map(|class| Position::Class(input.clone(), class.get())),
                            )
                            .chain(
                                rule.lookahead_sequence()
                                    .iter()
                                    .map(|class| Position::Class(lookahead.clone(), class.get())),
                            )
                            .collect();

                        self.sequence(&positions, skipping);
                    }
                }
            }
            ChainedSequenceContext::Format3(table) => {
                let mut positions = Vec::new();

                for coverage in table.backtrack_coverages().iter() {
                    positions.push(Position::Covered(coverage?));
                }

                positions.reverse();

                for coverage in table
                    .input_coverages()
                    .iter()
                    .chain(table.lookahead_coverages().iter())
                {
                    positions.push(Position::Covered(coverage?));
                }

                self.sequence(&positions, skipping);
            }
        }

        Ok(())
    }

    /// The pairs of a pair positioning that start with the space and move either glyph.
    fn pair(&mut self, table: &PairPos<'_>) -> Result<(), ReadError> {
        let space = self.space;

        match table {
            PairPos::Format1(table) => {
                let Some(index) = table.coverage()?.get(space) else {
                    return Ok(());
                };

                for record in table
                    .pair_sets()
                    .get(usize::from(index))?
                    .pair_value_records()
                    .iter()
                {
                    self.sequence(
                        &[
                            Position::Glyph(space),
                            Position::Glyph(record?.second_glyph()),
                        ],
                        false,
                    );
                }
            }
            PairPos::Format2(table) => {
                if table.coverage()?.get(space).is_none() {
                    return Ok(());
                }

                let seconds = table.class_def2()?;
                let firsts = table
                    .class1_records()
                    .get(usize::from(table.class_def1()?.get(space)))?;

                for (class, record) in firsts.class2_records().iter().enumerate() {
                    let record = record?;

                    if !moves(record.value_record1()) && !moves(record.value_record2()) {
                        continue;
                    }

                    self.sequence(
                        &[
                            Position::Glyph(space),
                            Position::Class(seconds.clone(), class as u16),
                        ],
                        false,
                    );
                }
            }
        }

        Ok(())
    }
}

/// Whether a value record moves its glyph.
fn moves(value: &ValueRecord) -> bool {
    [
        value.x_placement(),
        value.y_placement(),
        value.x_advance(),
        value.y_advance(),
    ]
    .iter()
    .any(|delta| delta.is_some_and(|delta| delta != 0))
        || [
            value.x_placement_device,
            value.y_placement_device,
            value.x_advance_device,
            value.y_advance_device,
        ]
        .iter()
        .any(|device| !device.get().is_null())
}

/// `glyphs` and every glyph a `GSUB` substitution may turn into one of them: an error where the
/// table does not read.
fn substituted_into(
    font: &FontRef<'_>,
    mut glyphs: HashSet<GlyphId16>,
) -> Option<HashSet<GlyphId16>> {
    let Ok(gsub) = font.gsub() else {
        return Some(glyphs);
    };
    let lookups = gsub.lookup_list().ok()?;

    // Each pass adds what turns into a glyph found so far, until a pass adds nothing.
    loop {
        let found = glyphs.len();

        for lookup in lookups.lookups().iter() {
            match lookup.ok()?.subtables().ok()? {
                SubstitutionSubtables::Single(subtables) => {
                    for table in subtables.iter() {
                        match table.ok()? {
                            SingleSubst::Format1(table) => {
                                let delta = table.delta_glyph_id();

                                for glyph in table.coverage().ok()?.iter() {
                                    let into =
                                        GlyphId16::new(glyph.to_u16().wrapping_add_signed(delta));

                                    if glyphs.contains(&into) {
                                        glyphs.insert(glyph);
                                    }
                                }
                            }
                            SingleSubst::Format2(table) => {
                                for (glyph, into) in table
                                    .coverage()
                                    .ok()?
                                    .iter()
                                    .zip(table.substitute_glyph_ids())
                                {
                                    if glyphs.contains(&into.get()) {
                                        glyphs.insert(glyph);
                                    }
                                }
                            }
                        }
                    }
                }
                SubstitutionSubtables::Multiple(subtables) => {
                    for table in subtables.iter() {
                        let table = table.ok()?;

                        for (glyph, sequence) in
                            table.coverage().ok()?.iter().zip(table.sequences().iter())
                        {
                            if sequence
                                .ok()?
                                .substitute_glyph_ids()
                                .iter()
                                .any(|into| glyphs.contains(&into.get()))
                            {
                                glyphs.insert(glyph);
                            }
                        }
                    }
                }
                SubstitutionSubtables::Alternate(subtables) => {
                    for table in subtables.iter() {
                        let table = table.ok()?;

                        for (glyph, set) in table
                            .coverage()
                            .ok()?
                            .iter()
                            .zip(table.alternate_sets().iter())
                        {
                            if set
                                .ok()?
                                .alternate_glyph_ids()
                                .iter()
                                .any(|into| glyphs.contains(&into.get()))
                            {
                                glyphs.insert(glyph);
                            }
                        }
                    }
                }
                SubstitutionSubtables::Ligature(subtables) => {
                    for table in subtables.iter() {
                        let table = table.ok()?;

                        for (first, set) in table
                            .coverage()
                            .ok()?
                            .iter()
                            .zip(table.ligature_sets().iter())
                        {
                            for ligature in set.ok()?.ligatures().iter() {
                                let ligature = ligature.ok()?;

                                if glyphs.contains(&ligature.ligature_glyph()) {
                                    glyphs.insert(first);
                                    glyphs.extend(
                                        ligature.component_glyph_ids().iter().map(BigEndian::get),
                                    );
                                }
                            }
                        }
                    }
                }
                SubstitutionSubtables::Reverse(subtables) => {
                    for table in subtables.iter() {
                        let table = table.ok()?;

                        for (glyph, into) in table
                            .coverage()
                            .ok()?
                            .iter()
                            .zip(table.substitute_glyph_ids())
                        {
                            if glyphs.contains(&into.get()) {
                                glyphs.insert(glyph);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if glyphs.len() == found {
            return Some(glyphs);
        }
    }
}
