# `TaffyTree` keeps a stale hoisted out-of-flow list, and `round_layout` panics on it

Ready to file at https://github.com/DioxusLabs/taffy/issues, by the author's choice.

## Summary

`compute_child_layout` (src/tree/taffy_tree.rs) records a containing block's hoisted
out-of-flow children only through `compute_oof_layout`, which runs after a container layout
and returns early when the output has no positioning area. Two paths never reach it: the leaf
path (a box with no children, measured through the measure function) and the hidden path (a
box laid out under `RunMode::PerformHiddenLayout`, or its own `Display::None`). A list recorded
by an earlier visible pass therefore outlives the boxes in it, and once one of them is removed,
`round_layout` indexes the removed node and panics with `invalid SlotMap key used`
(taffy_tree.rs:564 in 0.15.1). It reproduces on 0.15.1 and on `main` at 4204e36.

## Reproductions

```rust
use taffy::{TaffyTree, prelude::AvailableSpace, style::{Display, Position, Style}};

fn space() -> taffy::geometry::Size<AvailableSpace> {
  taffy::geometry::Size { width: AvailableSpace::Definite(10.), height: AvailableSpace::Definite(10.) }
}

// A containing block whose last child, an absolute one, is removed becomes a leaf.
#[test]
fn a_leaf_keeps_a_stale_hoisted_list() {
  let mut taffy: TaffyTree<()> = TaffyTree::new();
  let child = taffy.new_leaf(Style { position: Position::Absolute, ..Style::default() }).unwrap();
  let root = taffy.new_with_children(Style { position: Position::Relative, ..Style::default() }, &[child]).unwrap();
  taffy.compute_layout(root, space()).unwrap();
  taffy.remove(child).unwrap();
  taffy.compute_layout(root, space()).unwrap(); // panics: invalid SlotMap key used
}

// A containing block inside a `Display::None` subtree keeps its list while hidden.
#[test]
fn a_hidden_box_keeps_a_stale_hoisted_list() {
  let mut taffy: TaffyTree<()> = TaffyTree::new();
  let pinned = taffy.new_leaf(Style { position: Position::Absolute, ..Style::default() }).unwrap();
  let other = taffy.new_leaf(Style { position: Position::Relative, ..Style::default() }).unwrap();
  let boxed = taffy.new_with_children(Style { position: Position::Relative, ..Style::default() }, &[pinned, other]).unwrap();
  let overlay = taffy.new_with_children(Style { position: Position::Absolute, ..Style::default() }, &[boxed]).unwrap();
  let root = taffy.new_with_children(Style { position: Position::Relative, ..Style::default() }, &[overlay]).unwrap();
  taffy.compute_layout(root, space()).unwrap();
  taffy.set_style(overlay, Style { position: Position::Absolute, display: Display::None, ..Style::default() }).unwrap();
  taffy.compute_layout(root, space()).unwrap();
  taffy.remove(pinned).unwrap();
  taffy.compute_layout(root, space()).unwrap(); // panics: invalid SlotMap key used
}
```

## Proposed fix

A box with no children and a box laid out hidden hoist nothing, so both paths clear the list:

```diff
         if inputs.run_mode == RunMode::PerformHiddenLayout {
             debug_log!("HIDDEN");
             self.taffy.nodes[node_id.into()].final_layout = Layout::with_order(0);
+            self.taffy.nodes[node_id.into()].hoisted_children.clear();
             return compute_hidden_layout(self, node_id);
         }
 ...
-                (Display::None, _) => compute_hidden_layout(tree, node_id),
+                (Display::None, _) => {
+                    tree.taffy.nodes[node_id.into()].hoisted_children.clear();
+                    compute_hidden_layout(tree, node_id)
+                }
 ...
                 (_, false) => {
                     let node_key = node_id.into();
+                    tree.taffy.nodes[node_key].hoisted_children.clear();
                     let style = &tree.taffy.nodes[node_key].style;
```

An alternative is for `TaffyTree::remove` to drop the removed node from every hoisted list, which
fixes the panic but keeps a hidden box's list of boxes it no longer lays out.

# A box laid out hidden stops `mark_dirty`, so a subtree moved into it keeps its layout

Ready to file at https://github.com/DioxusLabs/taffy/issues, by the author's choice.

