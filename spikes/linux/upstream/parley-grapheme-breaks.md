# parley: a line break can fall inside a grapheme cluster (emergency breaks, style splits, Khmer and Myanmar stacks)

Against: parley 0.12.0 / parley_engine 0.12.0 (crates.io). Not filed; ready to file.

## Summary

Parley breaks lines at the start of an *atom*, which is a shaped cluster intersected with
grapheme boundaries. Two things make an atom start where no break may fall, and a third may:

1. Itemization splits a grapheme. A style change (here a font size) between `e` and U+0301
   COMBINING ACUTE ACCENT puts the base and its mark in two shaping runs, so the mark starts an
   atom of its own, and an emergency break (`OverflowWrap::Anywhere` / `BreakWord`) is
   recorded between them.
2. Spacing marks that UAX #29 lets start a grapheme cluster. U+102C MYANMAR VOWEL SIGN AA (gc=Mc)
   is one of the spacing marks UAX #29 leaves out of `SpacingMark`, so it starts a grapheme,
   but UAX #14 LB9 keeps it with its base; an emergency break lands between 'မ' and 'ာ'.
3. Invisible stackers. Khmer U+17D2 COENG and Myanmar U+1039 VIRAMA join the next consonant to
   the cluster before them; UAX #29 (whose GB9c conjunct rule covers six Indic scripts, not
   Khmer or Myanmar) puts a grapheme boundary after them. HarfRust's Khmer and Myanmar shapers
   merge a syllable into one cluster, so the atom spans the stack in the cases tried (no break
   after a coeng appears below), but nothing in the line breaker relies on that.

CSS Text 3 (§ 5.1, `overflow-wrap: anywhere`) allows an emergency break only between
typographic character units; a mark split from its base by a style change is not a unit of
its own.

## Reproduction

`OverflowWrap::Anywhere`, `break_all_lines(Some(10.))`, Unifont at 17 px (any font that draws
Myanmar), text `မြန်မာဘာသာစကားသည်`:

```text
Burmese in Unifont at 17 px within 10 px, overflow-wrap anywhere: ["မြ", "န\u{103a}", "မ", "ာဘ", "ာသ", "ာစ", "က", "ားသ", "ည\u{103a} ", "မြ", "န\u{103a}", "မ", "ာန\u{102d}\u{102f}", "င\u{103a}", "င\u{1036}", "၏ ", "ရ\u{102f}\u{1036}", "းသ\u{102f}\u{1036}", "းဘ", "ာသ", "ာစ", "က", "ား ", "ဖြ", "စ\u{103a}", "သ", "ည\u{103a}", "။"]
```

`e\u{301}e\u{301}e\u{301}` in Inter at 17 px, each U+0301 pushed at 23 px, within 5 px:

```text
e + U+0301 thrice in Inter, each acute larger, within 5 px: ["e", "\u{301}e", "\u{301}e", "\u{301}"]
```

The lines break between 'မ' and 'ာ' (U+102C), and between each `e` and its acute. (Program:
`parley-stock-repro.rs` with `parley-stock-repro.Cargo.toml` beside this report, built against
crates.io parley 0.12.0 with `complex-scripts`, `std` and `system`.)

## Proposed change

`parley_engine`'s analysis sets a `CONTINUES_CLUSTER` flag on a character that is not a
grapheme start, is a mark (gc Mn, Mc or Me), or follows an invisible stacker
(`Indic_Syllabic_Category=Invisible_Stacker`); `CharacterFlags` and `ShapedClusterFlags` carry
it. The line breaker records neither a soft wrap opportunity nor an emergency one before an atom
whose first character has the flag, and `ContentWidthsMeasurer` skips soft breaks there too, so
min-content widths agree with what the breaker can do.

The patch is Skene's `vendor/parley_engine/src/analysis.rs` (`continues_cluster`,
`is_invisible_stacker`), `shape/data.rs`, and `vendor/parley/src/layout/line_break.rs` (the
`else if first_character.flags.continues_cluster() {}` arm before the soft wrap and emergency
arms), `layout/data.rs` (`measure_text_run`).

## Tests

- `no_break_falls_inside_a_cluster`: Burmese, Khmer, Lao, Thai and Devanagari corpus texts laid
  out at every width from 10 to 200 px in steps of 5; every line starts at a UAX #29 grapheme
  boundary, not before a mark, not after an invisible stacker.
- `no_break_falls_where_a_style_splits_a_grapheme`: the acute case above at 1, 5, 10, 15 and
  20 px.

Both fail with the line breaker's arm removed (Skene's tree, change disabled): at 10 px the
Burmese text breaks between 'မ' and 'ာ', and the acute text breaks between 'e' and U+0301.
