//! The gallery: every built-in view and control, live, with minimal styling.

#![cfg(target_arch = "wasm32")]

use skene_gui::{Overlay, prelude::*};

fn background() -> Color {
  Color::srgb(0.1, 0.1, 0.1)
}

fn surface() -> Color {
  Color::srgb(0.15, 0.15, 0.15)
}

fn edge() -> Color {
  Color::srgb(0.2, 0.2, 0.2)
}

fn faint() -> Color {
  Color::srgb(0.55, 0.57, 0.63)
}

fn bright() -> Color {
  Color::srgb(0.78, 0.87, 0.8)
}

fn accent() -> Color {
  Color::srgb(0.195, 0.215, 0.195)
}

#[main]
fn app() -> impl IntoView {
  Column::new()
    .set_item_alignment(ItemAlignment::Center)
    .set_padding(Inset::symmetric(24., 48.))
    .set_color(background())
    .set_child(
      Column::new()
        .set_spacing(24.)
        .set_max_width(560.)
        .set_children((
          Text::new("GALLERY")
            .set_font_size(13.)
            .set_font_weight(500.)
            .set_color(faint()),
          typography(),
          buttons(),
          toggles(),
          inputs(),
          layouts(),
          list(),
        )),
    )
}

fn typography() -> impl IntoView {
  section(
    "Text",
    Column::new().set_spacing(12.).set_children((
      Text::new("A run of text with a size, a weight, and a color.")
        .set_font_size(16.)
        .set_color(bright()),
      RichText::new([
        plain("Rich text mixes "),
        plain("weights").set_font_weight(700.),
        plain(", "),
        plain("colors").set_color(bright()),
        plain(", and "),
        plain("links").set_link("https://example.com"),
        plain(" in one paragraph."),
      ]),
    )),
  )
}

fn buttons() -> impl IntoView {
  let count = Signal::new(0u32);

  section(
    "Button",
    Row::new()
      .set_spacing(12.)
      .set_item_alignment(ItemAlignment::Center)
      .set_children((
        Button::new()
          .handle_click(move || count.update(|value| *value += 1))
          .set_style(chip())
          .set_child(label("Click me")),
        Text::derive(move || format!("clicked {} time(s)", count.get()))
          .set_font_size(13.)
          .set_color(faint()),
      )),
  )
}

fn toggles() -> impl IntoView {
  let agreed = Signal::new(true);
  let flavor = Signal::new("mint".to_owned());

  section(
    "Checkbox & Radio",
    Column::new().set_spacing(12.).set_children((
      Row::new()
        .set_spacing(8.)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          Checkbox::new().bind_checked(agreed),
          Text::derive(move || {
            if agreed.get() {
              "checked".to_owned()
            }
            else {
              "unchecked".to_owned()
            }
          })
          .set_font_size(13.)
          .set_color(faint()),
        )),
      Row::new().set_spacing(16.).set_children((
        radio_choice("mint", flavor),
        radio_choice("vanilla", flavor),
        radio_choice("plum", flavor),
      )),
    )),
  )
}

fn radio_choice(value: &'static str, flavor: Signal<String>) -> impl IntoView {
  Row::new()
    .set_spacing(8.)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Radio::new("flavor", value).bind_value(flavor),
      Text::new(value).set_font_size(13.).set_color(faint()),
    ))
}

fn inputs() -> impl IntoView {
  let name = Signal::new(String::new());
  let bio = Signal::new(String::new());
  let volume = Signal::new(40.0f32);
  let season = Signal::new("summer".to_owned());

  section(
    "TextField, TextEditor, Slider & Picker",
    Column::new().set_spacing(16.).set_children((
      TextField::new()
        .set_placeholder("Your name")
        .bind_value(name)
        .set_style(field()),
      TextEditor::new()
        .set_placeholder("A few lines about you")
        .set_lines(3)
        .bind_value(bio)
        .set_style(field()),
      Row::new()
        .set_spacing(12.)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          Slider::new(0., 100.).set_step(1.).bind_value(volume),
          Text::derive(move || format!("{:.0}%", volume.get()))
            .set_font_size(13.)
            .set_color(faint()),
        )),
      Picker::new()
        .set_options([
          ("spring", "Spring"),
          ("summer", "Summer"),
          ("autumn", "Autumn"),
          ("winter", "Winter"),
        ])
        .bind_value(season)
        .set_style(field()),
    )),
  )
}