`mark_dirty` stops climbing at a node whose cache is already empty, on the ground that its
ancestors are then dirty too. A box laid out hidden beneath a `Display::None` ancestor breaks that:
`compute_hidden_layout` clears its cache and stores nothing (`Cache::store` skips
`RunMode::PerformHiddenLayout`), while the ancestor's cache holds the hidden output. A write to the
box, such as `set_children` moving a visible subtree into it, then marks the box alone, the
ancestor's hidden layout is served from its cache, and the moved subtree is never laid out hidden:
it keeps the layout it had where it was visible, and `round_layout`, which walks a hidden subtree,
follows the hoisted lists it kept into boxes since removed and panics with `invalid SlotMap key
used`. skene-layout's `a_subtree_moved_into_a_hidden_box_lays_out_hidden` holds it:

```rust
// root > [hidden (display: none) > inner, outer (2x2) > middle (relative) > placed (absolute)]
taffy.compute_layout(root, space).unwrap();
taffy.remove(placed).unwrap();
taffy.set_children(inner, &[outer]).unwrap();
taffy.compute_layout(root, space).unwrap(); // panics: invalid SlotMap key used
```

Fix: a box laid out hidden counts its cache as holding entries (`Cache::hold_hidden`), so marking
it dirty goes on to clear the hidden ancestor, whose next layout lays the box out hidden again.

# Layouts that change with the space they are given

Ready to file at https://github.com/DioxusLabs/taffy/issues, by the author's choice, one issue
per section or one issue for all. Each was found by laying a random tree out from a box at its
natural (max-content) height and then at that height as a definite space, or as its own height,
where the box should keep its size and every box beneath it its place; each section names where taffy 0.15.1 lets the
space it is given leak into a layout, the HTML Chromium (156) lays out for it in px, the numbers
taffy gives in units of 16px, and the change Skene carries for it (marked `Skene:` in its copy).
skene-layout's tests hold each reproduction, named as here.

## A column is shorter than an item shrunk below its content

