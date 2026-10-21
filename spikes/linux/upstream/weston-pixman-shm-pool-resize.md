# weston: the pixman renderer crashes after a client grows a wl_shm_pool it shows a buffer of

Against: weston 13.0.0 (Ubuntu 24.04's), `--backend=x11 --renderer=pixman`, libpixman 0.42.2,
libwayland-server 1.22. Not filed; ready to file.

## Summary

A client shows a buffer of a `wl_shm_pool` on one surface, then grows that pool
(`wl_shm_pool.resize`, which the protocol allows at any time) to make a buffer for another
surface and commits it. weston segfaults inside pixman on its next repaint, reading the first
buffer through the pool's old mapping: libwayland-server remaps the pool on the resize
(`mremap` with `MREMAP_MAYMOVE`), and the pixman renderer's image of the shown buffer still
points into the mapping that moved. The kernel log reads:

```text
weston[6421]: segfault at 7f38b4fcd000 ip 00007f38b76da2f5 sp 00007ffdd15af9e0 error 4 in libpixman-1.so.0.42.2[802f5,7f38b7664000+8d000]
```

libwayland-server defers the remap while the compositor holds a reference on the pool
(`wl_shm_buffer_ref_pool`), but libweston-13 does not take one: `nm -D` of
`libweston-13.so.0` lists no `wl_shm_buffer_ref_pool` (VERIFIED). The GL renderer copies a
buffer into a texture as it is attached, so it is not affected (JUDGMENT, from reading).

## Reproduction

Skene's Linux prototype (`spikes/linux`) before its change to one pool per buffer: the window
and its popups took buffers from one smithay-client-toolkit `SlotPool`, which grows in place.
The driver test `lazy::a_pickers_list_is_as_tall_as_its_list_box_lets_it_be`
(`conformance-linux/tests/it`) on the runner's weston leg opens a picker whose list is 600
pixels tall, `WAYLAND_DEBUG=client`:

```text
-> wl_shm_pool@11.create_buffer(wl_buffer@26, 2906112, 464, 600, 1856, 0)   # the popup's first buffer
-> wl_surface@27.attach(wl_buffer@26, 0, 0)
-> wl_surface@27.commit()
-> wl_surface@8.attach(wl_buffer@21, 0, 0)                                 # the window shows a buffer of the same pool
-> wl_surface@8.commit()
-> wl_shm_pool@11.resize(9830400)                                          # grown for the popup's second buffer
-> wl_shm_pool@11.create_buffer(wl_buffer@31, 4019712, 464, 600, 1856, 0)
-> wl_surface@27.attach(wl_buffer@31, 0, 0)
-> wl_surface@27.commit()
Io error: Broken pipe (os error 32)                                        # weston is gone
```

3 of 3 runs crashed weston; sway, mutter and KWin on the same test pass.

## Expected

A pool grown while one of its buffers is shown keeps the shown buffer readable: the pixman
renderer takes a reference on the pool (`wl_shm_buffer_ref_pool`) for as long as an image of
one of its buffers lives, and drops it (`wl_shm_pool_unref`) with the image, so
libwayland-server finishes the resize only once nothing reads the old mapping.

## Workaround in Skene

`gui-linux/src/wayland.rs` (`Buffers`): each buffer lives in a pool of its own, made to its
size and never resized; a surface takes turns with two of them.
