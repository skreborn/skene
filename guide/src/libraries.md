# Component libraries

A component library is a core of colors, sizes and logic, naming no platform, with a
crate per facade holding its presets: functions returning the facade's built-in views,
styled from the core. Simple is built from the public API alone, and a library of your
own stands on the same footing.

```sh
skene new lib acme-ui web terminal
```

```text
acme-ui
├── .cargo         config.toml: the platform plain cargo builds, the web
├── Cargo.toml     the workspace
├── core           acme-ui: the palette and the sizes
├── terminal       acme-ui-terminal: the presets of the terminal's views
└── web            acme-ui-web: the presets of the web's views
```

## The palette

A palette is a token set: a struct of tokens a `Theme` defines over a subtree. Each
token is a name with a definition, and a definition may answer conditions, the dark
scheme among them:

```rust
use skene_foundation::{Color, Definition, Media, Scalar, Token, TokenName, TokenRef, TokenSet};

/// Acme UI's colors.
#[derive(Debug, Clone, PartialEq)]
pub struct AcmeUiPalette<N: Scalar> {
  /// A badge's surface.
  pub badge: Token<Color, N>,
}

impl<N: Scalar> AcmeUiPalette<N> {
  /// A badge's surface.
  pub const BADGE: TokenRef<Color> = TokenRef::new("acme-ui-badge");
}

impl<N: Scalar> Default for AcmeUiPalette<N> {
  fn default() -> Self {
    Self {
      badge: Token::new(
        Self::BADGE.name().as_str(),
        Definition::from(Color::srgb(0.88, 0.91, 1.)).when(Media::Dark, Color::srgb(0.19, 0.18, 0.5)),
      ),
    }
  }
}

impl<N: Scalar, V: From<Definition<Color, N>>> TokenSet<V> for AcmeUiPalette<N> {
  fn definitions(&self) -> Vec<(TokenName, V)> {
    vec![self.badge.define()]
  }
}
```

A preset references a token by its `TokenRef` and emits only its name; the theme in
force supplies the value. A `Theme` of the palette defines it beneath, and where no
theme does, a read of it (`AcmeUiPalette::current()` or `AcmeUiPalette::watch()`)
defines its default at the root. A preset is a plain function that may run before any
platform is installed, so it defines nothing itself.

## The sizes

A library's own unit trait gives each medium's unit its sizes: logical pixels (`f32`) on
the web and whole cells (`i32`) on the terminal. A preset asks its facade's unit for
them, so the core keeps both media's sizes side by side:

```rust
use skene_foundation::{Inset, Length, Scalar};

/// A length unit Acme UI's presets know their sizes in.
#[diagnostic::on_unimplemented(
  message = "the Acme UI presets have no sizes in `{Self}`",
  label = "an Acme UI preset is drawn in logical pixels (`f32`) or cells (`i32`)"
)]
pub trait AcmeUiUnit: Scalar {
  /// The inset between a badge's edge and its label.
  fn badge_inset() -> Inset<Self>;
  /// The corner radius of a badge.
  fn badge_radius() -> Length;
}

impl AcmeUiUnit for f32 {
  fn badge_inset() -> Inset<Self> {
    Inset::symmetric(8, 2)
  }

  fn badge_radius() -> Length {
    Length::from(6)
  }
}

impl AcmeUiUnit for i32 {
  fn badge_inset() -> Inset<Self> {
    Inset::symmetric(1, 0)
  }

  fn badge_radius() -> Length {
    Length(0.5)
  }
}
```

## Presets per facade

A preset is a function returning a styled built-in of its facade, so every member of the
built-in applies to it and an app styles or wraps it further as it would the built-in.
Each facade's crate writes its presets over its own views, from the core's palette and
the sizes in its unit. The scaffold's web crate writes its badge this way:

