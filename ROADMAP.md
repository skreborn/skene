# Roadmap

What remains, in the order to take it. The architecture is in `DESIGN.md`, the doctrine
in `AGENTS.md`; this file records neither, only the work that is open and the shape it
lands in. What has shipped is not listed: the code, `DESIGN.md` and the tests describe
it.

The reactive engine's baseline, `cargo bench -p skene-reactive --bench engine` on the
development machine (2026-09-05), against which a change to the engine is measured:
creating a thousand signals 78 µs; a signal read outside an effect 11 ns; a write
reaching ten effects 260 ns; a write through a chain of ten memos 393 ns.

## Open

One list, every item big and small, top first. An item is resolved by landing it, by
proving it unnecessary, or by recording it under the decisions at the end; it is never
left half-done in a code comment.

1. **Pictures beyond direct kitty transmission.** Shared memory or a file where the
    protocol's query over that medium shows the terminal shares the host, sparing the
    base64 for every change; unicode placeholders (`U=1`) through tmux, which passes no
    APC; sixel (DA1 attribute 4: xterm, foot, WezTerm, mlterm, Windows Terminal, VTE;
    not kitty, not Ghostty), palette-bound and without ids or z-order, so a change
    repaints the box whole; iTerm2's OSC 1337 last, for iTerm2, WezTerm and mintty.
