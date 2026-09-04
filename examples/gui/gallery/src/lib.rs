//! The gallery: every built-in view and Simple component, live.

use skene_gui::prelude::*;

#[main]
fn app() -> impl IntoView {
  // The theme follows the OS scheme, and it carries the ambient ink and selection
  // colors with it, so nothing here names a color that a palette already holds.
  Theme::new(SimplePalette::default()).set_child_builder(|| {
    Container::new()
      .set_color(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24., 48.))
          .set_child(
            Container::new().set_max_width(560.).set_child(
              Column::new().set_spacing(24.).set_children((
                Text::new("GALLERY")
                  .set_font_size(13.)
                  .set_font_weight(500.)
                  .set_color(SimplePalette::MUTED_FOREGROUND),
                typography(),
                buttons(),
                toggles(),
                inputs(),
                layouts(),
                list(),
              )),
            ),
          ),
      )
  })
}

fn typography() -> impl IntoView {
  section(
    "Text",
    // The paragraph rhythm: text-box trimming removes the leading, so the spacing
    // carries the whole between-paragraph gap.
    Column::new().set_spacing(16.).set_children((
      Text::new("A run of text with a size, a weight, and a color.").set_font_size(16.),
      Text::new(
        "Selection is opt-in: this paragraph is selectable, so drag across it and copy—\
         the chrome around it never joins in.",
      )
      .set_selectable(true)
      .set_font_size(14.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
      RichText::new().set_spans([
        plain("Rich text mixes "),
        plain("weights").set_font_weight(700.),
        plain(", "),
        plain("colors").set_color(Color::srgb(1., 0.62, 0.25)),
        plain(", and "),
        Span::new("links")
          .set_font_size(14.)
          .set_link("https://example.com"),
        plain(" in one paragraph."),
      ]),
    )),
  )
}

fn buttons() -> impl IntoView {
  let count = Signal::tagged("clicks", 0u32);

  section(
    "Button",
    Column::new().set_spacing(12.).set_children((
      Row::new()
        .set_spacing(8.)
        .set_wrap(Wrap::Forward)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          SimpleButton::new("Primary").handle_click(move || count.update(|value| *value += 1)),
          SimpleButton::new("Secondary").set_variant(SimpleVariant::Secondary),
          SimpleButton::new("Outline").set_variant(SimpleVariant::Outline),
          SimpleButton::new("Ghost").set_variant(SimpleVariant::Ghost),
          SimpleButton::new("Destructive").set_variant(SimpleVariant::Destructive),
        )),
      Text::derive(move || format!("the primary one clicked {} time(s)", count.get()))
        .set_font_size(13.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn toggles() -> impl IntoView {
  let agreed = Signal::tagged("agreed", true);
  let flavor = Signal::tagged("flavor", Some("mint".to_owned()));

  section(
    "Checkbox & Radio",
    Column::new().set_spacing(12.).set_children((
      Row::new()
        .set_spacing(8.)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
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
          .set_font_size(13.)
          .set_color(SimplePalette::MUTED_FOREGROUND),
        )),
      SimpleRadioGroup::new()
        .bind_chosen(flavor)
        .set_label("Flavor")
        .set_child(Row::new().set_spacing(16.).set_children((
          radio_choice("mint"),
          radio_choice("vanilla"),
          radio_choice("plum"),
        ))),
    )),
  )
}

fn radio_choice(value: &'static str) -> impl IntoView {
  SimpleRadio::new(value).set_label(value)
}

fn inputs() -> impl IntoView {
  let name = Signal::tagged("name", String::new());
  let bio = Signal::tagged("bio", String::new());
  let volume = Signal::tagged("volume", 40.0f32);
  let season = Signal::tagged("season", "summer".to_owned());

  section(
    "TextField, TextEditor, Slider & Picker",
    Column::new().set_spacing(16.).set_children((
      SimpleTextField::new()
        .set_placeholder("Your name")
        .bind_value(name),
      SimpleTextEditor::new()
        .set_placeholder("A few lines about you")
        .set_lines(3)
        .bind_value(bio),
      Row::new()
        .set_spacing(12.)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          SimpleSlider::new()
            .set_min(0.)
            .set_max(100.)
            .set_step(1.)
            .bind_value(volume),
          Text::derive(move || format!("{:.0}%", volume.get()))
            .set_font_size(13.)
            .set_color(SimplePalette::MUTED_FOREGROUND),
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
      Container::new()
        .set_height(64.)
        .set_color(SimplePalette::SECONDARY_SURFACE)
        .set_corner_radius(8.)
        .set_child(
          Stack::new().set_child(
            Pinned::new()
              .set_edges(Inset {
                top: Dimension::Auto,
                right: Dimension::Length(8.),
                bottom: Dimension::Length(8.),
                left: Dimension::Auto,
              })
              .set_child(badge("pinned")),
          ),
        ),
      Row::new().set_children((
        SimpleButton::new("Open a modal")
          .set_variant(SimpleVariant::Outline)
          .handle_click(move || open.set(true)),
        // The overlay is layering and dismissal alone; the surface is a card inside it,
        // which is what dresses a popup in the Simple design language.
        Overlay::modal().bind_open(open).set_child(
          SimpleCard::new().set_spacing(20.).set_children((
            Text::new("A modal overlay; Escape or the backdrop dismisses.").set_font_size(14.),
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
    Container::new().set_height(160.).set_child(
      LazyColumn::new().set_item_height(32.).derive_items(
        || 0..1000,
        |index| *index,
        move |index| {
          Row::new()
            .set_spacing(8.)
            .set_padding(Inset::symmetric(12., 6.))
            .set_item_alignment(ItemAlignment::Center)
            .set_children((
              Text::new(format!("Row {index}")).set_font_size(13.),
              Text::new("virtualized: only the visible rows exist")
                .set_font_size(12.)
                .set_color(SimplePalette::MUTED_FOREGROUND),
            ))
        },
      ),
    ),
  )
}

/// A titled card around one section's content.
fn section(title: &'static str, content: impl IntoView) -> impl IntoView {
  SimpleCard::new().set_children((
    Text::new(title)
      .set_font_size(12.)
      .set_font_weight(600.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    content,
  ))
}

fn plain(text: &'static str) -> Span {
  Span::new(text)
    .set_font_size(14.)
    .set_color(SimplePalette::MUTED_FOREGROUND)
}

fn badge(text: &'static str) -> impl IntoView {
  Container::new()
    .set_padding(Inset::symmetric(8., 4.))
    .set_color(SimplePalette::CARD)
    .set_corner_radius(6.)
    .set_child(
      Text::new(text)
        .set_font_size(12.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )
}

fn swatch(brightness: f32) -> impl IntoView {
  Container::new()
    .set_height(32.)
    .set_color(Color::srgb(brightness * 0.5, brightness, brightness * 0.55))
    .set_corner_radius(6.)
}