fn layouts() -> impl IntoView {
  let open = Signal::new(false);

  section(
    "Grid, Stack & Overlay",
    Column::new().set_spacing(16.).set_children((
      Grid::uniform(4).set_spacing(8.).set_children((
        swatch(0.30),
        swatch(0.40),
        swatch(0.50),
        swatch(0.60),
        swatch(0.60),
        swatch(0.50),
        swatch(0.40),
        swatch(0.30),
      )),
      Stack::new()
        .set_height(64.)
        .set_color(accent())
        .set_border_radius(8.)
        .set_child(
          Pinned::new(badge("pinned")).set_edges(skene_gui::core::Inset {
            top: Dimension::Auto,
            right: Dimension::Length(8.),
            bottom: Dimension::Length(8.),
            left: Dimension::Auto,
          }),
        ),
      Row::new().set_children((
        Button::new()
          .handle_click(move || open.set(true))
          .set_style(chip())
          .set_child(label("Open a modal")),
        Overlay::modal()
          .bind_open(open)
          .set_style(card())
          .set_children((
            Text::new("A modal overlay; Escape or the backdrop dismisses.")
              .set_font_size(14.)
              .set_color(bright()),
            Button::new()
              .handle_click(move || open.set(false))
              .set_style(chip())
              .set_child(label("Close")),
          )),
      )),
    )),
  )
}

fn list() -> impl IntoView {
  section(
    "LazyColumn",
    LazyColumn::new(
      32.,
      || 1000,
      |index| {
        Row::new()
          .set_spacing(8.)
          .set_padding(Inset::symmetric(12., 6.))
          .set_item_alignment(ItemAlignment::Center)
          .set_children((
            Text::new(format!("Row {index}"))
              .set_font_size(13.)
              .set_color(bright()),
            Text::new("virtualized: only the visible rows exist")
              .set_font_size(12.)
              .set_color(faint()),
          ))
      },
    )
    .set_style(BoxStyle::new().set_height(160.)),
  )
}

/// A titled card around one section's content.
fn section(title: &'static str, content: impl IntoView) -> impl IntoView {
  Column::new()
    .set_spacing(16.)
    .set_padding(Inset::symmetric(24., 20.))
    .set_color(surface())
    .set_border(Border::new(1., edge()))
    .set_border_radius(8.)
    .set_children((
      Text::new(title)
        .set_font_size(12.)
        .set_font_weight(600.)
        .set_color(faint()),
      content,
    ))
}

fn plain(text: &'static str) -> Span {
  Span::text(text).set_font_size(14.).set_color(faint())
}

fn label(text: &'static str) -> Text {
  Text::new(text).set_font_size(14.).set_color(bright())
}

fn badge(text: &'static str) -> impl IntoView {
  Container::new()
    .set_padding(Inset::symmetric(8., 4.))
    .set_color(surface())
    .set_border_radius(6.)
    .set_child(Text::new(text).set_font_size(12.).set_color(faint()))
}

fn swatch(brightness: f32) -> impl IntoView {
  Container::new()
    .set_height(32.)
    .set_color(Color::srgb(brightness * 0.5, brightness, brightness * 0.55))
    .set_border_radius(6.)
}

fn chip() -> BoxStyle {
  BoxStyle::new()
    .set_padding(Inset::symmetric(14., 8.))
    .set_color(accent())
    .set_border(Border::new(1., edge()))
    .set_border_radius(8.)
}

fn field() -> BoxStyle {
  BoxStyle::new()
    .set_padding(Inset::symmetric(12., 8.))
    .set_color(background())
    .set_border(Border::new(1., edge()))
    .set_border_radius(8.)
}

fn card() -> BoxStyle {
  BoxStyle::new()
    .set_padding(Inset::all(24.))
    .set_color(surface())
    .set_border(Border::new(1., edge()))
    .set_border_radius(12.)
}
