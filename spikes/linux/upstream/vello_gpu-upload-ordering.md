# vello_gpu: an image uploaded after `destroy_image` in the same command encoder is erased at submit

Against: vello_gpu 0.3.0 (crates.io; vello_hybrid renamed), vello_common 0.3.0, wgpu 30.0.1,
Vulkan (Mesa lavapipe 25.2.8 here; the ordering is wgpu's, not the driver's). Not filed; ready
to file. It replaces Skene's earlier report against vello_hybrid 0.2.0
(`vello_hybrid-upload-ordering.md`), half of which 0.3.0 fixed.

## Summary

`Renderer::upload_image` writes an image's pixels with `queue.write_texture`
(`impl AtlasWriter for Pixmap`, `src/render/wgpu/mod.rs`), but `Renderer::destroy_image` clears
the freed slot with a render pass recorded into the caller's `CommandEncoder`
(`Renderer::clear_atlas_region`: `LoadOp::Load`, a scissor rectangle and the atlas clear
pipeline).

wgpu runs every `queue.write_texture` issued before a `queue.submit` ahead of the submitted
command buffers. So `destroy_image(a)` then `upload_image(b)` in one encoder, `b` taking `a`'s
slot, writes `b`'s pixels and then runs the clear recorded before it, which erases them. The image
draws as transparent, with no error.

The second case Skene reported against vello_hybrid 0.2.0, an upload after one that grows the
atlas, is fixed in 0.3.0: image atlases are now separate 2D textures (#1859), so a new atlas is
a new texture and no copy of the old layers is recorded after the write.

## Reproduction

`vello_gpu-upload-ordering-repro.rs` (manifest `vello_gpu-upload-ordering-repro.Cargo.toml`)
uploads 16x16 images, draws the last one at the top left of a 64x64 target in the same encoder,
submits and reads pixel (2, 2). Output on lavapipe (`vello_gpu-upload-ordering-repro.out`):

```text
adapter: "llvmpipe (LLVM 20.1.2, 256 bits)"
1. upload after destroy_image in one encoder: drawn [0, 0, 0, 0], expected [0, 255, 0, 255]
   with the encoder submitted after destroy_image: drawn [0, 255, 0, 255]
   a second atlas texture exists: (4096, 4096, 1)
2. upload after an upload that needed another atlas texture, in one encoder: drawn [0, 255, 0, 255], expected [0, 255, 0, 255]
   and after a third: drawn [0, 255, 0, 255], expected [0, 255, 0, 255]
```

## Expected

Atlas operations take effect in the order they are called, whatever encoder or queue path each
uses: an image uploaded after a slot is freed draws its pixels.

## Impact

Skene draws glyph masks as atlas images. A frame that starts the atlas over (destroying every
image) and then uploads the frame's glyphs loses those glyphs on the GPU path (under vello_hybrid
0.2.0 this was 183649 of 183649 dark pixels missing on one test page).

## Workaround in Skene

`gui-linux/src/gpu.rs`, `Gpu::submit_so_far`: the frame's encoder is submitted, and a fresh one
started, after the atlas is cleared to start over, so the frame's later `write_texture`s run after
the recorded clears. The submit Skene made after an upload that grew the atlas is gone with
vello_gpu 0.3.0.

## Proposed change

Either write image pixels through the encoder too (a staging buffer and
`copy_buffer_to_texture`, or a `write_texture` into a staging texture then a recorded copy), so
every atlas operation is ordered by the encoder; or clear the freed slot outside the caller's
encoder (its own encoder, submitted at once, or a `queue.write_texture` of zeros as the text
path's `clear_atlas_region` already does), so the clear is ordered before any later
`write_texture`. Documenting the ordering on `upload_image` and `destroy_image` would help either
way.
