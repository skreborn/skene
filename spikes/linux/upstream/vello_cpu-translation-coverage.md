# vello_cpu: a shape's coverage changes when it is translated by a whole number of pixels

**Crate:** vello_cpu 0.3.0 (`u8_pipeline`, `RenderMode::OptimizeSpeed`), on x86-64 Linux.

## What happens

A path drawn at `(x, y)` and the same path drawn at `(x, y + n)`, `n` a whole number of pixels,
in a context taller by `n`, should give the same coverage in every pixel, shifted by `n` rows.
For a shape with curved edges at fractional coordinates it does not: a few edge pixels come out
one level apart. The larger the translation, the more pixels differ.

This matters to any renderer that draws a damaged band of a frame into a context of the band's
size (translating the scene by the band's top) and expects the band to match the frame drawn
whole: the two are drawn at different magnitudes, and coverage near an edge rounds differently.

## Reproduction

`vello_cpu-translation-coverage-repro.rs` (manifest beside it) fills a white circle of radius
16.5 at (47.25, 30.75) into a 96x64 context, then the same circle translated down by 4, 8, 100,
400, 1000 and 4000 pixels into a context that much taller, and compares the rows:

```
shifted down 4 px: 0 pixels differ []
shifted down 8 px: 0 pixels differ []
shifted down 100 px: 0 pixels differ []
shifted down 400 px: 0 pixels differ []
shifted down 1000 px: 1 pixels differ [(33, 39, 148, 147)]
shifted down 4000 px: 4 pixels differ [(54, 16, 214, 213), (61, 23, 214, 213), (33, 39, 148, 147), (38, 44, 148, 147)]
5 differing in all
```

(alpha shown: 148 against 147, 214 against 213.)

In an application (Skene's drawn Linux shell at scale 1.5), a switch's rounded thumb redrawn in a
band whose top is 48 device pixels down the frame shows one pixel one level apart from the frame
drawn whole; with a translucent shadow beside it, two pixels.

## Expected

The same coverage at any whole-pixel translation, as long as the shape stays inside the context.

## Likely cause

Line segments are flattened and tiled in `f32` in the context's own coordinates. A coordinate's
precision depends on its magnitude (the unit in the last place doubles with each power of two),
so the same edge, far from the origin, crosses a pixel at a point a few `1e-5` px away from
where it crosses near the origin; a coverage that lands within that distance of a rounding
boundary of the `u8` pipeline flips by one level.

Computing tile intersections relative to each tile's (or strip's) own origin, in coordinates
whose magnitude does not grow with the position in the context, would make coverage
translation-invariant for whole-pixel shifts.

## Workaround

Record the band in the frame's own coordinates, rasterizing only the band (a clip, or a context
the frame's height): every coordinate keeps the frame's magnitude. Skene's GPU path does this;
its CPU path tolerates one level at fractional scales instead (the cost of a frame-sized context per partial frame was not measured).
