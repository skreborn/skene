//! The gallery's views, in cells: every built-in view and Simple component, live.

#![cfg(not(target_arch = "wasm32"))]

use skene_tui::prelude::*;

/// The whole gallery: a scrollable column of titled sections, its text selecting in
/// the theme's colors.
#[must_use]
pub fn app() -> impl IntoView {
  TextTheme::new(gallery())
    .set_selection_color(SimplePalette::background())
    .set_selection_background(SimplePalette::foreground())
}

fn gallery() -> impl IntoView {
  Column::new()
    .set_item_alignment(ItemAlignment::Center)
    .set_overflow(Overflow::Auto)
    .set_color(SimplePalette::background())
    .set_child(
      Column::new()
        .set_spacing(1)
        .set_max_width(56)
        .set_padding(Inset::symmetric(2, 1))
        .set_children((
          Text::new("GALLERY")
            .set_bold(true)
            .set_color(SimplePalette::muted_foreground()),
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
        .set_color(SimplePalette::foreground()),
      Text::new(
        "Long runs wrap at the available width, breaking at spaces, so a paragraph \
         reads naturally in however many columns it gets.",
      )
      .set_color(SimplePalette::muted_foreground()),
      Text::new(
        "This paragraph opts into selection: drag to select—double-click a word, \
         triple-click a run—and right-click to copy it as it appears in the source.",
      )
      .set_selectable(true)
      .set_color(SimplePalette::muted_foreground()),
      RichText::new([
        Span::text("Rich text mixes ").set_color(SimplePalette::muted_foreground()),
        Span::text("weights").set_bold(true),
        Span::text(", ").set_color(SimplePalette::muted_foreground()),
        Span::text("colors").set_color(SimplePalette::foreground()),
        Span::text(", and ").set_color(SimplePalette::muted_foreground()),
        Span::text("links").set_link("https://example.com"),
        Span::text(" in one paragraph.").set_color(SimplePalette::muted_foreground()),
      ])
      .set_selectable(true),
    )),
  )
}

fn buttons() -> impl IntoView {
  let count = Signal::tagged("clicks", 0u64);

  section(
    "Button",
    Column::new().set_spacing(1).set_children((
      Row::new()
        .set_spacing(2)
        .set_wrap(Wrap::Forward)
        .set_line_spacing(1)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          SimpleButton::new("Primary").handle_click(move || count.update(|value| *value += 1)),
          SimpleButton::new("Secondary").set_variant(SimpleVariant::Secondary),
          SimpleButton::new("Outline").set_variant(SimpleVariant::Outline),
          SimpleButton::new("Ghost").set_variant(SimpleVariant::Ghost),
          SimpleButton::new("Destructive").set_variant(SimpleVariant::Destructive),
        )),
      Text::derive(move || format!("the primary one clicked {} time(s)", count.get()))
        .set_color(SimplePalette::muted_foreground()),
    )),
  )
}

fn toggles() -> impl IntoView {
  let agreed = Signal::tagged("agreed", true);
  let flavor = Signal::tagged("flavor", "mint".to_owned());

  section(
    "Checkbox & Radio",
    Column::new().set_spacing(1).set_children((
      Row::new().set_spacing(2).set_children((
        SimpleCheckbox::new()
          .bind_checked(agreed)
          .set_label("subscribed"),
        Text::derive(move || {
          if agreed.get() {
            "checked".to_owned()
          }
          else {
            "unchecked".to_owned()
          }
        })
        .set_color(SimplePalette::muted_foreground()),
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
  SimpleRadio::new("flavor", value)
    .bind_value(flavor)
    .set_label(value)
}

fn inputs() -> impl IntoView {
  let name = Signal::tagged("name", String::new());
  let volume = Signal::tagged("volume", 40.0f32);
  let season = Signal::tagged("season", "summer".to_owned());

  section(
    "TextField, Slider & Picker",
    Column::new().set_spacing(1).set_children((
      SimpleTextField::new()
        .set_placeholder("Your name")
        .bind_value(name),
      Row::new().set_spacing(2).set_children((
        Container::new()
          .set_width(20)
          .set_child(SimpleSlider::new(0., 100.).set_step(1.).bind_value(volume)),
        Text::derive(move || format!("{:.0}%", volume.get()))
          .set_color(SimplePalette::muted_foreground()),
      )),
      SimplePicker::new()
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
    .set_border(SimplePalette::border())
    .set_border_glyphs(glyphs)
    .set_child(Text::new(label).set_color(SimplePalette::muted_foreground()))
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
      Stack::new()
        .set_height(3)
        .set_color(SimplePalette::secondary())
        .set_child(
          Pinned::new(
            Container::new()
              .set_padding(Inset::horizontal(1))
              .set_color(SimplePalette::card())
              .set_child(Text::new("pinned").set_color(SimplePalette::muted_foreground())),
          )
          .set_edges(skene_tui::core::Inset {
            top: Dimension::Length(0),
            right: Dimension::Length(1),
            bottom: Dimension::Auto,
            left: Dimension::Auto,
          }),
        ),
      Row::new().set_children((
        SimpleButton::new("Open a modal")
          .set_variant(SimpleVariant::Secondary)
          .handle_click(move || open.set(true)),
        Overlay::modal()
          .bind_open(open)
          .set_style(
            BoxStyle::new()
              .set_padding(Inset::symmetric(2, 1))
              .set_color(SimplePalette::card())
              .set_border(SimplePalette::border())
              .set_border_glyphs(BorderGlyphs::ROUNDED)
              .set_shadow(Color::srgb(0., 0., 0.).with_alpha(0.5)),
          )
          .set_child(
            Column::new().set_spacing(1).set_children((
              Text::new("A modal overlay; Escape dismisses.")
                .set_color(SimplePalette::foreground()),
              Row::new().set_arrangement(Arrangement::End).set_child(
                SimpleButton::new("Close")
                  .set_variant(SimpleVariant::Secondary)
                  .handle_click(move || open.set(false)),
              ),
            )),
          ),
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
          Text::new(format!("Row {index}")).set_color(SimplePalette::foreground()),
          Text::new("virtualized").set_color(SimplePalette::muted_foreground()),
        ))
      },
    )
    .set_style(BoxStyle::new().set_height(6)),
  )
}

/// A titled card around one section's content.
fn section(title: &'static str, content: impl IntoView) -> impl IntoView {
  Container::new().set_min_width(52).set_child(
    SimpleCard::new().set_children((
      Text::new(title)
        .set_bold(true)
        .set_color(SimplePalette::muted_foreground()),
      content,
    )),
  )
}

fn swatch(brightness: f32) -> impl IntoView {
  Container::new().set_height(1).set_color(Color::srgb(
    brightness * 0.5,
    brightness,
    brightness * 0.55,
  ))
}
