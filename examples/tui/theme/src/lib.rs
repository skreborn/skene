//! Theming, in cells: a palette built property by property, an accent the reader
//! switches at run time, a nested theme, and a scheme the reader chooses.
//!
//! Nothing here treats light and dark as a pair of themes. A palette is one value whose
//! tokens answer conditions, and the color scheme is one of those conditions—which is
//! why the frame below is already right for the background the terminal reported, and
//! why the toggle re-inks it without rebuilding anything. A palette change is the same:
//! the theme redefines its tokens in place, and the tree beneath it stands.
//!
//! `Tab` moves focus, arrows adjust, `Enter` clicks, `Ctrl-Q` quits.

#![cfg(skene_platform = "terminal")]

use skene_simple_tui::prelude::*;
use skene_tui::prelude::*;

/// A definition taking `dark`'s value under the dark scheme and `light`'s otherwise.
fn pair(light: Color, dark: Color) -> Definition<Color> {
  Definition::from(light).when(Media::Dark, dark)
}

/// A brand accent: its surface in each scheme, and the surface under the pointer.
struct Accent {
  brand: Color,
  brand_dark: Color,
  hover: Color,
  hover_dark: Color,
}

/// The accents the reader chooses between. It derives `Snapshot`, since the signal
/// holding the choice is tagged and carried across a reload.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Snapshot)]
enum AccentName {
  #[snapshot(default)]
  Indigo,
  Teal,
}

/// The accent `name` selects.
fn accent(name: AccentName) -> Accent {
  match name {
    AccentName::Teal => Accent {
      brand: Color::srgb(0.05, 0.55, 0.53),
      brand_dark: Color::srgb(0.30, 0.80, 0.76),
      hover: Color::srgb(0.04, 0.46, 0.44),
      hover_dark: Color::srgb(0.42, 0.88, 0.84),
    },
    AccentName::Indigo => Accent {
      brand: Color::srgb(0.29, 0.33, 0.86),
      brand_dark: Color::srgb(0.51, 0.55, 0.97),
      hover: Color::srgb(0.24, 0.27, 0.75),
      hover_dark: Color::srgb(0.62, 0.65, 0.99),
    },
  }
}

/// The app's palette: the built-in one with the brand accent layered on, per property.
///
/// `set_primary` replaces one look wholesale; a look's own conditions ride its
/// definitions. Every `.when` here names `Media::Dark`, but nothing about the setter
/// is the scheme's—`Media::min_width(120)` would sit in exactly the same place.
fn palette(accent_name: AccentName) -> SimplePalette {
  let accent = accent(accent_name);

  SimplePalette::default()
    .set_ring(pair(accent.brand, accent.brand_dark))
    .set_primary(
      SimpleLook::new(
        SimpleVariant::Primary,
        pair(accent.brand, accent.brand_dark),
        pair(Color::srgb(1., 1., 1.), Color::srgb(0.09, 0.09, 0.14)),
      )
      .set_hover(pair(accent.hover, accent.hover_dark)),
    )
}

/// A palette for one subtree: the same tokens, warmed over. A nested theme replaces the
/// palette beneath it and leaves the one above alone.
fn warm(accent_name: AccentName) -> SimplePalette {
  palette(accent_name)
    .set_card(pair(
      Color::srgb(0.99, 0.96, 0.91),
      Color::srgb(0.16, 0.13, 0.09),
    ))
    .set_border(pair(
      Color::srgb(0.93, 0.87, 0.78),
      Color::srgb(0.27, 0.22, 0.16),
    ))
}

