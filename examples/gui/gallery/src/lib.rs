//! The gallery: every built-in view and Simple component, live.

#![cfg(skene_platform = "web")]

use std::fmt;

use skene_gui::{
  Choice,
  HasChild,
  Link,
  Resource,
  behavior::SliderRange,
  prelude::*,
  router::Route,
};
use skene_simple_gui::prelude::*;

/// The gallery page: every built-in view and Simple component, under the palette the
/// reader picks.
#[main]
#[must_use]
pub fn app() -> impl View {
  // The palette in force is a signal the gallery switches from inside itself: the theme
  // redefines its tokens in place and rebuilds nothing, so every counter below keeps its
  // value across a switch. Each palette follows the OS scheme per token, and carries
  // the ambient ink and selection colors, so nothing here names a color a palette holds.
  let palette = Signal::tagged("palette", PaletteName::Neutral);

  Theme::derive(move || named_palette(palette.get())).set_child_builder(move || {
    Container::new()
      .set_role(Role::Main)
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24, 48))
          .add_child(
            Container::new().set_max_width(560).set_child(
              Column::new().set_spacing(24).set_children((
                Text::new("GALLERY")
                  .set_font_size(13.)
                  .set_font_weight(500.)
                  .set_color(SimplePalette::MUTED_FOREGROUND),
                palettes(palette),
                typography(),
                Column::new()
                  .set_spacing(24)
                  .set_children((buttons(), built_ins())),
                tooltips(),
                toggles(),
                inputs(),
                layouts(),
                list(),
                lazy_row(),
                Column::new().set_spacing(24).set_children((
                  rows(),
                  context_menu(),
                  link(),
                  visibility_and_one_of(),
                  boundaries(),
                  painted(),
                  third_party(),
                )),
              )),
            ),
          ),
      )
  })
}

/// The palettes the reader switches between. It derives `Snapshot`, since the signal
/// holding the choice is tagged and carried across a reload.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Snapshot)]
enum PaletteName {
  #[snapshot(default)]
  Neutral,
  Indigo,
  Rose,
}

