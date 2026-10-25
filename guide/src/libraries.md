# Component libraries

A component library is a core of colors, sizes and logic, written once over the
platform, with a crate per platform that exposes its views as that platform's own.
Simple is built this way, from the public API alone, and a library of your own stands
on the same footing.

```sh
skene new lib acme-ui web terminal
```

```text
acme-ui
├── .cargo         config.toml: the platform plain cargo builds, the web
├── Cargo.toml     the workspace
├── core           acme-ui: the palette, the sizes and the views over any platform
├── terminal       acme-ui-terminal: the views as the terminal's own
└── web            acme-ui-web: the views as the web's own
```

## The palette

A palette is a token set: a struct of tokens a `Theme` defines over a subtree. Each
token is a name with a definition, and a definition may answer conditions, the dark
scheme among them:

```rust
use skene_foundation::{Color, Definition, Media, Scalar, Token, TokenName, TokenRef, TokenSet};

/// Acme's colors.
#[derive(Debug, Clone, PartialEq)]
pub struct AcmePalette<N: Scalar> {
  /// A badge's surface.
  pub badge: Token<Color, N>,
}

impl<N: Scalar> AcmePalette<N> {
  /// A badge's surface.
  pub const BADGE: TokenRef<Color> = TokenRef::new("acme-badge");
}

impl<N: Scalar> Default for AcmePalette<N> {
  fn default() -> Self {
    Self {
      badge: Token::new(
        Self::BADGE.name().as_str(),
        Definition::from(Color::srgb(0.88, 0.91, 1.)).when(Media::Dark, Color::srgb(0.19, 0.18, 0.5)),
      ),
    }
  }
}

impl<N: Scalar, V: From<Definition<Color, N>>> TokenSet<V> for AcmePalette<N> {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.badge.define()]
  }
}
```

A view references a token by its `TokenRef` and emits only its name; the theme in force
supplies the value. Where no theme defines the palette, a view defines its default at
the root through `ThemedIn::define_root`.

## Views over any platform

A view in the core is generic over the platform `P`, and states what it needs of a
medium as a bound: `P::StyleExt: CornerRadius` rounds where the medium can, and a
library's own unit trait gives each medium's unit its sizes. The core's view is built
from the foundation's views, exactly as the built-ins are.

Each platform's crate wraps the core's view in a type of its own, and implements the
view's `*Base` trait, whose default methods carry every setter. Simple's web crate
wraps its core's card this way:

```rust
use skene_foundation::{View, Wraps};
use skene_simple::SimpleCardBase;
use skene_web::Active;

/// A ready-made card.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleCard(skene_simple::SimpleCard<Active>);

impl SimpleCardBase<Active> for SimpleCard {}
```

An app depends on the platform crates it shells for, and imports each one's prelude
beside its facade's.

## A platform's own property

A property only one platform draws is a prefixed setter beside the shared ones, from
the same macro line under the platform's `cfg`. The setter compiles in every build, so
an app calls it without a `cfg` of its own, and does nothing outside its platform's
build, where its backing field does not exist:

```rust
use skene_foundation::{Color, Condition, Realization, Value};

/// A gauge's own style.
pub struct GaugeStyle<N> {
  needle: Option<Value<Color, Condition<N>>>,
  #[cfg(skene_platform = "linux")]
  linux_glow: Option<Value<Color, Condition<N>>>,
}

/// Acme's gauge, over any platform.
pub struct Gauge<P: Realization> {
  style: GaugeStyle<P::Scalar>,
}

impl<P: Realization> Gauge<P> {
  const fn style_mut(&mut self) -> &mut GaugeStyle<P::Scalar> {
    &mut self.style
  }

  skene_foundation::properties! { style_mut, P;
    set_needle / derive_needle (color) => needle: Color, "the needle's color";
    [cfg(skene_platform = "linux")]
    set_linux_glow / derive_linux_glow (color) => linux_glow: Color,
      "the glow Linux draws around the needle";
  }
}
```

The prefix names the platform (`set_linux_*`, `set_ios_*`, `set_web_*`), and
`set_apple_*` with `[cfg(any(skene_platform = "ios", skene_platform = "macos"))]` names
what iOS and macOS share; there is no mobile or desktop prefix. A shared option that
every platform realizes in its own idiom is not prefixed: it is an ordinary property.
