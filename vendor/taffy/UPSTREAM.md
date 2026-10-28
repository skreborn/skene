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

# Layouts that change with the space they are given

Ready to file at https://github.com/DioxusLabs/taffy/issues, by the author's choice, one issue
per section or one issue for all. Each was found by laying a random tree out from a box at its
natural (max-content) height and then at that height as a definite space, where the box should
keep its size and every box beneath it its place; each section names where taffy 0.15.1 lets the
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

Chromium also sizes an `aspect-ratio` box whose width and height are both `auto` from its
content height through the ratio (the second reproduction's row is 8px wide there, from the box's
16px border); taffy sizes it from its content width (0). That gap is not changed.

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
