Skene's copy of `taffy` 0.15.1 (crates.io), patched into the workspace through the root
manifest's `[patch.crates-io]` and carried until its changes are released upstream. It is not a
workspace member: it keeps its own manifest and lints, and the workspace's formatting, lints and
house rules do not reach it.

Skene's changes, each marked `Skene:` in the source. Each is held by a reproduction in
skene-layout's tests, and `UPSTREAM.md` holds the reports to offer, one per change. All but the
first two and the last make a box lay out alike whatever space it is given, as Chromium lays it
out: laid out
at its natural (max-content) height and then at that height as a definite space, or as its own
height, a box keeps its size and every box beneath it keeps its place, which skene-layout's tests
hold over random trees.

- tree/taffy_tree.rs (`compute_child_layout`): a box laid out hidden, by its own
  `Display::None` or an ancestor's, and a box with no children clear the out-of-flow children
  hoisted onto them. Neither path reaches `compute_oof_layout`, which rewrites the list, so a
  list recorded by an earlier pass outlived the boxes in it: a containing block whose last
  absolute child was removed, or one inside a hidden subtree, kept the removed child's id, and
  `round_layout` panicked on it (`invalid SlotMap key used`).
- tree/taffy_tree.rs (`compute_child_layout`) and tree/cache.rs (`Cache::hold_hidden`): a box
  laid out hidden beneath a `Display::None` ancestor counts its emptied cache as holding entries,
  so a write to it (a subtree moved into it) marks the hidden ancestor dirty and the subtree is
  laid out hidden. Taffy's `mark_dirty` stopped at the empty cache, so the ancestor's hidden
  layout was served from its cache, the moved subtree kept its visible layout, and `round_layout`
  followed the hoisted lists it kept into removed boxes and panicked.
- compute/flexbox.rs (`determine_container_main_size`): a container whose main size is not
  given takes each item at its hypothetical main size, its flex base size clamped by its min
  and max main sizes, the automatic minimum included. Taffy floored the flex base size by the
  specified min main size alone, so a column whose item's flex basis was below its content came
  out shorter than the content in a definite space.
- compute/flexbox.rs (`determine_container_main_size`): a column's intrinsic main size is the
  sum of its items' hypothetical main sizes, as in a definite space: its main axis is its block
  axis, whose intrinsic size is the size its layout gives it. Taffy summed the items' max-content
  contributions, which count an item's preferred height above its flex basis.
- compute/flexbox.rs (`determine_available_space`): a min-content constraint on a column's main
  axis is a max-content one, as the min-content block size of a grid container is its
  max-content block size. Taffy put each item of a wrapping column on a line of its own there.
- compute/flexbox.rs (`collect_flex_lines`, `collect_balanced_flex_lines`): a column with a
  definite max main size wraps against it whether or not its known main size is definite. Taffy
  collected its items into one line wherever its known main size was content-based, so a column
  laid out by an auto-height parent did not wrap where its intrinsic size had wrapped.
- compute/flexbox.rs (`line_limit`, used by `collect_flex_lines` and
  `collect_balanced_flex_lines`): a line may reach its definite space widened by a few units in
  the last place, so items that fill a container stretched to its own max-content size stay on one
  line. Taffy compared with the space itself, which f32 rounding of the container's padding and
  border taken off again left short of the items' sum, so the last item wrapped.
- compute/flexbox.rs (`resolve_flexible_lengths`): a flexed item's target main size is floored at
  its padding and border, as its flex base size and hypothetical main size are. Taffy floored it at
  zero, so an item whose max main size is below its padding and border shrank to it once flexed.
- compute/flexbox.rs (`collect_flex_lines`, `collect_balanced_flex_lines`): a column whose main
  size is not definite wraps against its max main size alone, as without one it does not wrap
  against the space an ancestor hands down. Taffy wrapped it against the smaller of the two.
- compute/flexbox.rs (`determine_flex_base_size`, `determine_container_main_size`): a stretched
  item is measured at its line's cross size only where that size is definite (a single-line
  container of definite cross size), and at fit-content in the available space elsewhere. Taffy
  measured it at the available cross space whenever it was stretched, so an item whose main size
  follows its cross size (through an aspect ratio) took its size from a height it never has.
- compute/grid/mod.rs (`compute_container_constants`) and compute/grid/track_sizing.rs
  (`track_sizing_algorithm`): a grid's block axis is laid out, as Chromium lays its rows out: a
  grid whose height is not given sizes its rows in an indefinite space whatever space an
  ancestor hands down, and its rows are never sized under a min- or max-content constraint, so
  their auto minimums take the items' minimum contributions. Taffy capped the rows by a definite
  available height and used the items' limited content contributions where the space was
  indefinite.
- compute/grid/mod.rs (`compute_grid_layout`): a re-run of the track sizing algorithm that is not
  resolving percentage tracks keeps the axis it sizes as indefinite as the first run had it,
  since its tracks are summed into the container's size again. Taffy used the first run's sum
  as a definite size, so the re-run's flexible tracks filled it.
- compute/grid/track_sizing.rs (`maximise_tracks`): an infinite free space grows each track to
  its growth limit capped by its `fit-content()` limit, as a finite one does. Taffy grew a
  `fit-content()` track past its limit under a max-content constraint.
- tree/taffy_tree.rs (`compute_child_layout`) and tree/cache.rs (`Cache::forget_final_layout`):
  a box laid out in full while an ancestor is only being sized (as a flex row does to read its
  items' baselines) makes every ancestor forget its cached final layout, so the ancestor's next
  full layout lays the box out again. Taffy kept them, so a final layout served from the cache
  left the box as the sizing pass had laid it out.
- tree/cache.rs (`Cache::get_resolving`), tree/taffy_tree.rs (`cache_get`) and style/mod.rs
  (`Style::resolves_parent_height`): a box whose own height, min-height or max-height is a
  percentage is served a cached size only for the parent height it was computed against. Taffy
  compared the parent's width alone.
- compute/flexbox.rs (`compute_flexbox_layout`, `column_wrap_width`): a wrapping column whose
  width is not given settles its width before it breaks its lines, as CSS Flexbox sizes it and
  Chromium lays it out (its largest min-content width contribution at min-content, the width of
  the lines its items break into in their largest max-content contribution at max-content, and
  a definite space clamped between the two), and lays its lines out at that width. Taffy broke
  the lines with the items measured in the space the column was given and took its width from
  them, so a definite height could widen it as if it wrapped where Chromium keeps its width.
- compute/mod.rs (`automatic_ratio_width`), compute/leaf.rs (`compute_leaf_layout`),
  compute/flexbox.rs (`compute_flexbox_layout`) and compute/grid/mod.rs (`compute_grid_layout`):
  a border-box box at an aspect ratio whose width and height are both automatic is at least as
  wide as its padding and border's height through the ratio, its own greatest width capping it,
  as Chromium sizes its min- and max-content widths; a flex or grid container its content leaves
  narrower is laid out again at that width. Taffy took its width from its content alone, so an
  empty box at a ratio was no wider than its padding and border.

The licence file (LICENSE) is the upstream repository's; the published crate carries none.
