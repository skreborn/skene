# parley: a break at a soft hyphen draws no hyphen

Against: parley 0.12.0 (crates.io). Not filed; ready to file.

## Summary

U+00AD SOFT HYPHEN is a line break opportunity, and Parley breaks there, but the line it ends
draws nothing: CSS Text 3 § 5.4 (`hyphens: manual`, the initial value) and every browser and
toolkit draw a hyphen at the end of a line broken after a soft hyphen. The break is also taken
without room for that hyphen, so a client that draws one itself overflows the line, or has to
lay the paragraph out again with each such line made narrower (which Skene's engine did before
this change).

## Reproduction

"Incom\u{AD}pre\u{AD}hen\u{AD}si\u{AD}bil\u{AD}i\u{AD}ties every\u{AD}where" in Inter at
17 px within 90 px:

```text
lines ["Incom\u{ad}pre\u{ad}", "hen\u{ad}si\u{ad}bil\u{ad}i\u{ad}", "ties every\u{ad}", "where"]
line 0: last cluster Some(("\u{ad}", 75.84424, 0.0))
line 1: last cluster Some(("\u{ad}", 65.8584, 0.0))
line 2: last cluster Some(("\u{ad}", 78.27637, 0.0))
line 3: last cluster Some(("e", 39.993164, 9.911133))
```

Each line ending at a soft hyphen ends with the soft hyphen's cluster at no advance: nothing is
drawn there. (Program: `parley-stock-repro.rs` beside this report.)

## Proposed change

- `ShapedRun` keeps the hyphen a break after one of its soft hyphens draws: U+2010 HYPHEN where
  the run's font maps it, else U+002D, with its advance, scaled as HarfRust's advances are.
- The character after a soft hyphen carries a `FOLLOWS_SOFT_HYPHEN` flag.
- The line breaker records the opportunity before such a character with a `ClusterAdvance` for
  the soft hyphen's cluster (the hyphen's advance and glyph), and only where the line with the
  hyphen fits; taking the break adds the hyphen's advance to the line and its advance entry to
  the line's list.
- A line's `ClusterAdvance`s live beside its other per-line data (`LineData::cluster_advances`)
  and reach `EffectiveSpacing`, which adds the advance after the cluster and makes the glyph
  iterator draw the soft hyphen's glyph as the hyphen.
- `ContentWidthsMeasurer` counts the hyphen in a fragment that ends at a soft hyphen's break, so
  min-content holds the widest hyphenated fragment.
- The analysis (`parley_engine`'s `analyze_text`) makes the position after a soft hyphen a soft
  wrap opportunity wherever a letter or digit follows it. ICU4X 2.3's line segmenter applies
  UAX #14 LB21a as "HL (HY | BA) ×", so after a Hebrew letter the soft hyphen (class BA) is no
  opportunity at all: `LineSegmenter::new_auto` reports breaks `[0, 10]` for "מי\u{AD}לה" and
  `[0, 7, 10]` for "Incom\u{AD}pre". Chromium 156 breaks Hebrew there (an inline-block at
  `width: min-content` holding "מי&shy;לה ארו&shy;כה" in DejaVu Sans 17 px lays out four lines,
  31.72 px wide; "ארוכה" alone is 45.69 px), and after a soft hyphen before Latin letters and
  digits too ("מי&shy;abc", "מי&shy;12" each take two lines). Pango 1.52 does not break there.
  Without the change, a Hebrew word wider than its box overflows unbroken, and where an
  overflow-wrap break happens to fall after the soft hyphen, the line draws the soft hyphen's
  own (blank) glyph and no hyphen.

The same per-line `ClusterAdvance` carries a tab's distance to its stop (see the tab-stops
report), so the two land as one mechanism.

## Tests

`a_break_after_a_soft_hyphen_draws_a_hyphen_at_its_width`: the corpus text at 90 and 140 px
breaks where Pango does (Pango's lines are in the test), each line ending at a soft hyphen draws
U+2010's glyph at its advance, and every line fits. With the hyphen removed (Skene's tree, the
hyphen lookup disabled), the drawn glyph is the soft hyphen's own and the test fails.

`a_soft_hyphen_breaks_the_least_width_in_right_to_left_text_too`: the least width of
"מי\u{AD}לה ארו\u{AD}כה מאו\u{AD}ד בע\u{AD}ב\u{AD}רית ובש\u{AD}פות אח\u{AD}רות" in a right-to-left
paragraph is 33.7 px, under its widest unhyphenated word's 54.9 (54.9 against 54.9 without the
analysis change), as the Latin corpus text's is (55.5 against 170.3).

`a_soft_hyphen_in_right_to_left_text_draws_its_hyphen_at_the_lines_end`: the Hebrew text, a
longer Hebrew one and a mixed Latin and Hebrew one, right to left at every width from 20 to 240
px in steps of 4: every line ending at a soft hyphen draws a hyphen glyph, apart from the line's
other glyphs and left of all of them where a Hebrew word ends the line. Without the analysis
change it fails at 20 px: "מי\u{AD}" ends at an overflow-wrap break and draws no hyphen.
