//! The gallery's views, in cells: every built-in view and control, live.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::prelude::*;

fn background() -> Color {
  Color::srgb(0.08, 0.08, 0.1)
}

fn surface() -> Color {
  Color::srgb(0.13, 0.13, 0.16)
}

fn edge() -> Color {
  Color::srgb(0.35, 0.35, 0.45)
}

fn faint() -> Color {
  Color::srgb(0.55, 0.57, 0.63)
}

fn bright() -> Color {
  Color::srgb(0.8, 0.9, 0.8)
}

fn accent() -> Color {
  Color::srgb(0.2, 0.22, 0.2)
}

/// The whole gallery: a scrollable column of titled sections.
#[must_use]
pub fn app() -> impl IntoView {
  Column::new()
    .set_item_alignment(ItemAlignment::Center)
    .set_overflow(Overflow::Auto)
    .set_color(background())
    .set_child(
      Column::new()
        .set_spacing(1)
        .set_max_width(56)
        .set_padding(Inset::symmetric(2, 1))
        .set_children((
          Text::new("GALLERY").set_bold(true).set_color(faint()),
          text(),
          buttons(),
          toggles(),
          inputs(),
          borders(),
          layouts(),
          list(),
        )),
    )
}

fn text() -> impl IntoView {
  section(
    "Text",
    Column::new().set_spacing(1).set_children((
      Text::new("A run of text with a color and a bold flag.")
        .set_bold(true)
        .set_color(bright()),
      Text::new(
        "Long runs wrap at the available width, breaking at spaces, so a paragraph \
         reads naturally in however many columns it gets.",
      )
      .set_color(faint()),
    )),
  )
}

fn buttons() -> impl IntoView {
  let count = Signal::tagged("clicks", 0u64);

  section(
    "Button",
    Row::new().set_spacing(2).set_children((
      button("Click me", move || count.update(|value| *value += 1)),
      Text::derive(move || format!("clicked {} time(s)", count.get())).set_color(faint()),
    )),
  )
}

fn toggles() -> impl IntoView {
  let agreed = Signal::tagged("agreed", true);
  let flavor = Signal::tagged("flavor", "mint".to_owned());

  section(
    "Checkbox & Radio",
    Column::new().set_spacing(1).set_children((
      Row::new().set_spacing(1).set_children((
        Checkbox::new().bind_checked(agreed),
        Text::derive(move || {
          if agreed.get() {
            "checked".to_owned()
          }
          else {
            "unchecked".to_owned()
          }
        })
        .set_color(faint()),
      )),
      Row::new().set_spacing(2).set_children((
        radio_choice("mint", flavor),
        radio_choice("vanilla", flavor),
        radio_choice("plum", flavor),
      )),
    )),
  )
}

fn radio_choice(value: &'static str, flavor: Signal<String>) -> impl IntoView {
  Row::new().set_spacing(1).set_children((
    Radio::new("flavor", value).bind_value(flavor),
    Text::new(value).set_color(faint()),
  ))
}

fn inputs() -> impl IntoView {
  let name = Signal::tagged("name", String::new());
  let volume = Signal::tagged("volume", 40.0f32);
  let season = Signal::tagged("season", "summer".to_owned());

  section(
    "TextField, Slider & Picker",
    Column::new().set_spacing(1).set_children((
      TextField::new()
        .set_placeholder("Your name")
        .bind_value(name)
        .set_style(BoxStyle::new().set_width(24).set_color(background())),
      Row::new().set_spacing(2).set_children((
        Container::new()
          .set_width(20)
          .set_child(Slider::new(0., 100.).set_step(1.).bind_value(volume)),
        Text::derive(move || format!("{:.0}%", volume.get())).set_color(faint()),
      )),
      Picker::new()
        .set_options([
          ("spring", "Spring"),
          ("summer", "Summer"),
          ("autumn", "Autumn"),
          ("winter", "Winter"),
        ])
        .bind_value(season),
    )),
  )
}