2. **The asset system grows.** `skene.toml` declares assets today, served in dev,
    copied into static output and collected across the dependency graph; it gains
    further settings as they earn their place (Flutter's pubspec is the model). Font
    assets add generated metadata—per-face ascender, descender, line gap, cap height,
    read at build time—so the web can emulate text-box trimming where an engine lacks
    it and native and the terminal lay text out with the same numbers.
3. **Internationalization**: message catalogs, locale-aware formatting, reactive locale
    switching.
4. **Notifications and progress from the terminal**: OSC 99 (kitty, with ids and
    closing) or OSC 9 for a desktop notification, OSC 9;4 for the taskbar's progress
    (Windows Terminal, ConEmu, Ghostty); a facade seam beside the title, ignored where
    unsupported. Low value until an app asks.
5. **The terminal runs on Windows.** The `tty` module has a unix layer alone; the
    console layer (ConPTY's virtual terminal input, which helix found buggy with CJK
    and non-ASCII text, or the legacy console input records as the fallback) is
    unwritten, and no Windows machine is at hand; the item waits for one.
6. **The slider's standard parts on the web.** The web slider is drawn, so nothing
    waits on them, but when engines ship `::slider-fill-*` (csswg-drafts#12419) the
    native input's track can be restyled through them instead. Blocked on engines.
7. **The children-last rule as a lint.** Nothing enforces the ordering convention.
    Rust has no stable way to ship a lint that runs under plain `cargo clippy`; the
    ecosystem answer is a dylint crate at the cost of pinning an exact nightly.
8. **The native driver family.** The largest phase and deliberately the last: it
    realizes an already-complete vocabulary once, instead of chasing additions.
    - Shared machinery: taffy layout, vello paint, parley text, AccessKit (the
    accessibility tree a native app exposes, from the same reported semantics the web
    maps to ARIA), the frame
    clock running `skene_core::interpolate` as the terminal does, `Condition`
    resolution against window and pointer signals through the one resolution point.
    - Compositing premise: **the platform composites.** On iOS/Android/macOS the driver
    realizes the tree into the native view hierarchy—painted content in vello-backed
    layers that are views, embeds as ordinary native views—so custom paint and real
    platform controls interleave in z-order for free. The Linux leaf is winit + a
    single vello surface (no native control set exists to embed); Windows chooses
    per its composition APIs.
    - The `PlatformView` escape trait and per-leaf embed constructors
    (`ios::native_view`, …): real text fields, maps, web views.
    - View transitions: snapshot the outgoing tree into a layer and interpolate named
    pairs.
    - Off-thread delivery: the engine's scheduler wired to the event-loop proxy.
    - A loop-integrated timer (the one sanctioned exception to timers-are-userland).
    - Per-OS glue in leaves: IME, clipboard, menus, window management.
    - `skene-gui` resolving native triples to it; `skene build`/`run` for `ios`,
    `android` and the desktop leaves, landing under `dist/<target>`.

Watched, not scheduled:

- `aria-activedescendant` for the listbox, the menu and the picker's list: focus moves
    into the options today, the roving pattern, which assistive technology follows;
    the composite pattern keeps focus on the container and needs a node-valued report
    and stable per-node ids in the web driver. Worth it only if a screen reader
    stumbles on the roving form.
- A scrolling box as a tab stop in the terminal: the web gives one `tabindex="0"` so
    the keyboard reaches its content, as Chromium does; the terminal's scroll boxes
    are reached by the wheel and the page keys from a focused child alone.

- `ErrorBoundary` stays on probation: it exists only for errors with no render site of
    their own. If per-resource matching keeps covering real apps, delete it.
- A snapshot key is matched by a 127-bit hash of its value, so two values hashing alike
    share one scope and their state crosses on a restore; a collision check at every
    build was judged not worth its cost. If the hash ever proves short, the route is a
    keyed step carrying the value's postcard bytes and a type hash, at a
    `Serialize + DeserializeOwned` bound on every key and a heap step per keyed scope.
- The hybrid renderer also runs on WebGL2 (its `webgl` feature, no wgpu) for engines
    without WebGPU; not built, since the CPU path already covers them.

## The terminal's enhancements, as a whole

Every item above that asks the terminal for a capability—the further picture
transports and notifications—shares one rule with those that shipped (synchronized
output, pointer shapes, in-band resize, underlines, the keyboard flags, grapheme
clustering, the pixel mouse, text sizing, the graphics layer with the decoration on
it, the palette read back, the answering clipboard). A plain emulator is the floor and stays whole: every capability is asked for at launch,
confirmed by a parsed reply or ignored in silence, and degrades to the cell rendering
that exists today. None becomes a requirement, a type bound or a launch setting a
terminal must meet, and none is inferred from a name (XTVERSION, `TERM`), since a name
is not a capability. Multiplexers are part of the floor: tmux answers most queries
itself, passes graphics only as unicode placeholders and passes no pixel mouse, so
every item is proven through tmux and ssh as well as bare.

## Decided against, for the record

- Hot-patching: state-preserving reloads come from the snapshot instead.
- A custom devtools/inspector: native realizes into the platform view hierarchy, so the
  platform's own inspectors already work.
- Islands architecture: compiled no-JS styling already removes most reasons a static
  page ships wasm.
- LiveView-style server rendering: adjacent to the server-side non-goal.
- A webview desktop shell.
- A lazy split of the WebGPU renderer, measured 2026-09-15 on release builds after
  `wasm-bindgen`: an app that paints nothing (`counter`) ships 911 KB carrying 10.6 KB
  of wgpu code and no shader text, since the linker drops what no box reaches; an app
  that paints (`gallery`) ships 1.9 MB carrying 135 KB for the GPU path beside the
  271 KB CPU renderer it needs either way. Nothing worth a split.
- Rendering the whole terminal frame as one image: the text would then be ours to
  rasterize, and the terminal's font, selection, copy and accessibility would go with
  it, a poor GUI in place of a good terminal.
- Any terminal enhancement gated by a terminal's name.
- Inferring a terminal's depth: a terminal answering no DECRQSS paints at 16 colors,
  Terminal.app among them; the cost of inferring nothing, taken knowingly. Client hints
  are likewise Chromium-only (`Accept-CH`/`Critical-CH`), and other engines get the
  documented defaults.
- A vertical fractional edge in the terminal: only the lower block glyphs exist, so a
  top-anchored fill would need upper ones; a vertical fill ends on a whole cell.
- Replacing `IntoChildren`, `IntoChildOf` and `IntoView` with std traits, checked
  against the compiler: a blanket `impl<I: IntoIterator<Item = V>> IntoChildren for I`
  conflicts with the tuple impls, and the same conflict appears with `From`; tuples are
  not optional because children are heterogeneous. `Into<AnyView>` cannot replace
  `IntoView` either, since `AnyView` is itself a view.
- A collision check on snapshot keys at every build (see the watched item above).
- A `&'static str` signal restoring: it is `opaque`, since it cannot be deserialized
  owned.
- A `HashMap` store field iterating in a stable order: its entries come in its
  iteration order; an ordered list wants a `BTreeMap` or a keyed `Vec`.