/// The palette `name` selects, each from the public constructors: the neutral default,
/// or the default with a brand accent laid over its ring, its link and its primary look.
fn named_palette(name: PaletteName) -> SimplePalette {
  let (brand, brand_dark, hover, hover_dark) = match name {
    PaletteName::Indigo => (
      Color::srgb(0.29, 0.33, 0.86),
      Color::srgb(0.51, 0.55, 0.97),
      Color::srgb(0.24, 0.27, 0.75),
      Color::srgb(0.62, 0.65, 0.99),
    ),
    PaletteName::Rose => (
      Color::srgb(0.80, 0.16, 0.40),
      Color::srgb(0.96, 0.45, 0.65),
      Color::srgb(0.68, 0.12, 0.33),
      Color::srgb(0.98, 0.58, 0.74),
    ),
    PaletteName::Neutral => return SimplePalette::default(),
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

fn palettes(palette: Signal<PaletteName>) -> impl View {
  section(
    "Palette",
    Column::new().set_spacing(12).set_children((
      SimpleRadioGroup::new()
        .bind_chosen(palette)
        .set_label("Palette")
        .set_child(Row::new().set_spacing(16).set_children((
          radio_choice(PaletteName::Neutral, "neutral"),
          radio_choice(PaletteName::Indigo, "indigo"),
          radio_choice(PaletteName::Rose, "rose"),
        ))),
      Text::new(
        "The theme redefines its tokens in place: switch, and the click count below keeps \
         its value.",
      )
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn typography() -> impl View {
  section(
    "Text",
    // The paragraph rhythm: text-box trimming removes the leading, so the spacing
    // carries the whole between-paragraph gap.
    Column::new().set_spacing(16).set_children((
      Text::new("A run of text with a size, a weight, and a color.").set_font_size(16.),
      Text::new(
        "Selection is opt-in: this paragraph is selectable, so drag across it and copy—\
         the chrome around it never joins in.",
      )
      .set_selectable(true)
      .set_font_size(14.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
      RichText::new().add_spans([
        plain("Rich text mixes "),
        plain("weights").set_font_weight(700.),
        plain(", "),
        plain("colors")
          .set_color(Color::srgb(0.7, 0.3, 0.).when(Media::Dark, Color::srgb(1., 0.62, 0.25))),
        plain(", and "),
        Span::new("links")
          .set_font_size(14.)
          .set_link("https://example.com"),
        plain(" in one paragraph."),
      ]),
    )),
  )
}

fn buttons() -> impl View {
  let count = Signal::tagged("clicks", 0u32);

  section(
    "Button",
    Column::new().set_spacing(12).set_children((
      Row::new()
        .set_spacing(8)
        .set_wrap(Wrap::Forward)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          SimpleButton::new("Primary").handle_activate(move || count.update(|value| *value += 1)),
          SimpleButton::secondary("Secondary"),
          SimpleButton::outline("Outline"),
          SimpleButton::ghost("Ghost"),
          SimpleButton::destructive("Destructive"),
          SimpleButton::new("Disabled").set_disabled(true),
        )),
      Text::derive(move || format!("the primary one clicked {} time(s)", count.get()))
        .set_font_size(13.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn built_ins() -> impl View {
  let published = Signal::tagged("published", 0u32);
  let agreed = Signal::tagged("built-in agreed", true);
  let wifi = Signal::tagged("built-in wifi", false);
  let size = Signal::tagged("built-in size", 2u8);
  let level = Signal::tagged("built-in level", 40.0f32);
  let title = Signal::tagged("built-in title", String::new());
  let summary = Signal::tagged("built-in summary", String::new());
  let unit = Signal::tagged("built-in unit", 2u8);
  let fruit = Signal::tagged("built-in fruit", Some(1u8));

  // The framework's own controls, drawn in the platform's colors: what a component
  // library's presets build on, each default a value its setters replace.
  section(
    "Built-in controls",
    Column::new().set_spacing(12).set_children((
      Row::new()
        .set_spacing(8)
        .set_wrap(Wrap::Forward)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          Button::new("Publish")
            .set_prominent(true)
            .handle_activate(move || published.update(|value| *value += 1)),
          Button::new("Cancel"),
          Button::new("Disabled").set_disabled(true),
        )),
      Row::new()
        .set_spacing(16)
        .set_wrap(Wrap::Forward)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          Checkbox::new().set_label("Agree").bind_checked(agreed),
          Toggle::new().set_label("Wi-Fi").bind_checked(wifi),
        )),
      Row::new()
        .set_spacing(16)
        .set_wrap(Wrap::Forward)
        .set_item_alignment(ItemAlignment::Start)
        .set_children((
          RadioGroup::new()
            .set_label("Size")
            .bind_chosen(size)
            .set_children((
              Radio::new(1u8).set_label("Small"),
              Radio::new(2u8).set_label("Medium"),
              Radio::new(3u8).set_label("Large"),
            )),
          Slider::new()
            .set_label("Level")
            .set_range(SliderRange::new(0., 100., 5.))
            .bind_value(level),
        )),
      Row::new()
        .set_spacing(16)
        .set_wrap(Wrap::Forward)
        .set_item_alignment(ItemAlignment::Start)
        .set_children((
          Column::new().set_spacing(8).set_children((
            TextField::new()
              .set_label("Title")
              .set_placeholder("Untitled")
              .bind_value(title),
            TextEditor::new()
              .set_label("Summary")
              .set_placeholder("A few lines")
              .bind_value(summary),
          )),
          Picker::new()
            .set_label("Unit")
            .bind_chosen(unit)
            .set_options((
              ListboxOption::new(1u8).set_label("Pixels"),
              ListboxOption::new(2u8).set_label("Points"),
              ListboxOption::new(3u8).set_label("Ems"),
            )),
          Listbox::new()
            .set_label("Fruit")
            .bind_chosen(fruit)
            .set_children((
              ListboxOption::new(1u8).set_label("Apple"),
              ListboxOption::new(2u8).set_label("Cherry"),
              ListboxOption::new(3u8).set_label("Plum"),
            )),
        )),
      Text::derive(move || format!("published {} time(s)", published.get()))
        .set_font_size(13.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn tooltips() -> impl View {
  section(
    "Tooltip",
    Row::new()
      .set_spacing(8)
      .set_wrap(Wrap::Forward)
      .set_item_alignment(ItemAlignment::Center)
      .set_children((
        SimpleTooltip::new("Saves your work").set_child(SimpleButton::secondary("Save")),
        SimpleTooltip::view(
          Row::new()
            .set_spacing(6)
            .set_item_alignment(ItemAlignment::Center)
            .set_children((
              Text::new("⌘S").set_color(SimplePalette::BACKGROUND),
              Text::new("saves, too").set_color(SimplePalette::BACKGROUND),
            )),
        )
        .set_placement(Placement::Bottom)
        .set_child(SimpleButton::outline("Hover me")),
        SimpleTooltip::new("Plain text takes one as well").set_child(
          Text::new("or a run of text")
            .set_font_size(14.)
            .set_color(SimplePalette::MUTED_FOREGROUND),
        ),
      )),
  )
}

fn toggles() -> impl View {
  let agreed = Signal::tagged("agreed", true);
  let flavor = Signal::tagged("flavor", Flavor::Mint);

  section(
    "Checkbox & Radio",
    Column::new().set_spacing(12).set_children((
      Row::new()
        .set_spacing(8)
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
        .set_child(Row::new().set_spacing(16).set_children((
          radio_choice(Flavor::Mint, "mint"),
          radio_choice(Flavor::Vanilla, "vanilla"),
          radio_choice(Flavor::Plum, "plum"),
        ))),
    )),
  )
}

/// A flavor the radio group chooses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Snapshot)]
enum Flavor {
  #[snapshot(default)]
  Mint,
  Vanilla,
  Plum,
}

/// A season the picker chooses.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Snapshot)]
enum Season {
  Spring,
  #[snapshot(default)]
  Summer,
  Autumn,
  Winter,
}

/// One radio of a group, tagged with `value` and captioned `label`.
fn radio_choice(value: impl Choice, label: &'static str) -> impl View {
  SimpleRadio::new(value).set_label(label)
}

fn inputs() -> impl View {
  let name = Signal::tagged("name", String::new());
  let bio = Signal::tagged("bio", String::new());
  let volume = Signal::tagged("volume", 40.0f32);
  let season = Signal::tagged("season", Season::Summer);

  section(
    "TextField, TextEditor, Slider & Picker",
    Column::new().set_spacing(16).set_children((
      SimpleTextField::new()
        .set_placeholder("Your name")
        .bind_value(name),
      SimpleTextEditor::new()
        .set_placeholder("A few lines about you")
        .set_lines(3)
        .bind_value(bio),
      Row::new()
        .set_spacing(12)
        .set_item_alignment(ItemAlignment::Center)
        .set_children((
          SimpleSlider::new()
            .set_accessible_name("Volume")
            .set_range(SliderRange::new(0., 100., 1.))
            .bind_value(volume),
          Text::derive(move || format!("{:.0}%", volume.get()))
            .set_font_size(13.)
            .set_color(SimplePalette::MUTED_FOREGROUND),
        )),
      SimplePicker::new()
        .set_label("Season")
        .set_options((
          SimpleListboxOption::new(Season::Spring, "Spring"),
          SimpleListboxOption::new(Season::Summer, "Summer"),
          SimpleListboxOption::new(Season::Autumn, "Autumn"),
          SimpleListboxOption::new(Season::Winter, "Winter"),
        ))
        .bind_chosen(season),
    )),
  )
}

fn layouts() -> impl View {
  let open = Signal::saved(false);

  section(
    "Grid, Stack & Overlay",
    Column::new().set_spacing(16).set_children((
      Grid::uniform(4).set_spacing(8).set_children((
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
        .set_height(64)
        .set_background(SimplePalette::SECONDARY_SURFACE)
        .set_corner_radius(8)
        .set_child(
          Stack::new().add_child(
            Pinned::new()
              .set_edges(Inset {
                top: Dimension::Auto,
                end: Dimension::Length(8.),
                bottom: Dimension::Length(8.),
                start: Dimension::Auto,
              })
              .set_child(badge("pinned")),
          ),
        ),
      Row::new().set_children((
        SimpleButton::outline("Open a modal").handle_activate(move || open.set(true)),
        // The overlay is layering and dismissal alone; the surface is a card inside it,
        // which is what dresses a popup in the Simple design language.
        Overlay::modal()
          .bind_open(open)
          .set_child(
            SimpleCard::new().set_child(Column::new().set_spacing(20).set_children((
              Text::new("A modal overlay; Escape or the backdrop dismisses.").set_font_size(14.),
              Row::new().set_arrangement(Arrangement::End).add_child(
                SimpleButton::secondary("Close").handle_activate(move || open.set(false)),
              ),
            ))),
          ),
      )),
    )),
  )
}

fn list() -> impl View {
  section(
    "LazyColumn",
    Container::new()
      .set_height(160)
      .set_child(LazyColumn::new().set_item_height(32).derive_items(
        || 0..1000,
        |index| *index,
        move |index| {
          Row::new()
            .set_spacing(8)
            .set_padding(Inset::symmetric(12, 6))
            .set_item_alignment(ItemAlignment::Center)
            .set_children((
              Text::new(format!("Row {index}")).set_font_size(13.),
              Text::new("virtualized: only the visible rows exist")
                .set_font_size(12.)
                .set_color(SimplePalette::MUTED_FOREGROUND),
            ))
        },
      )),
  )
}

fn lazy_row() -> impl View {
  section(
    "LazyRow",
    Container::new().set_height(48).set_child(
      LazyRow::new()
        .set_item_width(ItemExtent::Estimated(96.))
        .derive_items(
          || 0..1000,
          |index| *index,
          move |index| {
            Container::new()
              .set_padding(Inset::symmetric(12, 6))
              .set_child(Text::new(format!("Cell {index}")).set_font_size(13.))
          },
        ),
    ),
  )
}

fn rows() -> impl View {
  let opened = Signal::tagged("opened", 0u32);
  let starred = Signal::tagged("starred", 0u32);

  section(
    "Activatable row",
    Column::new().set_spacing(8).set_children((
      // A clickable container is an activatable row: a tab stop that Enter and Space
      // activate as a click does, while the button inside it owns its own presses.
      Container::new()
        .set_padding(Inset::symmetric(12, 8))
        .set_corner_radius(6)
        .set_background(
          SimplePalette::CARD
            .when(State::Hovered, SimplePalette::SECONDARY_SURFACE)
            .when(State::Focused, SimplePalette::SECONDARY_SURFACE),
        )
        .handle_click(move || opened.update(|value| *value += 1))
        .set_child(
          Row::new()
            .set_spacing(8)
            .set_arrangement(Arrangement::SpaceBetween)
            .set_item_alignment(ItemAlignment::Center)
            .set_children((
              Text::new("Inbox: a click or Enter opens it"),
              SimpleButton::outline("Star")
                .handle_activate(move || starred.update(|value| *value += 1)),
            )),
        ),
      Text::derive(move || {
        format!(
          "opened {} time(s), starred {} time(s)",
          opened.get(),
          starred.get()
        )
      })
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

fn context_menu() -> impl View {
  let last = Signal::tagged("last command", String::from("nothing yet"));
  let ran = move |verb: &'static str| move || last.set(verb.to_owned());

  section(
    "Context menu",
    Column::new().set_spacing(8).set_children((
      SimpleContextMenu::new()
        .set_items([
          MenuEntry::new("Open").handle_activate(ran("Opened")),
          MenuEntry::new("Rename").handle_activate(ran("Renamed")),
          MenuEntry::separator(),
          MenuEntry::new("Delete").handle_activate(ran("Deleted")),
        ])
        .set_child(
          Container::new()
            .set_padding(Inset::symmetric(12, 8))
            .set_corner_radius(6)
            .set_focus_order(FocusOrder::Auto)
            .set_background(
              SimplePalette::CARD
                .when(State::Hovered, SimplePalette::SECONDARY_SURFACE)
                .when(State::Focused, SimplePalette::SECONDARY_SURFACE),
            )
            .set_child(Text::new("Right-click me, or focus me and press Shift-F10")),
        ),
      Text::derive(move || format!("last command: {}", last.get()))
        .set_font_size(13.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )),
  )
}

/// The gallery's one route, which a link returns to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Gallery {
  /// The top of the page.
  Top,
}

impl Route for Gallery {
  fn parse(path: &str) -> Option<Self> {
    (path == "/").then_some(Self::Top)
  }

  fn to_path(&self) -> String {
    "/".to_owned()
  }
}

fn link() -> impl View {
  section(
    "Link",
    Link::new(Gallery::Top).set_child(
      Text::new("Back to the top")
        .set_color(SimplePalette::LINK)
        .set_decoration(TextDecoration::Underline),
    ),
  )
}

fn visibility_and_one_of() -> impl View {
  let shown = Signal::tagged("details shown", false);
  let pane = Signal::tagged("pane", String::from("summary"));

  section(
    "Visibility & OneOf",
    Column::new().set_spacing(12).set_children((
      SimpleCheckbox::new()
        .set_label("Show the details")
        .bind_checked(shown),
      Visibility::new()
        .derive_shown(move || shown.get())
        .set_child(
          Text::new("The details, built the first time they show.")
            .set_font_size(14.)
            .set_color(SimplePalette::MUTED_FOREGROUND),
        ),
      Row::new().set_spacing(8).set_children((
        SimpleButton::outline("Summary").handle_activate(move || pane.set(String::from("summary"))),
        SimpleButton::outline("History").handle_activate(move || pane.set(String::from("history"))),
      )),
      OneOf::new()
        .derive_key(move || pane.get())
        .add_case(String::from("summary"), || {
          Text::new("One case at a time: the summary.").set_font_size(14.)
        })
        .add_case(String::from("history"), || {
          Text::new("And now the history, the summary gone.").set_font_size(14.)
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
    Column::new().set_spacing(12).set_children((
      Suspense::new()
        .set_fallback_builder(|| Text::new("Loading…").set_color(SimplePalette::MUTED_FOREGROUND))
        .set_child_builder(|| {
          let loaded = Resource::client(|| async { String::from("Loaded in the background.") });

          Text::derive(move || loaded.get().unwrap_or_default()).set_font_size(14.)
        }),
      ErrorBoundary::new()
        .set_fallback_builder(|errors: Errors| {
          Row::new()
            .set_spacing(8)
            .set_item_alignment(ItemAlignment::Center)
            .set_children((
              Text::derive(move || {
                errors.with(|failed| failed.first().map_or_else(String::new, ToString::to_string))
              })
              .set_font_size(14.)
              .set_color(SimplePalette::DESTRUCTIVE_SURFACE),
              SimpleButton::outline("Recover").handle_activate(move || errors.clear()),
            ))
        })
        .set_child_builder(|| {
          let errors = Errors::current();

          SimpleButton::destructive("Fail").handle_activate(move || {
            if let Some(errors) = &errors {
              errors.report(Failed);
            }
          })
        }),
    )),
  )
}

/// A box painted through vello: the portable low-level path for pixel media, drawn on
/// the CPU into a canvas in the browser and left empty by a server render.
fn painted() -> impl View {
  use skene_gui::kurbo::{Circle, Rect};

  section(
    "Painted",
    Row::new()
      .set_spacing(12)
      .set_item_alignment(ItemAlignment::Center)
      .set_children((
        Painted::new(|canvas, size| {
          let (width, height) = (f64::from(size.width), f64::from(size.height));

          canvas.fill(
            &Rect::new(0., 0., width, height),
            Color::srgb(0.16, 0.25, 0.45),
          );
          canvas.fill(
            &Circle::new((width / 2., height / 2.), height.min(width) / 3.),
            Color::srgb(0.98, 0.80, 0.30),
          );
        })
        .set_width(120)
        .set_height(64),
        Text::new("a vello scene sized by its box")
          .set_font_size(13.)
          .set_color(SimplePalette::MUTED_FOREGROUND),
      )),
  )
}

/// A low-level view from outside the framework: a map through the web's native door,
/// served as its markup and painted once the browser holds its element.
fn third_party() -> impl View {
  section("Third party", acme_map::Map::new())
}

/// A titled card around one section's content.
fn section(title: &'static str, content: impl View) -> impl View {
  SimpleCard::new().set_child(
    Column::new()
      .set_spacing(SimpleCard::spacing())
      .set_children((
        Text::new(title)
          .set_font_size(12.)
          .set_font_weight(600.)
          .set_color(SimplePalette::MUTED_FOREGROUND),
        content,
      )),
  )
}

fn plain(text: &'static str) -> Span {
  Span::new(text)
    .set_font_size(14.)
    .set_color(SimplePalette::MUTED_FOREGROUND)
}

fn badge(text: &'static str) -> impl View {
  Container::new()
    .set_padding(Inset::symmetric(8, 4))
    .set_background(SimplePalette::CARD)
    .set_corner_radius(6)
    .set_child(
      Text::new(text)
        .set_font_size(12.)
        .set_color(SimplePalette::MUTED_FOREGROUND),
    )
}

fn swatch(brightness: f32) -> impl View {
  Container::new()
    .set_height(32)
    .set_background(Color::srgb(brightness * 0.5, brightness, brightness * 0.55))
    .set_corner_radius(6)
}
