# text-parity

The shared harness for revisiting how Skene draws text everywhere but the web. One corpus is
laid out and drawn three ways, and every pair is compared:

- **A, platform engines** (the current decision): TextKit 2, `android.text` over Minikin,
  DirectWrite, each spike filling the [record schema](#the-record-schema) from its own engine.
- **B, Parley everywhere**: Parley's layout and shaping over fontique's fonts, glyphs drawn by
  vello_cpu. This crate is B.
- **C, Parley layout, platform glyphs**: B's glyph runs (`--glyphs`), drawn by the platform's
  rasterizer (CoreText, DirectWrite, `Canvas.drawGlyphs` or FreeType), which is Chromium's model.

The harness compares engine against engine: lines, metrics, fonts and pixels. It has no view of
input methods, accessibility or selection chrome, which belong to the platform's protocols
rather than its engine (the prior memo's "Not engine gains").

```
corpus.json          the corpus, read by every platform
src/corpus.rs        cases and variants
src/record.rs        the record schema (Rust types, serde)
src/fonts.rs         fontique's system collection, or an explicit FontSource
src/layout.rs        Parley's layout, marks (soft-hyphen hyphen, ellipsis), records, glyph runs
src/emoji.rs         emoji-presentation clusters, asked of the emoji family first
src/render.rs        vello_cpu drawing to PNG by the image convention
src/image.rs         PNG reading and pixel metrics
src/compare.rs       two record sets compared, as markdown and JSON
src/bench.rs         timings
src/run.rs           one Parley run, as the CLI and an embedding spike start it
src/main.rs          the `text-parity` CLI
native/pango/        a stand-in native engine (Pango) that validates the harness on Linux
```

## Running it

The crate is its own workspace pinned to Rust 1.99.0. Build with `--release` for timings (each
set records `build` in its engine settings).

```sh
cargo run --release -- corpus --expanded > expanded.json        # every variant spelled out
cargo run --release -- parley --out out/parley --glyphs         # B: records.json + png/
cargo run --release -- validate out/native/records.json         # a native set against the schema
cargo run --release -- compare out/native/records.json out/parley/records.json \
  --out report.md --json compare.json --diff diff/ --scale 2   # A against B
cargo run --release -- restamp out/parley/records.json --images out/c --rasterizer coretext
cargo run --release -- compare out/native/records.json out/c/records.json --out a-vs-c.md
```

`parley` takes `--scales 1,2,3`, `--fonts FONTS.json` (an [explicit source](#explicit-fonts)),
`--glyphs` (glyph runs in the records), `--only TEXT` (variants whose id contains it), `--hint`
(vello's vertical hinting), `--quantize` (Parley's pixel-rounded line metrics), `--features`
(OpenType features on the whole text in `font-feature-settings` form, `'"chws" 1'`), `--plain-emoji`
(Parley's own emoji fallback, see below), `--repeats 5`, `--labels 1000`, `--no-images`,
`--os-version`, `--device`. `compare` takes `--threshold 64` (a pixel differs where a channel
differs by more), `--tolerance 0.5` (a width or height agrees within it, in logical pixels) and
`--scale` (the scale the per-variant table shows; the largest both sets drew by default).

From Rust, `text_parity::run::parley(&ParleyRun { .. })` is the `parley` command, and
`run::parley_json(config)` takes the same `ParleyRun` as JSON (every field optional:
`{"out": "/data/.../parley", "scales": [1, 2.625], "fonts": {...}, "glyphs": true}`) for a
wrapper over JNI or a C ABI.

## The corpus

`corpus.json` holds `version` and `cases`. Every field of a case is always present:

| field | type | meaning |
|---|---|---|
| `id` | string | stable, file-safe (`[a-z0-9-]`) |
| `note` | string | what the case checks |
| `tags` | [string] | the groups the comparison totals by |
| `text` | string | every character significant; invisible ones are `\u` escapes |
| `locale` | string | BCP 47 (`ja-JP`, `zh-Hant-TW`) |
| `family` | string | `system` (the UI font), `serif`, `monospace`, or a family name |
| `sizes` | [number] | font sizes in logical pixels per em: points on Apple, dp on Android, DIPs on Windows |
| `widths` | [number or null] | box widths in logical pixels; `null` is unconstrained (max-content) |
| `weight` | number | 100 to 900 |
| `italic` | bool | |
| `alignment` | `start` `center` `end` `justify` | `start` follows the paragraph's direction (TextKit's `.natural` as content direction, CSS `start`) |
| `direction` | `auto` `ltr` `rtl` | `auto` takes the first strong character |
| `max_lines` | number or null | lines shown; the rest is cut |
| `ellipsis` | bool | a cut text ends its last line with an ellipsis, cut at a grapheme cluster |
| `hyphenation` | `none` `manual` `auto` | `auto` turns on the engine's hyphenation (TextKit `hyphenationFactor`, Android `HYPHENATION_FREQUENCY_NORMAL`); a soft hyphen breaks and draws a hyphen in every mode |
| `line_break` | `normal` `strict` `loose` `anywhere` | CSS `line-break`; Android's `LineBreakConfig` style (API 33) |
| `word_break` | `normal` `break-all` `keep-all` `phrase` | CSS `word-break`; `keep-all` is TextKit's `.hangulWordPriority` for Korean, `phrase` Android's `LINE_BREAK_WORD_STYLE_PHRASE` (API 33) and CSS `auto-phrase` |

A **variant** is a case at one size and one width, enumerated with each case's sizes outermost
and its widths innermost. Its id is `case/size/width` (`latin-kerning/17/max`, `max` for `null`,
a whole number without decimals) and its file stem `case_size_width`. `corpus --expanded`
prints every variant with its case's fields inlined, so an engine needs no corpus logic of its
own. Version 1 has 66 cases and 131 variants covering: Latin kerning, ligatures, digits and SF's
contextual punctuation; a word too long for its line; a URL; soft hyphens; automatic
hyphenation candidates in English and German; no-break spaces; Greek and Cyrillic; Vietnamese
and stacked combining marks; one Han string under zh-Hans, zh-Hant, ja and ko; Chinese,
Japanese (kinsoku, strict) and Korean paragraphs, Japanese phrase breaking, Korean keep-all;
Arabic with and without harakat, Hebrew, bidi with numbers, LTR and RTL bases; Thai, Khmer, Lao
and Burmese; Devanagari, Bengali, Tamil; ZWJ families, skin tones, regional and subdivision
flags, keycaps and VS15/VS16, Unicode 15, 15.1 and 16 emoji; a mixed-script paragraph through
the whole fallback chain; the size ladder 11, 13, 17, 20, 34 and 64 in Latin, CJK and Arabic;
bold, light, italic and synthesized bold-italic CJK; centre, end, justified and centred RTL;
two-line, one-line, CJK and RTL ellipses and a cut without one; and the guaranteed core (blanks
kept, leading blanks, newlines and an empty line, an empty text, a trailing newline, tabs).

## The record schema

An engine writes one `records.json` (a record set) and its images. Coordinates are logical
pixels, x to the right and y down from the layout's top left (the first line box's top). A text
range is `[start, end)` in UTF-8 bytes (`utf8`) or UTF-16 code units (`utf16`) of the case's
text; a native record gives either and the harness derives the other. Optional fields may be
left out (an optional number, string or object may also be `null`). Unknown fields are ignored.

```json
{
  "schema": 1,
  "corpus_version": 1,
  "engine": {"name": "textkit2", "version": "iOS 26.2", "rasterizer": "coregraphics",
             "settings": {"smoothing": "default"}},
  "platform": {"os": "ios", "os_version": "26.2", "device": "iPhone Air simulator"},
  "scales": [1, 2, 3],
  "timings": {"font_collection_ms": 4.1, "first_layout_ms": 0.9,
              "labels": {"count": 1000, "size": 13, "width": 150, "build_ms": 30.2,
                         "layout_ms": 80.5, "relayout_ms": 74.0, "draw_ms": 141.0}},
  "records": [{
    "variant": "latin-kerning/17/120",
    "lines": [{"utf16": [0, 13], "x": 0, "width": 101.2, "top": 0, "height": 20.3,
               "baseline": 16.2, "ascent": 16.2, "descent": 4.1, "leading": 0,
               "hyphen": false, "ellipsis": false}],
    "width": 101.2, "height": 101.5, "truncated": false,
    "runs": [{"utf16": [0, 13], "line": 0, "family": "System Font",
              "postscript": ".SFUI-Regular", "size": 17, "weight": 400, "italic": false,
              "synthetic_bold": false, "synthetic_italic": false, "rtl": false}],
    "clusters": [{"utf16": [0, 1], "line": 0, "x": 0, "advance": 11.3}],
    "intrinsic": {"min": 58.6, "max": 380.4},
    "timings": {"cold_ms": 0.31, "warm_ms": 0.08},
    "images": {"1": "png/latin-kerning_17_120@1x.png", "2": "png/latin-kerning_17_120@2x.png"}
  }]
}
```

**Record set**

| field | | meaning |
|---|---|---|
| `schema` | required | `1` |
| `corpus_version` | required | the corpus's `version` |
| `engine` | required | `name`, `version`, `rasterizer`, and `settings` (string to string): anything that matters to reading the numbers (hinting, smoothing, the build) |
| `platform` | required | `os` (`ios`, `macos`, `android`, `windows`, `linux`), `os_version`, `device` |
| `scales` | optional | the scales images were drawn at |
| `timings` | optional | `font_collection_ms` (enumerating the system's fonts, or the first font query), `first_layout_ms` (`Hello` at 17 right after), `labels` |
| `timings.labels` | optional | `count` labels from the corpus's texts in turn (the empty one left out) at `size` 13 wrapped at `width` 150: `build_ms` (making each paragraph, shaping where the engine shapes then), `layout_ms`, `relayout_ms` (all again at width 140), `draw_ms` (all into one image at scale 1, eight columns), `glyphs`, `lines` |
| `records` | required | one per variant; `validate` reports each one missing, `compare` lists them and goes on |

**Record**

| field | | meaning |
|---|---|---|
| `variant` | required | the variant's id |
| `lines` | required | top to bottom; an empty text has one |
| `width` | required | the widest line's `width` |
| `height` | required | the last kept line box's bottom |
| `truncated` | optional | whether `max_lines` cut text off |
| `runs` | optional | glyph runs, each in one font, line by line |
| `clusters` | optional | clusters, line by line; compared where both sets give the same range |
| `intrinsic` | optional | `min` (min-content: every break opportunity taken) and `max` (max-content) widths |
| `timings` | optional | `cold_ms` (the variant's first layout in the run) and `warm_ms` (the median of later ones, each from the text again) |
| `images` | optional | scale (`"2"`, `"2.625"`) to a PNG path relative to `records.json` |
| `glyph_runs` | optional | Parley's glyph runs, for option C |
| `notes` | optional | what the engine could not honour |

**Line**

| field | | meaning |
|---|---|---|
| `utf8` / `utf16` | one required | the text the line holds, trailing blanks and the line break included; only the starts are compared |
| `x` | required | the line's left edge as drawn |
| `width` | required | from the line's start edge (left in LTR, right in RTL) to the far edge of its last visible cluster: trailing blanks and the line break left out, leading blanks and a drawn hyphen or ellipsis in. Android's `getLineMax`, DirectWrite's `widthIncludingTrailingWhitespace` less the trailing whitespace, TextKit's `typographicBounds` width less `CTLineGetTrailingWhitespaceWidth` |
| `top`, `height` | required | the line box |
| `baseline` | required | from the layout's top |
| `ascent`, `descent` | required | positive distances from the baseline to the line's ascent and descent |
| `leading` | optional | what the line box adds beyond ascent and descent |
| `hyphen` | optional | the line ends in a drawn hyphen |
| `ellipsis` | optional | the line ends in a drawn ellipsis |

**Run**

| field | | meaning |
|---|---|---|
| `utf8` / `utf16` | one required | the text the run draws |
| `line` | required | its line |
| `family`, `postscript`, `path`, `index` | one required | the font as the platform names it (`CTFontCopyFamilyName`, `CTFontCopyPostScriptName`; `IDWriteFontFace3::GetInformationalStrings` with `DWRITE_INFORMATIONAL_STRING_POSTSCRIPT_NAME`; Android's `Font.getFile()` and `getTtcIndex()` from `TextRunShaper`, API 31). Where only `path` is given, the harness reads the names from the file |
| `size` | required | the size it is drawn at |
| `weight`, `italic` | optional | the face's |
| `synthetic_bold`, `synthetic_italic`, `rtl` | optional | |

**Cluster**: a range, `line`, `x` (its left edge) and `advance`.

**Glyph run** (`glyph_runs`, Parley's only): `line`; `font` (`family`, `postscript`, `path`
where fontique loaded it from a file, `index`, `data_len`); `size`; `coords` (normalized
variation coordinates, F2Dot14 bits, in the font's axis order); `variations` (the user-space
`tag` and `value` they come from, `wght` 700); `embolden`; `skew` (degrees of synthetic slant);
`glyphs` (`id`, `x` the origin from the layout's left, `y` the baseline from its top, `cluster`
the byte its cluster starts at). In Rust, `Laid::glyph_runs()` gives the same runs with the
font's bytes (`parley::FontData`) for a rasterizer that opens fonts from memory.

### The image convention

An image covers the layout box plus an 8-pixel margin: the box is the variant's width (the
record's own `width` where unconstrained) by the record's `height`. At scale `s` it is
`round((ceil(box_width) + 16) * s)` by `round((ceil(height) + 16) * s)` pixels, the layout's top
left at `(8 * s, 8 * s)`. Text is black (`#000`) on white, drawn unclipped so ink past the box
(Thai marks, emoji) lands in the margin; colour glyphs keep their colours. A transparent
background is composited over white. `validate` checks every image's size.

### Explicit fonts

A `FontSource` (`--fonts FONTS.json`, or `ParleyRun.fonts`) names the collection when a
platform's list is better taken from the platform: Android's `ASystemFontIterator`
(`AFont_getFontFilePath`, `AFont_getCollectionIndex`, `AFont_getWeight`, `AFont_isItalic`,
`AFont_getLocale`, `AFont_getAxisTag`/`Value`) or `SystemFonts.getAvailableFonts()` see updatable
and OEM fonts, which fontique's Android backend (a reader of `/system/etc/fonts.xml`) does not.

```json
{
  "system": false,
  "files": [
    {"path": "/system/fonts/Roboto-Regular.ttf", "family": "Roboto", "weight": 400,
     "axes": [{"tag": "wght", "value": 400}]},
    {"path": "/system/fonts/NotoSansCJK-Regular.ttc", "family": "Noto Sans CJK"},
    {"path": "/system/fonts/NotoColorEmoji.ttf"}
  ],
  "generics": [{"generic": "system-ui", "families": ["Roboto"]},
               {"generic": "emoji", "families": ["Noto Color Emoji"]}],
  "fallbacks": [{"script": "Hani", "locale": "ja", "families": ["Noto Sans CJK"]},
                {"script": "Hani", "families": ["Noto Sans CJK"]}]
}
```

`system` keeps fontique's backend as well; a file is read whole and registered with every face
it holds; `generics` and `fallbacks` replace the backend's for each generic, and for each script
and locale, named. A family a generic or fallback names must be in the collection.

## The comparison

`compare A B` normalizes both sets (each range in both units, names from font files) and, for
every variant both have, reports (deltas are `b - a`):

- **breaks**: identical when every line starts at the same byte; the lines' texts where not;
- **lines** counted and **cut** (`truncated`) in each;
- **widths**: per line, over the lines both have, and the largest absolute delta;
  **height** and the **first baseline**, the largest **baseline**, **line-height** and
  **ascent or descent** deltas; **intrinsic** min- and max-content deltas;
- **fonts**: for each visible character both drew, whether its run's family is the same
  (ignoring case, spaces, hyphens and a trailing face name) and, where both name one, the same
  PostScript font; each distinct disagreement listed;
- **clusters**: advances of clusters with the same range, mean and largest absolute delta;
- **pixels** at each scale both drew, both images padded with white to the larger extent: the
  mean absolute difference (each pixel's largest channel difference, out of 255), the share of
  pixels over the threshold, the largest difference, each image's ink box (a pixel is ink where a
  channel is below `255 - threshold`) and their edge deltas in pixels, ink IoU, and a mean SSIM of
  luminance over 8 by 8 windows at a stride of 4;
- **timings**: each set's cold and warm layout per variant, and the run-wide timings.

The markdown has totals overall and per tag, then a table per case with its break and font
disagreements and both sets' notes. `--json` writes everything, `--diff DIR` a difference image
per variant at the table's scale: dark where both have ink, **red where only `a` does, blue where
only `b` does**. A vertical offset moves every glyph, so read the pixel columns beside Δbaseline:
a whole-line shift with matching widths is a metrics difference, not a drawing one.

## How each platform uses it

Every spike reads `corpus.json` (or `corpus --expanded`), lays each variant out with its own
engine, draws it by the image convention and writes `records.json`; then runs B on the same
device and compares. B must run where A runs (the simulator, the emulator, the Windows runner),
so that fontique sees the same fonts.

- **macOS** (Rust, objc2): depend on `text-parity = { path = "../../text-parity" }`; build each
  `Record` from TextKit 2 (line fragments' `typographicBounds` and `glyphOrigin`, runs from
  `CTLineGetGlyphRuns` of each fragment's attributed string), draw through the spike's flipped
  `NSView` into a `CGBitmapContext`, call `run::parley` in the same process and
  `compare::compare`.
- **iOS** (Swift, XCTest on a simulator): bundle `corpus.json`; write records and PNGs from
  `TextNodeView`'s TextKit 2 layout. For B, a three-line wrapper crate
  (`crate-type = ["staticlib"]`, an `extern "C"` function over `run::parley_json`) built with
  `cargo build --release --target aarch64-apple-ios-sim` and linked into the test target; the
  comparison runs on the macOS host over the pulled directories.
- **Android** (Kotlin, instrumented test; Rust through cargo-ndk): the spike's Rust library
  depends on this crate and exposes a JNI function over `run::parley_json`, given
  `filesDir` and a `FontSource` built from `SystemFonts.getAvailableFonts()` (or `"system":
  true` for fontique's own reader). Kotlin fills the native record from `StaticLayout`
  (`getLineStart`, `getLineMax`, `getLineTop`/`Bottom`/`Baseline`/`Ascent`/`Descent`), fonts
  per run from `TextRunShaper` (API 31) or by path, and draws with `Canvas` into a `Bitmap`
  compressed to PNG; `adb pull` the directory, compare on the host.
- **Windows** (windows-rs): depend on this crate; fill records from `IDWriteTextLayout`
  (`GetLineMetrics`, `GetClusterMetrics`, a collecting `IDWriteTextRenderer` for runs and fonts,
  `IDWriteLocalFontFileLoader::GetFilePathFromKey` for files), draw with Direct2D into WIC.
- **Linux**: the drawn prototype's engine (`spikes/linux/parley`: fontconfig fallback, FreeType
  masks) can fill a record set as "the platform", against this crate's fontique-only Parley.

**Option C.** `parley --glyphs` writes each record's `glyph_runs`. The platform draws them into
`DIR/png/<stem>@<scale>x.png` by the image convention, and `restamp PARLEY.json --images DIR
--rasterizer NAME` writes `DIR/records.json`: Parley's records over the platform's images, so
`compare` gives A against C (layout and drawing) and B against C (drawing alone). Rasterizers:
CoreText `CTFontDrawGlyphs` (the font from `path` or the bytes through
`CGDataProvider`/`CGFont`, variations through `kCTFontVariationAttribute`, a slant through the
text matrix, emboldening through a fill-and-stroke); DirectWrite `CreateGlyphRunAnalysis` or
Direct2D `DrawGlyphRun` (a font face from the file, `DWRITE_FONT_SIMULATIONS_BOLD`/`OBLIQUE`,
advances from consecutive `x`); Android `Canvas.drawGlyphs` (API 31: `Font.Builder(File)`,
`setTtcIndex`, `setFontVariationSettings` from `variations`); FreeType on Linux (the prototype's
`MaskPainter`).

## Cross-compiling

```sh
cargo check --all-targets --target aarch64-apple-darwin
cargo check --all-targets --target aarch64-apple-ios-sim
cargo check --all-targets --target aarch64-linux-android     # or cargo ndk -t arm64-v8a build
cargo check --all-targets --target x86_64-linux-android
cargo check --all-targets --target x86_64-pc-windows-msvc
```

The graph has no C build script for any of these targets: fontique's backends are objc2
(Apple), windows-rs (Windows), and roxmltree (Android); only the Linux build links fontconfig
(`yeslogic-fontconfig-sys`). Linking needs each platform's SDK (Xcode, the NDK, MSVC).

## Validating the harness on Linux

`native/pango` is a stand-in native engine: Pango 1.52 through gtk-rs fills the schema (its own
workspace; it depends on this crate for the types). It proves the tool, not a result:

```sh
cargo run --release -- parley --out out/parley --glyphs
cargo run --release --manifest-path native/pango/Cargo.toml -- out/pango
cargo run --release -- validate out/pango/records.json
cargo run --release -- compare out/pango/records.json out/parley/records.json \
  --out report.md --diff diff/ --scale 2
```

What building the harness found about Parley 0.12 and vello_cpu 0.3 (on this host, 2026-10-16):

- Parley asks the emoji family **after** a run's own families for an emoji cluster
  (`parley::shape::FontSelector::select_font`), so a text font that covers `#` and U+20E3, or
  `❤` before U+FE0F, draws a keycap or a VS16 sequence in monochrome. The harness asks the
  emoji family first for every emoji-presentation cluster (UTS #51, the drawn prototype's rule);
  `--plain-emoji` measures Parley alone.
- glifo 0.4's `glyph_transform` does not slant a run under a scaled transform (a 1.5x transform
  drew it upright; at 1x it sheared wrongly). The harness shears the scene about the run's
  baseline instead, and grows a synthetic bold by Skia's ratio (1/24 of the size at 9 to 1/32 at
  36, half per side).
- Parley hangs trailing blanks only on lines it breaks; a paragraph's last line keeps them in its
  advance. The harness measures a line's width from its clusters.
- Parley 0.12 has no automatic hyphenation, no phrase breaking and no ellipsis. The harness draws
  a hyphen at a soft hyphen's break, and cuts the last line of an ellipsized text at a grapheme
  (shaping the tail again with `line-break: anywhere`, the lines above kept); a record's `notes`
  name what it could not honour.
