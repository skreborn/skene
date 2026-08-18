//! Theming: a palette built property by property, a nested theme, and a scheme the
//! reader chooses.
//!
//! Nothing here treats light and dark as a pair of themes. A palette is one value whose
//! tokens answer conditions, and the color scheme is one of those conditions—which is
//! why the page below is already correct with no script running at all, and why the
//! toggle re-inks it without rebuilding anything.

#![cfg(target_arch = "wasm32")]

use skene_gui::prelude::*;

/// The app's palette: the built-in one with a brand accent layered on, per property.
///
/// `set_primary` replaces one look wholesale; a look's own conditions sit on the look's
/// own setters. Every `set_<prop>_when` here names `DarkMode`, but nothing about the
/// setter is the scheme's—`Condition::MinWidth(900.)` would sit in exactly the same
/// place.
fn palette() -> SimplePalette {
  let brand = Color::srgb(0.29, 0.33, 0.86);
  let brand_dark = Color::srgb(0.51, 0.55, 0.97);

  SimplePalette::default()
    .set_ring(brand)
    .set_ring_when(Condition::DarkMode, brand_dark)
    .set_primary(
      SimpleLook::new(brand, Color::srgb(1., 1., 1.))
        .set_surface_when(Condition::DarkMode, brand_dark)
        .set_ink_when(Condition::DarkMode, Color::srgb(0.09, 0.09, 0.14))
        .set_hover(Color::srgb(0.24, 0.27, 0.75))
        .set_hover_when(Condition::DarkMode, Color::srgb(0.62, 0.65, 0.99)),
    )
}

/// A palette for one subtree: the same tokens, warmed over. A nested theme replaces the
/// palette beneath it and leaves the one above alone.
fn warm() -> SimplePalette {
  palette()
    .set_card(Color::srgb(0.99, 0.96, 0.91))
    .set_card_when(Condition::DarkMode, Color::srgb(0.16, 0.13, 0.09))
    .set_border(Color::srgb(0.93, 0.87, 0.78))
    .set_border_when(Condition::DarkMode, Color::srgb(0.27, 0.22, 0.16))
}

#[main]
fn app() -> impl IntoView {
  // The reader's stored choice, or `None`—which leaves the platform's own preference in
  // force, answered by the compiled media query with no script at all.
  let scheme = Signal::new(stored_scheme());

  SimpleTheme::new(palette()).set_child(move || {
    derive_scheme(move || scheme.get());

    Container::new()
      .set_color(SimplePalette::current().background)
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
      .set_color(SimplePalette::current().muted_foreground),
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
  let palette = SimplePalette::current();
  let swatch = |token: Token, label: &'static str| {
    Column::new().set_spacing(6.).set_children((
      Container::new()
        .set_height(44.)
        .set_border_radius(8.)
        .set_color(token)
        .set_border(palette.border.clone().map(|color| Border::new(1., color))),
      Text::new(label)
        .set_font_size(12.)
        .set_color(palette.muted_foreground.clone()),
    ))
  };

  SimpleCard::new().set_spacing(16.).set_children((
    Text::new("Tokens").set_font_weight(600.),
    Grid::new()
      .set_columns([
        GridTrack::Flex(1.),
        GridTrack::Flex(1.),
        GridTrack::Flex(1.),
      ])
      .set_spacing(12.)
      .set_children((
        swatch(palette.card.clone(), "card"),
        swatch(palette.primary.surface.clone(), "primary"),
        swatch(palette.destructive.surface.clone(), "destructive"),
      )),
    Row::new().set_spacing(12.).set_children((
      SimpleButton::new("Primary"),
      SimpleButton::new("Delete").set_variant(SimpleVariant::Destructive),
    )),
  ))
}

fn nested() -> impl IntoView {
  SimpleTheme::new(warm()).set_child(|| {
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
