//! The gallery's views, in cells: every built-in view and Simple component, live.

#![cfg(not(target_arch = "wasm32"))]

use std::fmt;

use skene_simple::tui::prelude::*;
use skene_tui::{ActivePlatform, HasChild, Resource, prelude::*};

/// The whole gallery: a scrollable column of titled sections, dressed by one theme
/// whose palette the gallery switches from inside itself and whose tokens follow the
/// terminal's own background.
#[must_use]
pub fn app() -> impl View {
  // The theme redefines its tokens in place and rebuilds nothing, so every counter
  // below keeps its value across a switch.
  let palette = Signal::tagged("palette", Some("neutral".to_owned()));

  Theme::derive(move || palette.with(|name| named_palette(name.as_deref())))
    .set_child_builder(move || gallery(palette))
}

/// The palette `name` selects, each from the public constructors: the neutral default,
/// or the default with a brand accent laid over its ring, its link and its primary look.
fn named_palette(name: Option<&str>) -> SimplePalette {
  let (brand, brand_dark, hover, hover_dark) = match name {
    Some("indigo") => (
      Color::srgb(0.29, 0.33, 0.86),
      Color::srgb(0.51, 0.55, 0.97),
      Color::srgb(0.24, 0.27, 0.75),
      Color::srgb(0.62, 0.65, 0.99),
    ),
    Some("rose") => (
      Color::srgb(0.80, 0.16, 0.40),
      Color::srgb(0.96, 0.45, 0.65),
      Color::srgb(0.68, 0.12, 0.33),
      Color::srgb(0.98, 0.58, 0.74),
    ),
    _ => return SimplePalette::default(),
  };
  let pair = |light: Color, dark: Color| Definition::from(light).when(Media::Dark, dark);

  SimplePalette::default()
    .set_ring(pair(brand, brand_dark))
    .set_link(pair(brand, brand_dark))
    .set_primary(
      SimpleLook::new(
        SimpleVariant::Primary,
        pair(brand, brand_dark),
        pair(Color::srgb(1., 1., 1.), Color::srgb(0.09, 0.09, 0.14)),
      )
      .set_hover(pair(hover, hover_dark)),
    )
}

fn gallery(palette: Signal<Option<String>>) -> impl View {
  Container::new()
    .set_overflow(Overflow::Auto)
    .set_color(SimplePalette::BACKGROUND)
    .set_child(
      Column::new()
        .set_item_alignment(ItemAlignment::Center)
        .add_child(
          Container::new().set_max_width(56).set_child(
            Column::new()
              .set_spacing(1)
              .set_padding(Inset::symmetric(2, 1))
              .set_children((
                Text::new("GALLERY")
                  .set_bold(true)
                  .set_color(SimplePalette::MUTED_FOREGROUND),
                palettes(palette),
                text(),
                buttons(),
                tooltips(),
                toggles(),
                inputs(),
                borders(),
                layouts(),
                list(),
                lazy_row(),
                Column::new().set_spacing(1).set_children((
                  context_menu(),
                  show_and_switch(),
                  boundaries(),
                  third_party(),
                )),
              )),
          ),
        ),
    )
}

