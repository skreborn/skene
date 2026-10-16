Skene's copy of `taffy` 0.14.0 (crates.io), carried until the changes are offered upstream
or a taffy release that has them is taken.

Skene's changes, each marked `Skene:` in the source, measure a child only where a step reads
what it measures:
- compute/grid/track_sizing.rs: an item's min-content and max-content contributions are
  measured only where a track it spans has a sizing function that uses them, as taffy 0.15
  does (`GridItem::spans_track_matching`, in compute/grid/types/grid_item.rs).
- compute/flexbox.rs: in a single-line flex container of definite cross size, a stretched
  item's hypothetical cross size is the line's, which the stretch gives it in any case, and
  is not measured.
- compute/flexbox.rs: a size asked of the main axis alone returns once the container's main
  size is known, the items' cross sizes never laid out for it.
- Cargo.toml: the examples and the dev-dependencies are left out.
