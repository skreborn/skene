//! Theming: a palette built property by property, a nested theme, and a scheme the
//! reader chooses.
//!
//! Nothing here treats light and dark as a pair of themes. A palette is one value whose
//! tokens answer conditions, and the color scheme is one of those conditions—which is
//! why the page below is already correct with no script running at all, and why the
//! toggle re-inks it without rebuilding anything.

use skene_gui::prelude::*;

/// A definition taking `dark`'s value under the dark scheme and `light`'s otherwise.
fn pair(light: Color, dark: Color) -> Definition<Color> {
  Definition::from(light).when(Media::Dark, dark)
}

/// The app's palette: the built-in one with a brand accent layered on, per property.
///
/// `set_primary` replaces one look wholesale; a look's own conditions ride its
/// definitions. Every `.when` here names `Media::Dark`, but nothing about the setter
/// is the scheme's—`Media::MinWidth(900.)` would sit in exactly the same place.
fn palette() -> SimplePalette {
  let brand = Color::srgb(0.29, 0.33, 0.86);
  let brand_dark = Color::srgb(0.51, 0.55, 0.97);

  SimplePalette::default()
    .set_ring(pair(brand, brand_dark))
    .set_primary(
      SimpleLook::new(
        SimpleVariant::Primary,
        pair(brand, brand_dark),
        pair(Color::srgb(1., 1., 1.), Color::srgb(0.09, 0.09, 0.14)),
      )
      .set_hover(pair(
        Color::srgb(0.24, 0.27, 0.75),
        Color::srgb(0.62, 0.65, 0.99),
      )),
    )
}

/// A palette for one subtree: the same tokens, warmed over. A nested theme replaces the
/// palette beneath it and leaves the one above alone.
fn warm() -> SimplePalette {
  palette()
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

  Theme::new(palette()).set_child_builder(move || {
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
              swatches(),
              nested(),
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

fn nested() -> impl IntoView {
  Theme::new(warm()).set_child_builder(|| {
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