```rust
# mod acme_ui {
#   use skene_foundation::{Color, Definition, Inset, Length, Scalar, Token, TokenName, TokenRef, TokenSet};
#
#   #[derive(Debug, Clone, PartialEq)]
#   pub struct AcmeUiPalette<N: Scalar> {
#     pub badge: Token<Color, N>,
#     pub badge_ink: Token<Color, N>,
#   }
#
#   impl<N: Scalar> AcmeUiPalette<N> {
#     pub const BADGE: TokenRef<Color> = TokenRef::new("acme-ui-badge");
#     pub const BADGE_INK: TokenRef<Color> = TokenRef::new("acme-ui-badge-ink");
#   }
#
#   impl<N: Scalar> Default for AcmeUiPalette<N> {
#     fn default() -> Self {
#       Self {
#         badge: Token::new(Self::BADGE.name().as_str(), Definition::from(Color::srgb(0.88, 0.91, 1.))),
#         badge_ink: Token::new(Self::BADGE_INK.name().as_str(), Definition::from(Color::srgb(0.19, 0.18, 0.5))),
#       }
#     }
#   }
#
#   impl<N: Scalar, V: From<Definition<Color, N>>> TokenSet<V> for AcmeUiPalette<N> {
#     fn definitions(&self) -> Vec<(TokenName, V)> {
#       vec![self.badge.define(), self.badge_ink.define()]
#     }
#   }
#
#   pub trait AcmeUiUnit: Scalar {
#     fn badge_inset() -> Inset<Self>;
#     fn badge_radius() -> Length;
#   }
#
#   impl AcmeUiUnit for f32 {
#     fn badge_inset() -> Inset<Self> {
#       Inset::symmetric(8, 2)
#     }
#
#     fn badge_radius() -> Length {
#       Length::from(6)
#     }
#   }
# }
use acme_ui::AcmeUiUnit;
use skene_web::prelude::*;

/// A badge: a short label on a rounded, tinted surface, a preset of [`Container`].
pub enum AcmeUiBadge {}

impl AcmeUiBadge {
  /// A badge reading `label`.
  #[expect(
    clippy::new_ret_no_self,
    reason = "a preset of a built-in view, so every member of the built-in applies"
  )]
  pub fn new(label: impl Into<String>) -> Container {
    Self::styled(Text::new(label))
  }

  /// A badge reading what `label` returns, kept current with the signals it reads.
  pub fn derive(label: impl Fn() -> String + 'static) -> Container {
    Self::styled(Text::derive(label))
  }

  /// `label` on a badge's surface, inked as a badge's label.
  pub fn styled(label: Text) -> Container {
    Container::new()
      .set_padding(<f32 as AcmeUiUnit>::badge_inset())
      .set_background(AcmeUiPalette::BADGE)
      .set_corner_radius(<f32 as AcmeUiUnit>::badge_radius())
      .set_child(label.set_color(AcmeUiPalette::BADGE_INK))
  }
}

/// The palette in the web's unit.
pub type AcmeUiPalette = acme_ui::AcmeUiPalette<f32>;

let badges = Theme::new(AcmeUiPalette::default()).set_child_builder(|| {
  Row::new()
    .set_spacing(8)
    .set_children((AcmeUiBadge::new("New"), AcmeUiBadge::new("Beta")))
});
```

The terminal's crate writes the same preset line for line over the terminal's views, in
cells (`<i32 as AcmeUiUnit>`). The type is a namespace no value inhabits:
`AcmeUiBadge::new` returns the container, never a badge. The pieces a preset composes
are public (`AcmeUiBadge::styled`), so another library composes them differently. A
preset with variants has a constructor per variant (`AcmeUiBadge::warning`), and a
variant the app changes at run time is a `derive_variant` whose built-in takes its
colors through `derive_` values, so a change re-inks it in place rather than rebuilding
it.

Where a preset is not enough, because the view's structure is the library's own rather
than one built-in's, the facade's crate writes a concrete view of its own: a function
returning `impl View`, composed from the facade's views as an app's are, its options its
parameters:

```rust
use skene_web::prelude::*;

/// A figure under its caption, the figure kept current with the signals `value` reads.
pub fn stat(caption: &str, value: impl Fn() -> String + 'static) -> impl View {
  Column::new().set_spacing(4).set_children((
    Text::new(caption).set_font_size(12.),
    Text::derive(value).set_font_size(28.),
  ))
}
```

An app depends on the platform crates it shells for, and imports each one's prelude
beside its facade's.

## A control of its own

A library's control is its facade's `Control` over a behavior. The shipped behaviors,
the `Behavior` trait a library's own implements with what its hooks name, and the
`ControlHandle` and `ControlOps` a behavior acts through live in each facade's
`behavior` module, so a library names them as an app does, and the facade's root keeps
its names for the views. A shipped behavior is configured as the app configures it:

```rust
use skene_web::{
  behavior::{Slider, SliderRange},
  prelude::*,
};

/// Acme's volume control: a slider from 0 to 11.
pub fn volume(level: Signal<f32>) -> BasicSlider {
  BasicSlider::new(
    Slider::new()
      .set_range(SliderRange::new(0., 11., 1.))
      .bind_value(level),
  )
  .set_label("Volume")
}
```

A behavior of the library's own states its role, the events it acts on and what it
reports, and the same `Control` gives it focus, a label, decoration and parts:

```rust
use skene_web::{
  Control,
  Prevent,
  behavior::{Behavior, ControlHandle, EventKind},
  prelude::*,
};

/// Acme's stepper: `ArrowUp` and `ArrowDown` move its count.
pub struct Stepper {
  count: Signal<f32>,
}

impl Behavior for Stepper {
  fn role(&self) -> Option<Role> {
    Some(Role::SpinButton)
  }

  fn listens(&self) -> &'static [EventKind] {
    &[EventKind::KeyDown]
  }

  fn on_key(&self, event: &KeyEvent, _: &ControlHandle) -> Prevent {
    match event.key {
      Key::Named(NamedKey::ArrowUp) => self.count.update(|count| *count += 1.),
      Key::Named(NamedKey::ArrowDown) => self.count.update(|count| *count -= 1.),
      _ => return Prevent::None,
    }

    Prevent::All
  }

  fn access(&self) -> Vec<AccessProp> {
    vec![AccessProp::Value(self.count.get())]
  }
}

/// A stepper over `count`.
pub fn stepper(count: Signal<f32>) -> Control<Stepper> {
  Control::new(Stepper { count }).set_label("Count")
}
```

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
