Skene's copy of `parley_engine` 0.12.0 (crates.io), patched into the workspace through the
root manifest's `[patch.crates-io]` and carried until its changes are proven in practice and
offered upstream; `parley` beside it is the other half. It is not a workspace member: it keeps
its own manifest and lints, and the workspace's formatting, lints and house rules do not reach
it.

Skene's changes, each marked `Skene:` in the source:
- analysis.rs: three flags on each character, read by `parley`'s line breaker and bidi line
  pass: whether it continues the cluster before it (inside a grapheme cluster, a combining mark,
  a character after an invisible stacker), whether it takes the paragraph's level where it ends
  a line (UAX #9 rule L1), and whether it follows a soft hyphen; shape/data.rs carries them on
  `CharacterFlags` and `ShapedClusterFlags`.
- analysis.rs: a soft hyphen is a soft wrap opportunity before the letter or digit after it,
  after a Hebrew letter too, where ICU4X's UAX #14 LB21a allows none.
- shape/shaped_text.rs: a run holding a tab keeps its font's space advance
  (`ShapedRun::space_advance`) and draws a tab as the space at no advance of its own; a run
  holding a soft hyphen keeps the hyphen a break after it draws (`ShapedRun::hyphen`); both are
  mapped through the character map subtable HarfRust reads (`shape::nominal_glyph`) and scaled
  as HarfRust's advances are.
- analysis.rs: a full stop (U+002E) between two letters is a word of its own, a word boundary
  before it and before the letter after it, as Chromium tailors UAX #29's WB6 and WB7, where
  ICU4X keeps `foo.bar` one word.
- shape/stack.rs (new), called from shape/shaped_text.rs: combining marks the font's mark
  positioning leaves colliding are stacked clear of one another, a later mark moving away from
  its base by a sixteenth of the font size until it clears every earlier one on its side.
- shape/words.rs (new), shape/shaper.rs: a per-word shape cache, as Minikin's `LayoutCache`:
  a run is split after its spaces where nothing its font does crosses them for the run's text
  (`Hazard`), each word shaped once for its font, instance, size, features, language, script,
  direction, text and the joining of the text around it (`RunKey`, `Context`), and the run's
  glyphs assembled from its words'; 4096 words unless set (`Shaper::set_word_cache_capacity`).
  shape/shaped_text.rs takes the glyphs as slices to assemble them.

ICU4X's dictionaries for Thai, Lao, Khmer and Burmese (and its Chinese and Japanese one) are
compiled in by the `complex-scripts` feature as published; the workspace's `parley` dependency
enables it.

The licence files (LICENSE-MIT, LICENSE-APACHE) are the published crate's.
