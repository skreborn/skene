# The terminal

A terminal app measures in cells. A length is a whole number of them, a fraction
rounding to the nearest (`set_width(1.5)` is two cells) while a font size keeps its
fraction of a cell; text is a color, a bold flag and a size that scales the cell, and a
border is a cell of box-drawing glyphs. What the terminal cannot draw does not exist to
call.

```rust
use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

fn status(saved: Signal<bool>) -> impl View {
  Row::new().set_spacing(2).set_children((
    Text::new("Draft").set_bold(true),
    Text::derive(move || if saved.get() { "saved" } else { "unsaved" }.to_owned())
      .set_color(SimplePalette::MUTED_FOREGROUND),
  ))
}
```

The Simple library draws its marks there in glyphs: a checkbox shows `[x]` or `[ ]`, a
radio `(x)` or `( )` and a picker's face `▾`, each from `SimpleGlyphs`, a token set of
the terminal's Simple crate that a theme redefines beneath it like any palette:

```rust
use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

fn consent(agreed: Signal<bool>) -> impl View {
  Theme::new(SimpleGlyphs::default().set_checkbox("[*]", "[ ]")).set_child_builder(move || {
    SimpleCheckbox::new()
      .set_label("I agree")
      .bind_checked(agreed)
  })
}
```

The terminal is its facade's one platform, so `Platform::CURRENT` is
`Platform::Terminal` in every build an app's terminal shell compiles in. Its own
vocabulary lives in `skene_terminal::platform::terminal`: `CellPainter`, the escape
trait a low-level view paints the terminal's cells through, and `install`, `mount` and
`uninstall`, the seams an in-process test builds its views through (see
[Testing](testing.md)), while `launch` stays at the root.

## Behaving as a browser does

Wherever the terminal can express it, an interaction takes the shape it has in a
browser: `Tab` moves focus, a click lands on press and release over one target, a drag
selects text, a double click a word, and the controls own their keys. `Ctrl-C`, `Ctrl-X`
and `Ctrl-V` copy, cut and paste, and `Ctrl-Q` quits.

## Capabilities

A plain emulator is the floor, and every capability beyond it is asked for at launch and
used only once the terminal's reply proves it: truecolor over sixteen colors, text drawn
at sizes other than one cell, pictures through the kitty graphics protocol, the kitty
keyboard protocol's key releases, the pointer in pixels. Nothing an app writes changes
when one arrives, and nothing is inferred from a terminal's name.

## Launching

`#[main]` launches with every default. A binary of its own chooses what the app offers
through `Launch`: which clipboard operations exist, and whether the app takes the
mouse at all.

```rust,no_run
use skene_terminal::{Clipboard, Launch, prelude::*};

fn app() -> impl View {
  Text::new("Copy me")
}

fn main() {
  Launch::new()
    .set_clipboard(Clipboard::COPY)
    .set_mouse(false)
    .run(app);
}
```

With the mouse taken, `Alt-M` hands it back to the terminal for one native selection,
and any key takes it again.
