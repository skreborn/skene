# vello_cpu: a pixel wholly inside a translucent shape changes by one level with where the tile grid falls

Against: vello_cpu 0.3.0 (crates.io), `u8_pipeline` (the low-precision fine path), on x86-64
with AVX2. Not filed; ready to file.

## Summary

With the `u8` pipeline, a translucent paint composited over a pixel the shape covers entirely
comes out one level apart depending on whether that pixel's 4x4 tile is one an edge of the shape
crosses (an alpha fill, every mask 255 in the covered pixels) or one the shape covers wholly (a
plain fill). Which tiles an edge crosses depends on where the drawing's tile grid falls, so
the same scene drawn into a context whose origin is a few pixels away (a renderer drawing only
the damaged part of a frame, a tiled renderer, a scrolled offscreen) gives a different value
for a pixel no edge touches.

The two paths in `fine/lowp/mod.rs` round differently:

- plain fill (`fill::alpha_composite_solid`, `alpha_composite_inner`):
  `result = src + div_255(bg * (255 - src_a))`
- alpha fill (`alpha_fill::alpha_composite_inner`), with mask `m`:
  `result = div_255(bg * (255 - div_255(src_a * m)) + src * m)`

`div_255(x)` is `(x + 255) >> 8`. At `m = 255` the second is `div_255(bg * (255 - a) + 255 * src)`,
which is not `src + div_255(bg * (255 - a))`: for `bg = 128`, `src = 128`, `a = 128` (white at half
alpha over mid-gray) the plain fill gives `128 + (16256 + 255) >> 8 = 192` and the alpha fill
`(16256 + 32640 + 255) >> 8 = 191`. The exact value is 191.75.

## Reproduction

`vello_cpu-tile-grid-rounding-repro.rs` (its manifest `vello_cpu-tile-grid-rounding-repro.Cargo.toml`)
draws an opaque gray background and a translucent rectangle from `x = start` to 32 as a path into
a 32x4 context, once with the context's origin at the scene's 0 and once at 1, 2 or 3 pixels
in, and compares the scene's pixel (9, 1), which the rectangle covers wholly every time.
Output (`vello_cpu-tile-grid-rounding-repro.out`):

```text
paint 255,0,0,128; rectangle from x=6.25: pixel 9 drawn from origin 0 [192, 64, 64, 255], from origin 2 [191, 64, 64, 255]
paint 30,144,255,128; rectangle from x=6.25: pixel 9 drawn from origin 0 [79, 136, 192, 255], from origin 2 [79, 136, 191, 255]
paint 255,255,255,128; rectangle from x=6.25: pixel 9 drawn from origin 0 [192, 192, 192, 255], from origin 2 [191, 191, 191, 255]
paint 255,0,0,150; rectangle from x=6.25: pixel 9 drawn from origin 0 [203, 53, 53, 255], from origin 2 [202, 53, 53, 255]
...
9 of 270 cases draw a wholly covered pixel differently
```

From origin 0 the rectangle's left edge (6.25) crosses the tile `[4, 8)` and pixel 9 sits in the
wholly covered tile `[8, 12)`; from origin 2 the edge (4.25 there) crosses the tile `[4, 8)` that
now holds pixel 9 (7 there).

The same holds for a shape that starts left of the target: the first tile of each row is an
alpha fill, so a context whose leading edge cuts through translucent content draws those
columns one level away from a context that holds the whole shape.

## Expected

A pixel's value does not depend on which tile it falls in when its coverage is the same (255
here). Drawing a scene in parts, at any origin, gives the pixels drawing it whole gives.

## Impact

Skene repaints only the damaged part of a window into a context the size of that part and
composites it into the window's frame. Before working around this, a repaint of a box at an
arbitrary place drew glyph edges and translucent fills one level away from the frame drawn
whole, so the screen depended on which repaints came before (901 differing pixels on one test
page; screenshots changed by ±1 at glyph edges between runs).

## Workaround in Skene

Every repaint is a band from the frame's leading edge, its top and bottom on the 4-pixel grid,
so tiles fall where they do for the whole frame (`gui-linux/src/paint.rs`,
`Renderer::band`). Its cost is repainting from the leading edge rather than from the damage's.

## Proposed change

Make the two paths agree at full coverage, for instance by compositing the alpha fill as
`div_255(src * m) + div_255(bg * inv)` (which is the plain fill's formula at `m = 255`), or by
using a correctly rounded division by 255 (`(x + 128 + ((x + 128) >> 8)) >> 8`) in both. The
second changes every reference image by at most one level; the first changes only alpha fills.
