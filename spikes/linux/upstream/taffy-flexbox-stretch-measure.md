# taffy: flexbox measures a stretched item's cross size that a single-line container of definite cross size never reads

Against: taffy 0.15.0 (crates.io). Not filed; ready to file. Skene carries the change below in
its vendored copy (`spikes/linux/vendor/taffy`, marked `Skene:`), as it did on 0.14.0.

## Summary

`determine_hypothetical_cross_size` (`src/compute/flexbox.rs`, CSS Flexbox 9.4 step 7) measures
every item's cross size. In a single-line container whose cross size is definite, step 8
(`calculate_cross_size`) sets the line's cross size to the container's inner cross size without
reading any item's hypothetical cross size, and step 11 (`determine_used_cross_size`) gives an
item with `align-self: stretch`, an `auto` cross size and no `auto` cross margin the line's
cross size, clamped by its min and max. For such an item the step 7 measurement is never read.

A leaf whose cross size is costly to measure (a paragraph laid out at the width it is given,
for a row) pays for a layout that is thrown away, once per item per container layout, on the
most common shape there is: a row of fixed height whose children stretch.

## Reproduction

`taffy-flexbox-stretch-measure-repro.rs` (manifest `taffy-flexbox-stretch-measure-repro.Cargo.toml`)
lays out a row 600x100 of three leaves with the default style (stretched, `auto` size) through
`TaffyTree::compute_layout_with_measure`, records every measure call and counts the
`ComputeSize` calls that ask for a height not yet known. Output
(`taffy-flexbox-stretch-measure-repro.out`):

```text
# taffy 0.15.0
item 0: ComputeSize Vertical known Some(150.0)xNone available 600x100
item 1: ComputeSize Vertical known Some(150.0)xNone available 600x100
item 2: ComputeSize Vertical known Some(150.0)xNone available 600x100
measure calls: 12, of which an unknown height measured: 3
item laid out at 150x100   (each of the three)

# with the change below applied
measure calls: 9, of which an unknown height measured: 0
item laid out at 150x100   (each of the three)
```

The layout is the same; the three height measurements are gone.

## Expected

Step 7 skips the measurement wherever its result cannot be read, and records the size the
stretch will give the item, so anything that does read `hypothetical_*_size` afterwards sees
the final value.

## Proposed change

At the top of the loop in `determine_hypothetical_cross_size`:

```rust
if !constants.is_wrap
    && child.align_self == AlignSelf::STRETCH
    && child.size_style.cross(constants.dir).is_auto()
    && !child.margin_is_auto.cross_start(constants.dir)
    && !child.margin_is_auto.cross_end(constants.dir)
{
    if let Some(node_cross) = constants.node_outer_size.cross(constants.dir) {
        let line_cross = (node_cross
            .maybe_clamp(constants.min_size.cross(constants.dir), constants.max_size.cross(constants.dir))
            - constants.content_box_inset.cross_axis_sum(constants.dir))
        .max(0.0);
        let child_inner_cross = (line_cross - child.margin.cross_axis_sum(constants.dir))
            .maybe_clamp(child.min_size.cross(constants.dir), child.max_size.cross(constants.dir))
            .max(padding_border_sum);

        child.hypothetical_inner_size.set_cross(constants.dir, child_inner_cross);
        child
            .hypothetical_outer_size
            .set_cross(constants.dir, child_inner_cross + child.margin.cross_axis_sum(constants.dir));

        continue;
    }
}
```

`line_cross` is computed exactly as `calculate_cross_size` computes the line's cross size from
the same `known_dimensions`, and the clamp is step 11's.

## Impact in Skene

Skene's Linux realization lays text out through Parley, and without the change a row of
stretched labels measures each label's height at its width once more per layout than the
layout reads.
