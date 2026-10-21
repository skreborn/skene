# parley_engine: stacked combining marks drawn over one another where the font positions each against its base alone

Against: parley_engine 0.12.0 (crates.io), over HarfRust 0.12.0. Not filed; ready to file. A
proposal rather than a bug: Parley draws what HarfRust positions, which is what HarfBuzz does.

## Summary

HarfRust (as HarfBuzz) positions combining marks by the font's `mark` and `mkmk` lookups, and
falls back to stacking them by canonical combining class only where the font has no GPOS mark
positioning at all. Most fonts on a Linux desktop position some marks but not every sequence of
them: two marks above a base are each anchored to the base and not to each other, and are drawn
on top of one another. Zalgo text, IPA, transliterations and the corpus's "Stacked: á̂̃ ǭ ẋ̣ q̣̇"
line hit it.

Measured with Skene's engine at 17 px over the 13 families its system sets Latin in (Inter,
DejaVu Sans/Serif/Sans Mono, Liberation Sans/Serif/Mono, FreeSans, FreeSerif, Cantarell,
Carlito, Caladea, Unifont) and the sequences `Z\u{353}\u{351}\u{352}`, `a\u{301}\u{302}\u{303}`,
`q\u{323}\u{307}`, `x\u{307}\u{323}`, `e\u{302}\u{301}`, `a\u{308}\u{301}`, `u\u{308}\u{304}`
and the corpus line: 19 pairs of marks collide (their ink overlaps by more than a quarter of the
narrower's width and a tenth of the shorter's height) in 12 of the 13 families; Cantarell keeps
them all apart.

Stock glyph positions in Inter (glyph id, x, y; y down):

```text
"Z\u{353}\u{351}\u{352}": [(497, 0.0, 16.0), (2720, 10.7, 16.0), (2718, 10.7, 16.0), (2719, 10.7, 16.0)]
"a\u{301}\u{302}\u{303}": [(509, 0.0, 16.0), (2694, 1.1, 12.3), (1768, 0.3, 8.7)]
```

Z's three marks (two above, one below) are drawn at one position, the two above over each
other; in the second sequence, three glyphs after composition, the font's own positioning sets
the two marks at different heights (12.3 and 8.7), which the pass leaves alone. (Program:
`parley-stock-repro.rs` beside this report.)

## Proposed change

An opt-in (or default-on) pass after shaping, per grapheme of three glyphs or more: in logical
order, a glyph is a mark where `GDEF` classes it as one or where it follows the grapheme's
first glyph with no advance (Liberation's fonts class their combining marks as bases); the base
is the first glyph with ink that is not a mark. A mark whose ink collides with an earlier mark's
on the same side of the base (above or below its center) moves away from the base until it
clears every earlier one there by a sixteenth of the font size, the gap HarfBuzz's fallback
stacks marks with. Marks the font keeps apart do not move. Glyph ink comes from skrifa's glyph
metrics bounds at the run's size and location.

Skene's patch: `vendor/parley_engine/src/shape/stack.rs`, called from `shape/shaped_text.rs`
`push_run` after the clusters are built; a cluster holding its glyph inline moves it into the
glyph list so it can be offset.

## Tests

- `stacked_marks_clear_one_another`: every pair of marks in the families and sequences above (more
  than 50 pairs) does not collide; fails with the pass removed (Inter: the marks of
  `Z\u{353}\u{351}\u{352}` collide).
- `marks_the_font_keeps_apart_stay_where_it_puts_them`: Vietnamese in Inter (whose `mkmk`
  stacks its tone marks): every glyph exactly where HarfRust puts it shaping the text whole.

## Open questions

Whether the pass belongs in HarfRust as an option (it is a positioning fallback, like
`_hb_ot_shape_fallback_mark_position`) rather than in Parley; and whether moving the mark away
from the base, rather than along the stacking direction of its combining class, suits below-base
marks of scripts other than Latin, Greek and Cyrillic (the tests cover those three).
