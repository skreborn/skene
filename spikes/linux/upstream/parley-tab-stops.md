# parley: a tab advances by its glyph, not to a tab stop (`tab-size`)

Against: parley 0.12.0 (crates.io). Not filed; ready to file.

## Summary

A tab (U+0009) is shaped like any character and advances by its glyph (the font's `.notdef`,
its space, or its own tab glyph, whatever it maps), so tab-separated columns do not align. CSS
Text 3 § 4.2 `tab-size` (initial `8`) sets tab stops a multiple of the space's advance (with
letter and word spacing) apart, measured from the line's start, and a tab advances to the next
stop at least half a space ahead (CSS: `0.5ch`; Blink: half the space).

## Reproduction

"col1\tcol2\tcol3\n1\t22\t333" in Inter at 17 px, unwrapped:

```text
line 0: ["\"c\"@0.0+9.7", "\"o\"@9.7+10.2", "\"l\"@19.9+4.1", "\"1\"@24.0+6.9", "\"\\t\"@30.9+11.2", "\"c\"@42.1+9.7", "\"o\"@51.8+10.2", "\"l\"@62.0+4.1", "\"2\"@66.1+10.4", "\"\\t\"@76.5+11.2", "\"c\"@87.6+9.7", "\"o\"@97.4+10.2", "\"l\"@107.5+4.1", "\"3\"@111.7+10.5", "\"\\n\"@122.2+0.0"]
line 1: ["\"1\"@0.0+6.9", "\"\\t\"@6.9+11.2", "\"2\"@18.1+10.4", "\"2\"@28.4+10.4", "\"\\t\"@38.8+11.2", "\"3\"@50.0+10.5", "\"3\"@60.5+10.5", "\"3\"@71.0+10.5"]
```

Each tab advances 11.2 px wherever it falls: "col2" starts at 42.1 and "22" at 18.1. Expected:
"col2" and "22" start at one stop, "col3" and "333" at the next. (Program:
`parley-stock-repro.rs` beside this report.)

## Proposed change

- A `TabSize` style property (`Spaces(f32)`, eight by default, or `Length(f32)` in layout
  units), resolved and scaled with the rest.
- `ShapedRun::space_advance`: the advance of the run font's space where the run holds a tab;
  the tab's own cluster draws the space glyph at no advance.
- The line breaker gives a tab's cluster a `ClusterAdvance` of its distance to the next stop
  from the line's start (its indent included), so wrapping, alignment and hit testing see it;
  `ContentWidthsMeasurer` measures each tab where it falls on the min-content and on the
  max-content line, and hangs a trailing tab by its advance on each.

Skene's patch: `vendor/parley/src/style/mod.rs` (`TabSize`, `advance_from`),
`resolve/mod.rs`, `layout/mod.rs`, `layout/line_break.rs`, `layout/data.rs`,
`layout/spacing.rs`; `vendor/parley_engine/src/shape/shaped_text.rs`.

## Tests

`a_tab_advances_to_a_stop_eight_spaces_apart` (the columns above align, the stops eight spaces
apart) and `an_engine_sets_its_tab_stops` (`TabSize::Length(50.)`). Both fail with
`advance_from` returning the spacing alone.

Against Pango: before, the tabs case's line widths were 122.2 and 81.5 px where Pango's are 115
and 113; after, 111.0 and 108.0. The rest is where the stops fall: Pango's serialized layout puts
them at 40 and 80 px (its advances are whole pixels), Parley's at 38.2 and 76.4 px, eight
times Inter's 4.78 px space at 17 px.
