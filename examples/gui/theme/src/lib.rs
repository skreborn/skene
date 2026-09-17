//! Theming: a palette built property by property, an accent the reader switches at run
//! time, a nested theme, and a scheme the reader chooses.
//!
//! Nothing here treats light and dark as a pair of themes. A palette is one value whose
//! tokens answer conditions, and the color scheme is one of those conditions—which is
//! why the page below is already correct with no script running at all, and why the
//! toggle re-inks it without rebuilding anything. A palette change is the same: the
//! theme redefines its tokens in place, and the tree beneath it stands.

use skene_gui::prelude::*;
use skene_simple::gui::prelude::*;

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

/// The accent `name` selects; indigo unless the reader chose teal.
fn accent(name: Option<&str>) -> Accent {
  match name {
    Some("teal") => Accent {
      brand: Color::srgb(0.05, 0.55, 0.53),
      brand_dark: Color::srgb(0.30, 0.80, 0.76),
      hover: Color::srgb(0.04, 0.46, 0.44),
      hover_dark: Color::srgb(0.42, 0.88, 0.84),
    },
    _ => Accent {
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
/// is the scheme's—`Media::MinWidth(900.)` would sit in exactly the same place.
fn palette(accent_name: Option<&str>) -> SimplePalette {
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
fn warm(accent_name: Option<&str>) -> SimplePalette {
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

#[main]
fn app() -> impl IntoView {
  // The reader's stored choice, or `None`—which leaves the platform's own preference in
  // force, answered by the compiled media query with no script at all.
  let scheme = Signal::new(stored_scheme());
  // The accent is a signal the theme derives its palette from: a switch redefines the
  // tokens in place and rebuilds nothing beneath the theme.
  let accent = Signal::tagged("accent", Some("indigo".to_owned()));

  Theme::derive(move || accent.with(|name| palette(name.as_deref()))).set_child_builder(move || {
    derive_scheme(move || scheme.get());

    Container::new()
      .set_color(SimplePalette::BACKGROUND)
      .set_min_height(Dimension::Fraction(1.))
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24., 48.))
          .set_child(Container::new().set_max_width(460.).set_child(
            Column::new().set_spacing(24.).set_children((
              heading(),
              chooser(scheme),
              accents(accent),
              swatches(),
              nested(accent),
            )),
          )),
      )
  })
}

fn heading() -> impl IntoView {
  Column::new().set_spacing(6.).set_children((
    Text::new("THEME")
      .set_font_size(13.)
      .set_font_weight(500.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("One palette, whose tokens answer conditions.").set_font_size(15.),
  ))
}

/// The scheme chooser. `set_scheme` is a facade function rather than a theme method: the
/// scheme in force is the document's, exactly as the platform's own preference is, while
/// the palette is what nests.
fn chooser(scheme: Signal<Option<ColorScheme>>) -> impl IntoView {
  let choice = move |label: &'static str, wanted: Option<ColorScheme>| {
    SimpleButton::new(label)
      .set_variant(SimpleVariant::Outline)
      .handle_click(move || scheme.set(wanted))
  };

  Row::new().set_spacing(12.).set_children((
    choice("System", None),
    choice("Light", Some(ColorScheme::Light)),
    choice("Dark", Some(ColorScheme::Dark)),
  ))
}

/// The accent chooser: the radio group writes the signal the theme derives from.
fn accents(accent: Signal<Option<String>>) -> impl IntoView {
  SimpleRadioGroup::new()
    .bind_chosen(accent)
    .set_label("Accent")
    .set_child(Row::new().set_spacing(16.).set_children((
      SimpleRadio::new("indigo").set_label("Indigo"),
      SimpleRadio::new("teal").set_label("Teal"),
    )))
}

fn swatches() -> impl IntoView {
  let swatch = |token: TokenRef<Color>, label: &'static str| {
    Column::new().set_spacing(6.).set_children((
      Container::new()
        .set_height(44.)
        .set_corner_radius(8.)
        .set_color(token)
        .set_border(Border::new(1., SimplePalette::BORDER)),
      Text::new(label)
        .set_font_size(12.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
    ))
  };

  SimpleCard::new().set_spacing(16.).set_children((
    Text::new("Tokens").set_font_weight(600.),
    Grid::new()
      .set_columns(vec![
        GridTrack::Flex(1.),
        GridTrack::Flex(1.),
        GridTrack::Flex(1.),
      ])
      .set_spacing(12.)
      .set_children((
        swatch(SimplePalette::CARD, "card"),
        swatch(SimplePalette::PRIMARY_SURFACE, "primary"),
        swatch(SimplePalette::DESTRUCTIVE_SURFACE, "destructive"),
      )),
    Row::new().set_spacing(12.).set_children((
      SimpleButton::new("Primary"),
      SimpleButton::new("Delete").set_variant(SimpleVariant::Destructive),
    )),
  ))
}

/// A nested theme follows the same accent, warmed over: it derives its palette from the
/// signal too, and replaces the palette for its subtree only.
fn nested(accent: Signal<Option<String>>) -> impl IntoView {
  Theme::derive(move || accent.with(|name| warm(name.as_deref()))).set_child_builder(|| {
    SimpleCard::new().set_spacing(12.).set_children((
      Text::new("Nested theme").set_font_weight(600.),
      Text::new("A theme inside a theme replaces the palette for its subtree only.")
        .set_font_size(14.)
        .set_color(SimplePalette::current().muted_foreground),
      Row::new().set_spacing(12.).set_children((
        SimpleButton::new("Primary"),
        SimpleButton::new("Cancel").set_variant(SimpleVariant::Outline),
      )),
    ))
  })
}
