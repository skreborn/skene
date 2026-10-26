# Styling and themes

A property's value is one of five kinds, and its type says which: a fixed value, a
design token, an expression, a value derived from signals, or any of those under a
condition.

```rust
use skene_web::prelude::*;

fn kinds(selected: Signal<bool>) -> impl View {
  Container::new()
    .set_padding(Inset::all(12))
    .set_background(
      Color::srgb(0.95, 0.95, 0.97).when(State::Hovered, Color::srgb(0.9, 0.9, 0.95)),
    )
    .derive_border_color(move || {
      if selected.get() {
        Color::srgb(0.2, 0.3, 0.8)
      }
      else {
        Color::srgb(0.85, 0.85, 0.9)
      }
    })
    .set_border_width(Inset::all(1))
    .set_child(Text::new("Hover me"))
}
```

A length is a plain number, an `i32` or an `f32`: `set_width(16)` and `set_width(16.5)`
both set a width, and every constructor measured in lengths takes either, in the
order of its fields.

```rust
use skene_web::prelude::*;

fn lengths() -> impl View {
  Row::new()
    .set_spacing(Spacing::new(8, -4))
    .set_padding(Inset::new(0, 8, 0, 8))
    .set_children((
      Container::new().set_width(16).set_child(Text::new("Whole")),
      Container::new().set_width(16.5).set_child(Text::new("Fractional")),
    ))
}
```

`Inset::new` takes the start, the top, the end and the bottom; `Inset::all`,
`Inset::symmetric`, `Inset::horizontal` and `Inset::vertical` the common shapes.

On the web the fixed values, tokens, expressions and conditions compile to CSS, so the
server's page is styled, responsive and hoverable with no script. A derived value is
for state, never for the environment: a width that answers the viewport is a condition,
and a color that answers a selection is derived.

## Tokens

A design token is a named slot a theme defines and every view beneath it reads. A
`TokenRef` names one, and any setter takes it in place of a value:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn card_title() -> impl View {
  Text::new("Title")
    .set_color(SimplePalette::FOREGROUND)
    .set_font_weight(600.)
}
```

A palette is a set of tokens with their definitions, and `Theme` defines it over a
subtree. Changing the palette rewrites the definitions in place: nothing beneath the
theme rebuilds, and the cost is the number of tokens, never the number of views.

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn themed(accent: Signal<Color>) -> impl View {
  Theme::derive(move || SimplePalette::default().set_ring(accent.get())).set_child_builder(
    || {
      SimpleCard::new().set_child(Column::new().set_spacing(12).set_children((
        Text::new("Themed").set_color(SimplePalette::FOREGROUND),
        SimpleButton::new("Go"),
      )))
    },
  )
}
```

A theme inside another redefines the tokens for its own subtree alone.

### The platform's colors

`SystemColors` names the platform's colors, as CSS's system colors are: the ink, the
window, a control's fill and border, the accent and what sits on it, a track, a knob, a
field and a selection, with a hover and a pressed variant where a control needs one. The
platform defines them at the root, so a view painted in them looks like the platform it
runs on: the web defines them as CSS's system colors, so a page follows the reader's
platform and color scheme with no script, and the terminal defines them from the colors
it reports. A set of your own that defines one of the names redefines it beneath its
theme as any token, and everything beneath that reads it follows:

```rust
use skene_web::{Definition, TokenName, prelude::*};

/// An accent of the app's own, in place of the platform's.
#[derive(Debug, Clone, PartialEq)]
struct BrandAccent {
  accent: Token<Color>,
}

impl<V: From<Definition<Color>> + 'static> TokenSet<V> for BrandAccent {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.accent.define()]
  }
}

fn branded() -> impl View {
  let accent = Token::new(
    SystemColors::ACCENT.name().as_str(),
    Color::srgb(1., 0.58, 0.),
  );

  Theme::new(BrandAccent { accent }).set_child_builder(|| {
    Container::new()
      .set_background(SystemColors::ACCENT)
      .set_child(Text::new("Ours").set_color(SystemColors::ON_ACCENT))
  })
}
```

A theme of `SystemColors` itself replaces every one of them with the set's definitions.

## Light and dark

Light and dark are one condition, `Media::Dark`, which a token's definition answers
per token like any other condition:

```rust
use skene_simple_web::prelude::*;
use skene_web::prelude::*;

fn palette() -> SimplePalette {
  SimplePalette::default().set_card(
    Definition::from(Color::srgb(1., 0.98, 0.95)).when(Media::Dark, Color::srgb(0.12, 0.1, 0.08)),
  )
}
```

The scheme in force follows the platform's preference until the app chooses one:
`set_scheme(Some(ColorScheme::Dark))` forces it, `set_scheme(None)` returns to the
platform, and `derive_scheme` follows a signal. Choosing a scheme rebuilds nothing.
`chosen_scheme()` reads the scheme a reader chose on an earlier visit, so a setting
starts where the page already is.

## Motion

A transition animates a property whenever its value changes, from any source: a
condition flipping, a token redefined, a derived value written.

```rust
use skene_web::prelude::*;

fn fading() -> impl View {
  Container::new()
    .set_background(Color::srgb(0.9, 0.9, 0.95).when(State::Hovered, Color::srgb(0.8, 0.8, 0.9)))
    .add_transition(Transition::new(Property::Background, 0.15))
    .set_child(Text::new("Hover me"))
}
```

A reader who prefers reduced motion sees the change as a cut, unless the transition
says otherwise.
