# parley_engine: a per-word shape cache (Minikin's `LayoutCache`, Blink's `ShapeCache`)

Against: parley_engine 0.12.0 (crates.io). Not filed; a proposal with an implementation and
measurements.

## Summary

Parley shapes every run with HarfRust on every layout. Android's Minikin keeps a
`LayoutCache` of shaped words and Blink a `ShapeCache`; a UI that lays the same labels out again
(a list rebuilt, a relayout after a style change elsewhere, a text field's paragraph per edit)
shapes the same words again.

## Proposed change

`Shaper` keeps a `WordCache` (4096 words unless set; `set_word_cache_capacity(0)` turns it off)
in two generations (a word found in the older moves to the newer; when the newer holds half the
capacity it becomes the older), so the words in use stay and a stream of new ones never holds
more than the capacity.

- A word is keyed by everything its shaping depends on: the font (blob id and index), the
  variation settings (synthesis's and the item's), the size's bits, the features, the
  language, the script, the direction (`RunKey`, shared by the words of a run), the word's text,
  and its context on each side as HarfRust reads it. HarfRust reads the pre- and post-context
  only to join cursive scripts (`arabic_joining`: the joining type of the nearest character,
  within five, that is not transparent) and for the dotted circle under the beginning-of-text
  flag, which Parley does not set; so a side's context is `None`, `NonJoining`, or the joining
  character itself.
- A run is split into words after each U+0020 SPACE (at a grapheme start that continues no
  cluster) only where nothing its font does can cross the space for this run's text:
  - `Hazard::of(font)`, once per font: a `GSUB`/`GPOS` sequence (ligature, context, chained
    context, reverse chain, pair, cursive, mark attachment) crosses the space where a position
    before its last takes it, or where its lookup flags skip the space and it has two positions
    or more; a `kern` pair crosses with the space on its left; `morx`, `mort` or `kerx` cross
    always. A crossing sequence can only match where each of its positions does, so it hangs on
    its narrowest position without the space; the hazard is those glyphs and every glyph a
    `GSUB` substitution (single, multiple, alternate, ligature, reverse) may turn into one,
    closed to a fixed point. A crossing sequence none of whose positions both leaves the space
    out and names finitely many glyphs (class 0 names every glyph its class definition leaves
    out) makes the hazard every glyph.
  - `Hazard::allows(run)`: no hazard, or a left-to-right run in a script the default shaper
    shapes (Latin, Greek, Cyrillic, Common: no mirroring, a mapped character kept rather than
    decomposed) with no mark (nothing composes) whose every character the character map
    subtable HarfRust reads maps to a glyph outside the hazard.
  Where a run does not split, it is cached whole by its text.
- Each word is shaped alone with the whole text as its pre- and post-context, and the run's
  glyph infos and positions are assembled from its words' (right to left: last word first),
  each word's clusters offset by its first character.

Fonts on the test system: DejaVu Serif, DejaVu Sans Mono, Liberation Mono, Cantarell, Unifont,
Loma, WenQuanYi Zen Hei, IPAGothic, Noto Color Emoji and OpenSymbol have no hazard. Inter's
`calt` (lookup 31, a chained context with the space in its backtrack) makes 74 glyphs a hazard:
`()*+-:<=>@[]x{}~±×÷`, dashes, arrows and comparison signs, so "3 x 4" is shaped whole and
"hello world" splits. Liberation Sans's `kern` pairs make `A T Y` and their Greek kin a hazard;
Carlito's Greek capitals with tonos; FreeSans `1 2`. Caladea's space kerning reaches every
glyph (a class-0 pair), so its runs are cached whole. Blink's
`HarfBuzzFace::HasSpaceInLigaturesOrKerning` asks the first, all-or-nothing question; Inter,
the default UI font here, fails it.

## Measurements (Skene's engine, release, 4 cores, a busy machine)

Instructions, counted by callgrind (deterministic where wall time on this machine is not), for
`verify shaping`: the 1000-row probe's 3000 texts ("0001", "Item number 1, a row of the list",
"1.37") shaped on a fresh engine seven times one at a time and seven times as a batch on the
engine's threads:

| | all | shaping the paragraphs | Parley's `shape_text` |
|---|---|---|---|
| parley 0.12 as published | 6.35 G | 4.48 G | 2.64 G |
| Skene's changes, no cache | 6.51 G | 4.63 G | 2.73 G |
| Skene's changes, 4096 words | 5.83 G | 3.93 G | 2.02 G |

HarfRust's own shaping (`shape_with_plan`) falls from 1.61 G to 0.74 G instructions; the
cache's lookups and the words' assembly add 0.16 G. On one fresh engine the cache finds 6993
words and shapes 3007 of the 10000 those texts split into. Wall time one at a time, three runs
each, medians of seven rounds: 35.9, 36.1 and 36.0 ms with the cache, 40.1, 40.2 and 43.1 ms
without, 37.6, 41.4 and 38.3 ms before; as a batch on three threads every build lands between
18 and 24 ms, within the machine's noise. HarfRust is about a third of a paragraph's cost here,
beside analysis, font selection, fallback and line breaking.

The same 1000 labels built again on one engine (`verify bench`, its rounds 1 and 2) take 20.4 to
32.3 ms with the cache against 29.5 to 50.9 ms before; callgrind counts 2.62 G instructions for
the whole bench against 2.82 G before, Parley's `shape_text` 409 M against 674 M.

The 1000-row probe's first frame on three display servers, five runs each, the three builds
taking turns round by round (median ms; prepare is styles resolved and texts shaped as a batch):

| | Xvfb | sway | mutter |
|---|---|---|---|
| parley 0.12 as published | 66.6 (prepare 36.9) | 74.2 (41.0) | 74.1 (38.0) |
| Skene's changes, no cache | 70.6 (39.3) | 70.1 (39.1) | 78.1 (40.5) |
| Skene's changes, 4096 words | 67.0 (37.2) | 65.1 (35.9) | 71.5 (36.4) |

The cache wins back what the other changes cost the first frame; the runs spread by 10 ms and
more, so no difference here is beyond the noise.


## Tests

- `the_shape_cache_changes_no_layout`: corpus texts plain and in runs of other sizes, weights,
  slants and a locale, twice each, cached against uncached glyph for glyph; the cache finds
  more words than it shapes.
- `the_shape_cache_keeps_what_a_font_does_across_a_space`: in each of 13 families, text across
  spaces those fonts do something across (Inter's `x` between numbers, arrows, a colon between
  numbers; the `kern` pairs; Greek capitals with tonos), cached against uncached. Fails with
  the hazard ignored.
- `a_run_splits_where_its_font_does_nothing_across_its_spaces`: Inter splits "hello world" and
  shapes "hello 3 x 4" whole.
- `a_word_a_style_splits_is_cached_by_what_joins_it`: an Arabic word cached alone, then joined
  to a letter a size change sets apart before it and after it, takes its medial and final forms.
  Fails with the context left out of the key (glyph 5317 for 5318, the initial ain for the
  medial).
- `the_shape_caches_context_reads_joining_as_harfrust_does`: every character ICU's
  `Joining_Type` calls non-joining joins nothing in HarfRust (DejaVu Sans's beh), and every
  transparent one lets beh join across it, for all assigned characters below U+30000 (over
  100000 checked).