fn borders() -> impl IntoView {
  section(
    "BorderGlyphs",
    Row::new()
      .set_spacing(2)
      .set_wrap(Wrap::Forward)
      .set_line_spacing(1)
      .set_children((
        framed("light", BorderGlyphs::LIGHT),
        framed("round", BorderGlyphs::ROUNDED),
        framed("double", BorderGlyphs::DOUBLE),
        framed("heavy", BorderGlyphs::HEAVY),
        framed("ascii", BorderGlyphs::ASCII),
      )),
  )
}

fn framed(label: &'static str, glyphs: BorderGlyphs) -> impl IntoView {
  Container::new()
    .set_padding(Inset::horizontal(1))
    .set_border(edge())
    .set_border_glyphs(glyphs)
    .set_child(Text::new(label).set_color(faint()))
}

fn layouts() -> impl IntoView {
  let open = Signal::new(false);

  section(
    "Grid, Stack & Overlay",
    Column::new().set_spacing(1).set_children((
      Grid::uniform(4).set_spacing(1).set_children((
        swatch(0.30),
        swatch(0.40),
        swatch(0.50),
        swatch(0.60),
        swatch(0.60),
        swatch(0.50),
        swatch(0.40),
        swatch(0.30),
      )),
      Stack::new().set_height(3).set_color(accent()).set_child(
        Pinned::new(
          Container::new()
            .set_padding(Inset::horizontal(1))
            .set_color(surface())
            .set_child(Text::new("pinned").set_color(faint())),
        )
        .set_edges(skene_tui::core::Inset {
          top: Dimension::Length(0),
          right: Dimension::Length(1),
          bottom: Dimension::Auto,
          left: Dimension::Auto,
        }),
      ),
      Row::new().set_children((
        button("Open a modal", move || open.set(true)),
        Overlay::modal()
          .bind_open(open)
          .set_style(
            BoxStyle::new()
              .set_padding(Inset::symmetric(2, 1))
              .set_color(surface())
              .set_border(edge())
              .set_border_glyphs(BorderGlyphs::ROUNDED)
              .set_shadow(Color::srgb(0., 0., 0.).with_alpha(0.5)),
          )
          .set_children((
            Text::new("A modal overlay; Escape dismisses.").set_color(bright()),
            button("Close", move || open.set(false)),
          )),
      )),
    )),
  )
}

fn list() -> impl IntoView {
  section(
    "LazyColumn",
    LazyColumn::new(
      1,
      || 1000,
      |index| {
        Row::new().set_spacing(1).set_children((
          Text::new(format!("Row {index}")).set_color(bright()),
          Text::new("virtualized").set_color(faint()),
        ))
      },
    )
    .set_style(BoxStyle::new().set_height(6)),
  )
}

/// A titled card around one section's content.
fn section(title: &'static str, content: impl IntoView) -> impl IntoView {
  Column::new()
    .set_spacing(1)
    .set_padding(Inset::symmetric(2, 1))
    .set_min_width(52)
    .set_color(surface())
    .set_border(edge())
    .set_border_glyphs(BorderGlyphs::ROUNDED)
    .set_children((Text::new(title).set_bold(true).set_color(faint()), content))
}

fn button(label: &'static str, action: impl Fn() + 'static) -> impl IntoView {
  Button::new()
    .set_style(
      BoxStyle::new()
        .set_padding(Inset::horizontal(1))
        .set_color(accent())
        .set_cursor(MouseCursor::Click),
    )
    .set_when(
      Condition::Focused,
      BoxStyle::new().set_color(Color::srgb(0.3, 0.4, 0.3)),
    )
    .set_when(
      Condition::Hovered,
      BoxStyle::new().set_color(Color::srgb(0.28, 0.32, 0.28)),
    )
    .set_child(Text::new(label))
    .handle_click(action)
}

fn swatch(brightness: f32) -> impl IntoView {
  Container::new().set_height(1).set_color(Color::srgb(
    brightness * 0.5,
    brightness,
    brightness * 0.55,
  ))
}
