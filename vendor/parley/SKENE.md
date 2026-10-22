Skene's copy of `parley` 0.12.0 (crates.io), patched into the workspace through the root
manifest's `[patch.crates-io]` and carried until its changes are proven in practice and offered
upstream; `parley_engine` beside it is the other half. It is not a workspace member: it keeps
its own manifest and lints, and the workspace's formatting, lints and house rules do not reach it.

Skene's changes, each marked `Skene:` in the source:
- layout/line_break.rs: no break, an emergency one included, falls before a cluster that
  continues the one before it (inside a grapheme a style or script split, before a combining
  mark, after an invisible stacker), the flag `parley_engine` sets.
- layout/line_break.rs: UAX #9 rule L1 for a wrapped line (`reset_trailing_whitespace`): the
  whitespace, separators and isolate and removed formatting characters ending the line take the
  paragraph's level, an item holding them and more splitting in two, so they are drawn at the
  line's end edge, where they hang; whether the line needs reordering is found after.
  layout/data.rs: such whitespace hangs in a run of any level when measuring min- and
  max-content widths; layout/run.rs: `Run::is_rtl` is the run's level on its line.
- layout/line_break.rs: a break after a soft hyphen (U+00AD) draws a hyphen (U+2010, or U+002D
  where the font lacks it) at the hyphen's advance, and is an opportunity only where the line
  holds the hyphen; layout/data.rs: min-content widths count it (`soft_hyphen_before`);
  layout/run.rs: the line draws the soft hyphen's cluster as the hyphen's glyph.
- style/mod.rs: `TabSize` (CSS's `tab-size`), a style property (`StyleProperty::TabSize`,
  `TextStyle::tab_size`), eight spaces unless set, resolved in resolve/mod.rs and kept in the
  layout's style (layout/mod.rs); layout/line_break.rs: a tab advances to the first stop at
  least half a space ahead, measured from the line's start with its indent;
  layout/data.rs: min- and max-content widths measure each tab on its own line.
- layout/spacing.rs: `ClusterAdvance`, an advance a line gives a cluster beyond its shaped
  one (a tab's distance to its stop, a soft hyphen's hyphen and its glyph), kept per line in
  layout/data.rs (`LayoutData::cluster_advances`, `LineData::cluster_advances`) and read through
  `EffectiveSpacing`, which borrows them (layout/line.rs, layout/run.rs,
  layout/whitespace.rs).
- context.rs: `LayoutContext::set_shape_cache_capacity` and `shape_cache_stats`, the shape
  cache `parley_engine` keeps.

The licence files (LICENSE-MIT, LICENSE-APACHE) are the published crate's.