fn palettes(palette: Signal<Option<String>>) -> impl View {
  section(
    "Palette",
    Column::new().set_spacing(1).set_children((
      SimpleRadioGroup::new()
        .bind_chosen(palette)
        .set_label("Palette")
        .set_child(Row::new().set_spacing(2).set_children((
          radio_choice("neutral"),
          radio_choice("indigo"),
          radio_choice("rose"),
        ))),
      Text::new(
        "The theme redefines its tokens in place: switch, and the click count below keeps \
         its value.",
      )
      .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn text() -> impl View {
  section(
    "Text",
    Column::new().set_spacing(1).set_children((
      Text::new("A run of text with a color and a bold flag.")
        .set_bold(true)
        .set_color(SimplePalette::FOREGROUND),
      Text::new(
        "Long runs wrap at the available width, breaking at spaces, so a paragraph \
         reads naturally in however many columns it gets.",
      )
      .set_color(SimplePalette::MUTED_FOREGROUND),
      Text::new(
        "This paragraph opts into selection: drag to select—double-click a word, \
         triple-click a run—and right-click to copy it as it appears in the source.",
      )
      .set_selectable(true)
      .set_color(SimplePalette::MUTED_FOREGROUND),
      RichText::new().set_spans([
        muted("Rich text mixes "),
        Span::new("weights").set_bold(true),
        muted(", "),
        Span::new("colors")
          .set_color(Color::srgb(0.7, 0.3, 0.).when(Media::Dark, Color::srgb(1., 0.62, 0.25))),
        muted(", and "),
        Span::new("links").set_link("https://example.com"),
        muted(" in one paragraph."),
      ]),
    )),
  )
}

fn buttons() -> impl View {
  let count = Signal::tagged("clicks", 0u64);

  section(
    "Button",
    Column::new().set_spacing(1).set_children((
      Row::new()
        .set_spacing(2)
        .set_wrap(Wrap::Forward)
        .set_line_arrangement(Arrangement::Start)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          SimpleButton::new("Primary").handle_click(move || {
            count.update(|value| *value += 1);
          }),
          SimpleButton::new("Secondary").set_variant(SimpleVariant::Secondary),
          SimpleButton::new("Outline").set_variant(SimpleVariant::Outline),
          SimpleButton::new("Ghost").set_variant(SimpleVariant::Ghost),
          SimpleButton::new("Destructive").set_variant(SimpleVariant::Destructive),
          SimpleButton::new("Disabled").set_disabled(true),
        )),
      Text::derive(move || format!("the primary one clicked {} time(s)", count.get()))
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn tooltips() -> impl View {
  section(
    "Tooltip",
    Row::new().set_spacing(2).set_children((
      SimpleTooltip::new("Saves your work")
        .set_child(SimpleButton::new("Save").set_variant(SimpleVariant::Secondary)),
      SimpleTooltip::view(Text::new("a view, too").set_color(SimplePalette::BACKGROUND))
        .set_placement(Placement::Bottom)
        .set_child(SimpleButton::new("Hover me").set_variant(SimpleVariant::Outline)),
    )),
  )
}

fn toggles() -> impl View {
  let agreed = Signal::tagged("agreed", true);
  let flavor = Signal::tagged("flavor", Some("mint".to_owned()));

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
        .set_color(SimplePalette::MUTED_FOREGROUND),
      )),
      SimpleRadioGroup::new()
        .bind_chosen(flavor)
        .set_label("Flavor")
        .set_child(Row::new().set_spacing(2).set_children((
          radio_choice("mint"),
          radio_choice("vanilla"),
          radio_choice("plum"),
        ))),
    )),
  )
}

fn radio_choice(value: &'static str) -> impl View {
  SimpleRadio::new(value).set_label(value)
}

fn inputs() -> impl View {
  let name = Signal::tagged("name", String::new());
  let bio = Signal::tagged("bio", String::new());
  let volume = Signal::tagged("volume", 40.0f32);
  let season = Signal::tagged("season", "summer".to_owned());

  section(
    "TextField, TextEditor, Slider & Picker",
    Column::new().set_spacing(1).set_children((
      SimpleTextField::new()
        .set_placeholder("Your name")
        .bind_value(name),
      SimpleTextEditor::new()
        .set_placeholder("A few lines about you")
        .set_lines(3)
        .bind_value(bio),
      Row::new().set_spacing(2).set_children((
        Container::new().set_width(20).set_child(
          SimpleSlider::new()
            .set_label("Volume")
            .set_min(0.)
            .set_max(100.)
            .set_step(1.)
            .bind_value(volume),
        ),
        Text::derive(move || format!("{:.0}%", volume.get()))
          .set_color(SimplePalette::MUTED_FOREGROUND),
      )),
      SimplePicker::new()
        .set_label("Season")
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

fn borders() -> impl View {
  section(
    "BorderGlyphs",
    Row::new()
      .set_spacing(2)
      .set_wrap(Wrap::Forward)
      .set_line_arrangement(Arrangement::Start)
      .set_children((
        framed("light", BorderGlyphs::LIGHT),
        framed("round", BorderGlyphs::ROUNDED),
        framed("double", BorderGlyphs::DOUBLE),
        framed("heavy", BorderGlyphs::HEAVY),
        framed("ascii", BorderGlyphs::ASCII),
      )),
  )
}

fn framed(label: &'static str, glyphs: BorderGlyphs) -> impl View {
  Container::new()
    .set_padding(Inset::horizontal(1))
    .set_border_width(Inset::all(1))
    .set_border_color(SimplePalette::BORDER)
    .set_border_glyphs(glyphs)
    .set_child(Text::new(label).set_color(SimplePalette::MUTED_FOREGROUND))
}

fn layouts() -> impl View {
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
      Container::new()
        .set_height(5)
        .set_color(SimplePalette::SECONDARY_SURFACE)
        .set_child(
          Stack::new().add_child(
            Pinned::new()
              .set_edges(Inset {
                top: Dimension::Auto,
                right: Dimension::Length(1),
                bottom: Dimension::Length(1),
                left: Dimension::Auto,
              })
              .set_child(
                Container::new()
                  .set_padding(Inset::horizontal(1))
                  .set_color(SimplePalette::CARD)
                  .set_child(Text::new("pinned").set_color(SimplePalette::MUTED_FOREGROUND)),
              ),
          ),
        ),
      Row::new().set_children((
        SimpleButton::new("Open a modal")
          .set_variant(SimpleVariant::Secondary)
          .handle_click(move || open.set(true)),
        // The overlay is layering and dismissal alone: the surface is a card inside it,
        // which is what dresses a popup in the Simple design language. Only the shadow
        // stays on the overlay itself, since a cast shadow is no palette's token.
        Overlay::modal()
          .bind_open(open)
          .set_shadow(Color::srgb(0., 0., 0.).with_alpha(0.5))
          .set_child(
            SimpleCard::new().set_children((
              Text::new("A modal overlay; Escape dismisses.").set_color(SimplePalette::FOREGROUND),
              Row::new().set_arrangement(Arrangement::End).add_child(
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

fn list() -> impl View {
  section(
    "LazyColumn",
    Container::new()
      .set_height(6)
      .set_child(LazyColumn::new().set_item_height(1.).derive_items(
        || 0..1000,
        |index| *index,
        |index| {
          Row::new().set_spacing(1).set_children((
            Text::new(format!("Row {index}")).set_color(SimplePalette::FOREGROUND),
            Text::new("virtualized").set_color(SimplePalette::MUTED_FOREGROUND),
          ))
        },
      )),
  )
}

fn lazy_row() -> impl View {
  section(
    "LazyRow",
    Container::new().set_width(40).set_height(1).set_child(
      LazyRow::new().set_item_width(9.).derive_items(
        || 0..1000,
        |index| *index,
        |index| Text::new(format!("Cell {index}")).set_color(SimplePalette::FOREGROUND),
      ),
    ),
  )
}

/// A component library and a low-level view from outside the framework: the badge is
/// written once over any platform, the sparkline against the terminal's cells alone.
fn context_menu() -> impl View {
  let last = Signal::tagged("last command", String::from("nothing yet"));
  let ran = move |verb: &'static str| move || last.set(verb.to_owned());

  section(
    "Context menu",
    Column::new().set_children((
      SimpleContextMenu::new()
        .set_items([
          MenuEntry::new("Open").handle_select(ran("Opened")),
          MenuEntry::new("Rename").handle_select(ran("Renamed")),
          MenuEntry::separator(),
          MenuEntry::new("Delete").handle_select(ran("Deleted")),
        ])
        .set_child(
          Container::new()
            .set_padding(Inset::symmetric(1, 0))
            .set_focus_order(FocusOrder::Auto)
            .set_color(
              SimplePalette::CARD
                .when(State::Hovered, SimplePalette::SECONDARY_SURFACE)
                .when(State::Focused, SimplePalette::SECONDARY_SURFACE),
            )
            .set_child(Text::new("Right-click me, or focus me and press Shift-F10")),
        ),
      Text::derive(move || format!("last command: {}", last.get()))
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn show_and_switch() -> impl View {
  let shown = Signal::tagged("details shown", false);
  let pane = Signal::tagged("pane", String::from("summary"));

  section(
    "Show & Switch",
    Column::new().set_spacing(1).set_children((
      SimpleCheckbox::new()
        .set_label("Show the details")
        .bind_checked(shown),
      Show::new().derive_shown(move || shown.get()).set_child(
        Text::new("The details, built the first time they show.")
          .set_color(SimplePalette::MUTED_FOREGROUND),
      ),
      Row::new().set_spacing(2).set_children((
        SimpleButton::new("Summary")
          .set_variant(SimpleVariant::Outline)
          .handle_click(move || pane.set(String::from("summary"))),
        SimpleButton::new("History")
          .set_variant(SimpleVariant::Outline)
          .handle_click(move || pane.set(String::from("history"))),
      )),
      Switch::new()
        .derive_key(move || pane.get())
        .set_case(String::from("summary"), || {
          Text::new("One case at a time: the summary.")
        })
        .set_case(String::from("history"), || {
          Text::new("And now the history, the summary gone.")
        }),
    )),
  )
}

/// The error the failing button reports.
#[derive(Debug)]
struct Failed;

impl fmt::Display for Failed {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("the button failed, as asked")
  }
}

impl std::error::Error for Failed {}

fn boundaries() -> impl View {
  section(
    "Suspense & ErrorBoundary",
    Column::new().set_spacing(1).set_children((
      Suspense::new()
        .set_fallback(Text::new("Loading…").set_color(SimplePalette::MUTED_FOREGROUND))
        .set_child_builder(|| {
          let loaded = Resource::new(|| async { String::from("Loaded in the background.") });

          Text::derive(move || loaded.get().unwrap_or_default())
        }),
      ErrorBoundary::new()
        .set_fallback_builder(|errors: Errors| {
          Row::new()
            .set_spacing(2)
            .set_item_alignment(ItemAlignment::Center)
            .set_children((
              Text::derive(move || {
                errors.with(|failed| failed.first().map_or_else(String::new, ToString::to_string))
              })
              .set_color(SimplePalette::DESTRUCTIVE_SURFACE),
              SimpleButton::new("Recover")
                .set_variant(SimpleVariant::Outline)
                .handle_click(move || errors.clear()),
            ))
        })
        .set_child_builder(|| {
          let errors = Errors::current();

          SimpleButton::new("Fail")
            .set_variant(SimpleVariant::Destructive)
            .handle_click(move || {
              if let Some(errors) = &errors {
                errors.report(Failed);
              }
            })
        }),
    )),
  )
}

fn third_party() -> impl View {
  section(
    "Third party",
    Column::new().set_spacing(1).set_children((
      Row::new().set_spacing(2).set_children((
        Flexible::new()
          .set_shrink_factor(0.)
          .set_child(acme_ui::AcmeBadge::<ActivePlatform>::new("acme-ui")),
        Text::new("a badge from a library that names no platform")
          .set_color(SimplePalette::MUTED_FOREGROUND),
      )),
      Row::new().set_spacing(2).set_children((
        acme_sparkline::Sparkline::<ActivePlatform>::new([
          3., 5., 4., 8., 6., 9., 7., 10., 6., 4., 5., 3.,
        ])
        .set_color(Color::srgb(1., 0.62, 0.25)),
        Text::new("a sparkline painted cell by cell").set_color(SimplePalette::MUTED_FOREGROUND),
      )),
    )),
  )
}

/// A titled card around one section's content; the width divides the swatch grid into
/// whole-cell tracks.
fn section(title: &'static str, content: impl View) -> impl View {
  Container::new().set_min_width(53).set_child(
    SimpleCard::new().set_children((
      Text::new(title)
        .set_bold(true)
        .set_color(SimplePalette::MUTED_FOREGROUND),
      content,
    )),
  )
}

fn muted(text: &'static str) -> Span {
  Span::new(text).set_color(SimplePalette::MUTED_FOREGROUND)
}

fn swatch(brightness: f32) -> impl View {
  Container::new().set_height(1).set_color(Color::srgb(
    brightness * 0.5,
    brightness,
    brightness * 0.55,
  ))
}