/// The whole page: the palette the accent selects, the scheme the reader chooses, and
/// a nested theme beneath.
#[main]
#[must_use]
pub fn app() -> impl View {
  // The reader's choice, or `None`—which leaves the background the terminal reported in
  // force.
  let scheme = Signal::new(chosen_scheme());
  // The accent is a signal the theme derives its palette from: a switch redefines the
  // tokens in place and rebuilds nothing beneath the theme.
  let accent = Signal::tagged("accent", AccentName::Indigo);

  Theme::derive(move || palette(accent.get())).set_child_builder(move || {
    derive_scheme(move || scheme.get());

    Container::new()
      .set_overflow(Overflow::ScrollIfNeeded)
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .add_child(
            Container::new().set_max_width(56).set_child(
              Column::new()
                .set_spacing(1)
                .set_padding(Inset::symmetric(2, 1))
                .set_children((
                  heading(),
                  chooser(scheme),
                  accents(accent),
                  swatches(),
                  nested(accent),
                )),
            ),
          ),
      )
  })
}

fn heading() -> impl View {
  Column::new().set_children((
    Text::new("THEME")
      .set_bold(true)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("One palette, whose tokens answer conditions.").set_color(SimplePalette::FOREGROUND),
  ))
}

/// The scheme chooser. `set_scheme` is a facade function rather than a theme method: the
/// scheme in force is the terminal's, exactly as its detected background is, while the
/// palette is what nests.
fn chooser(scheme: Signal<Option<ColorScheme>>) -> impl View {
  let choice = move |label: &'static str, wanted: Option<ColorScheme>| {
    SimpleButton::outline(label).handle_activate(move || scheme.set(wanted))
  };

  Row::new().set_spacing(2).set_children((
    choice("System", None),
    choice("Light", Some(ColorScheme::Light)),
    choice("Dark", Some(ColorScheme::Dark)),
  ))
}

/// The accent chooser: the radio group writes the signal the theme derives from.
fn accents(accent: Signal<AccentName>) -> impl View {
  SimpleRadioGroup::new()
    .bind_chosen(accent)
    .set_label("Accent")
    .set_child(Row::new().set_spacing(2).set_children((
      SimpleRadio::new(AccentName::Indigo).set_label("Indigo"),
      SimpleRadio::new(AccentName::Teal).set_label("Teal"),
    )))
}

fn swatches() -> impl View {
  let swatch = |token: TokenRef<Color>, label: &'static str| {
    Column::new().set_children((
      Container::new()
        .set_height(3)
        .set_background(token)
        .set_border_width(Inset::all(1))
        .set_border_color(SimplePalette::BORDER)
        .set_border_glyphs(BorderGlyphs::ROUNDED),
      Text::new(label).set_color(SimplePalette::MUTED_FOREGROUND),
    ))
  };

  SimpleCard::new().set_child(
    Column::new().set_spacing(1).set_children((
      Text::new("Tokens")
        .set_bold(true)
        .set_color(SimplePalette::FOREGROUND),
      Grid::new()
        .set_columns(vec![
          GridTrack::Flex(1.),
          GridTrack::Flex(1.),
          GridTrack::Flex(1.),
        ])
        .set_spacing(1)
        .set_children((
          swatch(SimplePalette::CARD, "card"),
          swatch(SimplePalette::PRIMARY_SURFACE, "primary"),
          swatch(SimplePalette::DESTRUCTIVE_SURFACE, "destructive"),
        )),
      Row::new().set_spacing(2).set_children((
        SimpleButton::new("Primary"),
        SimpleButton::destructive("Delete"),
      )),
    )),
  )
}

/// A nested theme follows the same accent, warmed over: it derives its palette from the
/// signal too, and replaces the palette for its subtree only.
fn nested(accent: Signal<AccentName>) -> impl View {
  Theme::derive(move || warm(accent.get())).set_child_builder(|| {
    SimpleCard::new().set_child(
      Column::new().set_spacing(1).set_children((
        Text::new("Nested theme")
          .set_bold(true)
          .set_color(SimplePalette::FOREGROUND),
        Text::new("A theme inside a theme replaces the palette for its subtree only.")
          .set_color(SimplePalette::current().muted_foreground),
        Row::new().set_spacing(2).set_children((
          SimpleButton::new("Primary"),
          SimpleButton::outline("Cancel"),
        )),
      )),
    )
  })
}