`a_column_is_as_tall_as_its_item_shrunk_below_its_content`. In a definite space,
`determine_container_main_size` sizes a container whose main size is not given to the sum of its
items' flex base sizes floored by their *specified* min main size, which leaves out the automatic
minimum: `<div style="display:flex;flex-direction:column"><div style="display:flex;
flex-direction:column;flex:0 1 16px"><div style="width:88px;height:24px;flex:none"></div></div>
</div>` is 24px tall in Chromium (the item's automatic minimum is its content); taffy gives 1
in place of 1.5 laid out in a definite space, and 1.5 at max-content. Fix: take each item's
hypothetical main size there (`hypothetical_inner_size`), which already clamps the flex base size
by the used min and max main sizes.

## A column at max-content counts a height its items do not lay out at

`a_column_at_max_content_is_as_tall_as_it_lays_out`. Under a min- or max-content constraint
`determine_container_main_size` sums the items' content contributions (9.9.1), which count an
item's preferred size: in `<div style="display:flex;align-items:flex-start"><div
style="display:flex;flex-direction:column"><div style="height:64px;flex:2 1 0px"><div
style="height:32px"></div></div></div></div>` Chromium lays the column out 32px tall (a column's
main axis is its block axis, whose intrinsic size is what its layout gives it), as taffy does in a
definite space; taffy gives 4 in place of 2 at max-content. Fix: for a column, sum the items'
hypothetical main sizes there too.

## A wrapping column puts each item on a line of its own at min-content

`a_wrapping_column_is_as_tall_at_min_content`. `determine_available_space` passes a min-content
constraint on a column's main axis through, so `collect_flex_lines` puts every item on a line of
its own: `<div style="display:grid;grid-template-rows:min-content"><div style="display:flex;
flex-direction:column;flex-wrap:wrap"><div style="height:16px"></div><div style="height:48px">
</div></div></div>` is 64px tall in Chromium; taffy gives 3 in place of 4. Fix: a min-content
constraint on a column's main axis is a max-content one, as the grid already maps a block-axis
min-content constraint for its expansion steps.

## A column with a max height wraps against a smaller space it is laid out in

`a_column_wraps_against_its_max_height_alone`. `collect_flex_lines` wraps a column whose height
is not definite against its max height, but against the available space where that is smaller,
although without a max height it rightly ignores the available space: `<div style="height:64px">
<div style="display:flex;flex-direction:column;flex-wrap:wrap;max-height:160px">` holding three
48px boxes is one line 144px tall in Chromium; taffy lays it out as three lines in a 4-high space.
Fix: a column wraps against its max main size alone (in `collect_balanced_flex_lines` too).

## A stretched item is measured at the available height

`a_stretched_item_takes_no_size_from_the_available_height` and
`a_row_is_as_wide_in_a_definite_height_as_at_max_content`. `determine_flex_base_size`, and the
intrinsic branch of `determine_container_main_size`, give a stretched item the available cross
space as its known cross size whatever the container's cross size is, flagging it indefinite but
measuring it at it, so an item whose main size follows its cross size takes it from a height it
never has: in a row of auto height, a row holding an empty `aspect-ratio:2.5` box takes a flex
base size of 2.5 times the available height (25 in a 10-high space, 0 at max-content); Chromium
gives it 0. Fix: give the item the line's cross size only where it is definite (a single-line
container with a definite cross size), and measure it at fit-content otherwise (step E).

## A grid's rows are capped by an ancestor's height and sized under a constraint

`a_grid_lays_its_rows_out_at_max_content`. A grid whose height is not given sizes its rows in the
available height (`compute_container_constants`), so a definite height an ancestor hands down caps
the rows' growth, and in an indefinite one it sizes them under a max-content constraint, using the
items' limited content contributions for auto minimums. Chromium lays a grid's rows out: their
space is indefinite and never a constraint. `<div style="display:grid;grid-template:1fr/1fr;
grid-auto-columns:24px;grid-auto-flow:column dense">` holding a scroll container whose content is
32px tall and a `min-height:0; grid-row:span 2 / 1` box whose content is 32px tall is 32px tall in
Chromium; taffy gives 3 in place of 2 at max-content. Fix: in the block axis, an available space
the grid's height does not give is max-content, and the intrinsic step treats it as no
constraint.

## A grid sized again fills the size its first pass summed

`a_grid_sized_again_keeps_its_height_indefinite`. When an item's contribution changes after the
columns are sized again, `compute_grid_layout` sizes the rows again with the container's content
box from the first pass as a definite size, so the flexible rows expand into that stale sum, and
the container's size is then summed from them: `<div style="display:grid;max-width:0;
grid-template-columns:minmax(10%,max-content);grid-template-rows:1fr">` holding an item spanning
two rows whose content wraps to 80px and another whose content wraps to 32px is 112px tall in
Chromium; taffy gives 6 in place of 7 in a definite space. Fix: a re-run not resolving
percentage tracks keeps the axis it sizes as indefinite as the first run had it.

## A `fit-content()` row outgrows its limit at max-content

`a_fit_content_row_keeps_its_limit_at_max_content`. `maximise_tracks` grows every track to its
growth limit in an infinite free space, but to its `fit-content()`-capped growth limit in a finite
one, and the growth limit of a `fit-content()` track is floored by the items' min-content
contribution: `<div style="display:grid;grid-auto-rows:fit-content(64px)">` holding a
`min-height:0` item with 96px of content is 64px tall in Chromium; taffy gives 6 in place of 4 at
max-content. Fix: cap the infinite case by the `fit-content()` limit too.

## A baseline read while sizing leaves its layout behind

`a_baseline_read_while_sizing_leaves_no_layout_behind`. `calculate_children_base_lines` lays a
baseline-aligned item out in full (`RunMode::PerformLayout`) to read its baseline, also while its
container is only being sized, which rewrites the item's subtree; when the container's final
layout is then served from the cache, nothing lays the subtree out again. In a row holding an
`align-items:baseline` row holding a `height:0%` row with a stretched child, laying the outer row
out at max-content and then in a definite height leaves the stretched child 1.5 high where
Chromium (and the first layout) lays it out at 0. Fix: a full layout run while an ancestor is
being sized makes every ancestor forget its cached final layout (`Cache::forget_final_layout`),
keeping its measure entries.

## A size computed against one parent height is served against another

`a_percentage_min_height_is_sized_per_parent_height`. A measure entry matches on the parent's
width alone (`x_axis_parent_size`), so a box whose own height, min-height or max-height is a
percentage is served a size computed against another parent height (or a definite one where it is
now indefinite): a `min-height:100%` box in a wrapping row of width 32px, measured once inside a
definite height, is served that size later, and its row comes out 5 high where Chromium gives
56px (3.5). Fix: for such a box (`Style::resolves_parent_height`), match the whole parent size
(`Cache::get_resolving`).

## A container stretched to its own max-content size wraps its last item

`a_row_stretched_to_its_own_width_keeps_one_line`. A container stretched to (or given) its own
max-content size takes its padding and border off a size they were added to, and in f32 the result
can fall short of the sum of the items it was summed from by a unit in the last place, so
`collect_flex_lines` wraps the last item onto a line of its own: `<div style="display:flex;
flex-direction:column"><div style="display:flex;flex-wrap:wrap;border-left:16px solid"><div
style="width:5.3333333px;height:16px;flex:none"></div><div style="width:12.8px;height:16px;
flex:none"></div></div></div>` is one line 16px tall in Chromium, whose fixed-point lengths sum
exactly; taffy puts the second item on a second line below a row 1 high. Fix: a line may reach its
definite space widened by a few units in the last place (`line_limit`, in
`collect_balanced_flex_lines` too).

## A flexed item comes out smaller than its padding and border

`a_flexed_item_keeps_its_border_above_its_max_height`. The min/max violation step of
`resolve_flexible_lengths` floors a target main size, a border-box size, at zero rather than at
the item's padding and border, as its flex base size and hypothetical main size are floored, so an
item whose max main size is below its padding and border shrinks to that max once flexed: in
`<div style="display:flex;flex-direction:column;width:16px;height:80px"><div
style="display:grid;overflow-x:clip;overflow-y:scroll;max-height:32px;border-top:32px solid;
border-bottom:32px solid"></div></div>` (border-box sizing) the item is 64px tall in Chromium;
taffy gives 2 in place of 4. Fix: floor the clamped target size at the padding and border.

## A wrapping column with a max height does not wrap inside a column of auto height

`a_column_in_an_automatic_column_wraps_against_its_max_height`. `collect_flex_lines` collects
every item into one line wherever the container's known main size is not definite, so a wrapping
column with a max height, laid out by a column of auto height, does not wrap, although its
intrinsic size (and Chromium) wraps it against the max height: in `<div style="display:flex;
flex-direction:column"><div style="display:flex;flex-direction:column;flex-wrap:wrap;
max-height:0"><div style="display:grid"></div><div style="display:grid;padding:25.6px 32px
6.4px 0"></div></div></div>` Chromium puts the two boxes on two lines, the empty one 0px wide;
taffy puts them on one, the empty box stretched 2 wide. Fix: a column with a definite max main
size wraps against it whether or not its known main size is definite (in
`collect_balanced_flex_lines` too).

## A wrapping column breaks its lines before its width is settled

`a_wrapping_column_settles_its_width_before_its_lines` and
`a_wrapping_column_is_as_wide_as_its_widest_item_at_min_content`. A `column wrap` container whose
width is not given breaks its lines with its items measured in the space it is given (at
min-content, each item at its min-content width) and takes its width from those lines. CSS
Flexbox §9.9.2 sizes it by first finding the largest min-content or max-content width
contribution among its items, then using that as the available width for each item during
layout, and Chromium settles it so: its min-content width is its largest min-content
contribution, its max-content width that of the lines its items break into laid out in their
largest max-content contribution, and in a definite space it is that space clamped between
the two. In `<div style="display:flex;width:0;height:64px"><div style="display:flex;
flex-direction:column;flex-wrap:wrap">` holding two boxes of wrapping content (a row of 64px,
40px and 16px words, and one of 16px and 76px words, 16px apart and 16px high), Chromium sizes
the column 76px wide (its widest word) and lays it out as one line; taffy measures the boxes at
min-content (48px and 32px tall), which break into two lines, and sizes the column 140px wide
(8.75 in place of 4.75). A column 32px tall holding a 10px and a 20px box, each 32px tall, is
20px wide at min-content in Chromium and 30px at max-content; taffy gives 30px at both. Fix:
settle the width first as above (`column_wrap_width`), then lay the column out at it.

## A box at an aspect ratio takes its width from its content alone

`an_empty_box_at_a_ratio_is_as_wide_as_its_border_through_it`. `compute_leaf_layout`,
`compute_flexbox_layout` and `compute_grid_layout` size a box whose width and height are both
`auto` from its content's width, so an `aspect-ratio` box is no wider than its content, its
padding and border included. Chromium floors its min- and max-content widths at its automatic
minimum height (its padding and border, for `box-sizing:border-box`) transferred through the
ratio (css-sizing-4 §5.2): an empty `display:grid; aspect-ratio:0.5; border-bottom:16px solid`
box is 8px by 16px in a flex row and in a grid that places it at its start, and the same box as
a flex or grid container holding a 4px by 40px box is 8px by 56px in a flex row; taffy gives
the first no width and the second 4px. Fix: floor such a box's content width at its padding and
border's height times the ratio, capped by its max width (`automatic_ratio_width`), laying a flex
or grid container out again at it where its content leaves it narrower.
