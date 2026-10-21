> Superseded by `vello_gpu-upload-ordering.md` (vello_hybrid is now vello_gpu 0.3.0, which fixed
> the second case below and keeps the first). Kept for its history; file the newer report.

# vello_hybrid: an image uploaded after `destroy_image`, or after an upload that grows the atlas, is overwritten at submit

Against: vello_hybrid 0.2.0 (crates.io), wgpu 29.0.4, Vulkan (Mesa lavapipe 25.2.8 here; the
ordering is wgpu's, not the driver's). Not filed; ready to file.

## Summary

`Renderer::upload_image` writes an image's pixels with `queue.write_texture`
(`impl AtlasWriter for Pixmap`, `render/wgpu/mod.rs`), but two other atlas operations record
commands into the caller's `CommandEncoder`:

- `Renderer::destroy_image` clears the freed slot with a render pass in the encoder
  (`clear_atlas_region`, `LoadOp::Load` and a scissored clear);
- an upload that needs another atlas layer makes a larger texture array and records a copy of
  the old layers into it in the encoder (`Programs::maybe_resize_atlas_texture_array`,
  `copy_atlas_texture_data`).

wgpu runs every `queue.write_texture` issued before a `queue.submit` ahead of the submitted
command buffers. So within one encoder:

1. `destroy_image(a)` then `upload_image(b)` where `b` takes `a`'s slot: `b`'s pixels are written,
   then the clear recorded before it runs and erases them;
2. `upload_image(big)` growing the array, then `upload_image(c)` into an old layer: `c`'s pixels are
   written into the new array, then the copy of the old layers runs and overwrites them.

The image draws as transparent (or as whatever the old layer held), with no error.

## Reproduction

`vello_hybrid-upload-ordering-repro.rs` (manifest `vello_hybrid-upload-ordering-repro.Cargo.toml`)
uploads 16x16 images, draws the last one at the top left of a 64x64 target in the same encoder,
submits and reads pixel (2, 2). Output on lavapipe (`vello_hybrid-upload-ordering-repro.out`):

```text
adapter: "llvmpipe (LLVM 20.1.2, 256 bits)"
1. upload after destroy_image in one encoder: drawn [0, 0, 0, 0], expected [0, 255, 0, 255]
   with the encoder submitted after destroy_image: drawn [0, 255, 0, 255]
   atlas side 4096
   atlas layers 1 -> 2
2. upload after an upload that grew the atlas, in one encoder: drawn [0, 0, 0, 0], expected [0, 255, 0, 255]
   atlas layers now 3
   with the encoder submitted after the growth: drawn [0, 255, 0, 255]
```

## Expected

Atlas operations take effect in the order they are called, whatever encoder or queue path each
uses: an image uploaded after a slot is freed, or after the array grows, draws its pixels.

## Impact

Skene draws glyph masks as atlas images. A frame that starts the atlas over (destroying every
image) and then uploads the frame's glyphs, or that grows the atlas in the middle of its glyphs,
lost glyphs on the GPU path: 183649 of 183649 dark pixels missing on one test page, 24617
pixels differing from the CPU path on another, and an intermittent failure on mutter.

## Workaround in Skene

`gui-linux/src/gpu.rs`, `Gpu::submit_so_far`: the frame's encoder is submitted (and a fresh one
started) after the atlas is cleared to start over, and after any upload that grows the atlas
array, so later `write_texture`s run after the recorded clears and copies.

## Proposed change

Either write image pixels through the encoder too (a staging buffer and
`copy_buffer_to_texture`, or a `write_texture` into a staging texture then a recorded copy), so
every atlas operation is ordered by the encoder; or do the clear and the growth copy outside the
caller's encoder (their own encoder submitted at once, before returning), so they are ordered
before any later `write_texture`. Documenting the ordering on `upload_image` and `destroy_image`
would help either way.
