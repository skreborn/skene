# parley: a wrapped bidi line's trailing whitespace keeps its run's level (UAX #9 rule L1)

Against: parley 0.12.0 (crates.io). Not filed; ready to file.

## Summary

UAX #9 rule L1 resets the whitespace (and segment separators, isolate initiators and
terminators, and the characters rule X9 removes) at the end of each line to the paragraph
embedding level. Parley 0.12 does not apply L1 (`hanging_whitespace` and
`ContentWidthsMeasurer` say so in comments), so a line of an LTR paragraph that wraps after
right-to-left text keeps its trailing space at the RTL level: the space is reordered with the
RTL run and drawn on that run's left, inside the line, instead of at the line's end edge where
it hangs. The line's width counts it, so alignment, the caret after the space and the line's
measured width are off by the space's advance, and min-content measurement disables hanging
for any item whose level is not the paragraph's.

## Reproduction

"Hello שלום עולם world" in DejaVu Sans at 17 px within 120 px, LTR paragraph:

```text
lines ["Hello שלום ", "עולם world"]
line 0 width 91.5: ["\"H\"@0.0+12.8", "\"e\"@12.8+10.5", "\"l\"@23.2+4.7", "\"l\"@28.0+4.7", "\"o\"@32.7+10.4", "\" \"@43.1+5.4", "\" \"@48.5+5.4", "\"ם\"@53.9+11.3", "\"ו\"@65.2+4.6", "\"ל\"@69.8+9.7", "\"ש\"@79.5+12.0"]
line 1 width 88.4: ["\"ם\"@0.0+11.3", "\"ל\"@11.3+9.7", "\"ו\"@20.9+4.6", "\"ע\"@25.6+10.6", "\" \"@36.2+5.4", "\"w\"@41.6+13.9", "\"o\"@55.5+10.4", "\"r\"@65.9+7.0", "\"l\"@72.9+4.7", "\"d\"@77.6+10.8"]
```

Line 0's trailing space (the second `" "`, at 48.5) is drawn between "Hello " and the Hebrew,
inside the line, and the line's advance (91.5) counts it. Expected (Pango, Chromium): the space
at the line's right edge, after the Hebrew, hanging out of the line's advance. (Program:
`parley-stock-repro.rs` beside this report.)

## Proposed change

After a line's items are committed (`commit_line`), `reset_trailing_whitespace` walks the
line's items from the logical end: the atoms whose characters all take the paragraph level at
a line's end (a `CharInfo` flag set from the bidi class: WS, S, B, BN, LRE, RLE, LRO, RLO, PDF,
LRI, RLI, FSI, PDI) are split off their item into an item at the paragraph's level; an
out-of-flow inline box is skipped, an in-flow one ends the sequence. `needs_reorder` is then
computed over the line's items, and `Run::is_rtl` reports the level the run has on its line.
`hanging_whitespace` then hangs the sequence as it does in a single-level line, and
`ContentWidthsMeasurer` lets trailing whitespace hang in a run of any level.

Skene's patch: `vendor/parley/src/layout/line_break.rs` (`reset_trailing_whitespace`, its call
in `commit_line`), `layout/data.rs` (`can_hang`), `layout/run.rs` (`is_rtl`),
`vendor/parley_engine/src/analysis.rs` (`takes_paragraph_level_at_line_end`).

## Tests

`a_wrapped_bidi_lines_trailing_blanks_hang_at_its_end`: the corpus's LTR-base and RTL-base
bidi texts at several widths; on each line with trailing blanks, every blank cluster sits at the
line's end edge (right of the ink in LTR, left in RTL) and the line's width is its ink's.
With the call removed it fails: `"Hello שלום "`: the blank at 14..15 is not right of the line's
text.

Against Pango (the same fontconfig): before, the bidi cases' widths differed by up to 5.8 px
(LTR base) and 4.8 px (RTL base); after, by 1.0 and 0.7 px, Pango rounding advances to whole
pixels.
