//! The notes screen on the pixel platforms: notes-core's state shown in skene-gui's views.
//!
//! Nothing here names a platform's build: the one platform difference is a plain `match` on
//! `Platform::CURRENT` (the title's size), and every view is a shared one.

use notes_core::{Note, Notes, Palette, Settings};
use skene_gui::prelude::*;

/// The app.
pub fn app() -> impl View {
  let notes = Notes::new();
  let settings = Settings::new();

  // The reader's scheme unless the app's setting asks for dark: the platform re-inks the
  // window, nothing is rebuilt.
  media::derive_scheme(move || settings.dark.get().then_some(Scheme::Dark));

  Theme::new(Palette::default()).set_child(
    Container::new()
      .set_background(Palette::BACKGROUND)
      .set_padding(16.when(Media::MinWidth(700), 32))
      .set_color(Palette::INK)
      .set_child(
        Column::new()
          .set_spacing(12.when(Media::MinWidth(700), 20))
          .add_child(header())
          .add_child(composer(notes))
          .add_child(list(notes))
          .add_child(detail(notes))
          .add_child(settings_row(settings)),
      ),
  )
}

/// The title, larger where the platform's titles are.
fn header() -> impl View {
  let size = match Platform::CURRENT {
    Platform::Ios | Platform::Macos => 34,
    _ => 28,
  };

  Row::new()
    .set_spacing(8)
    .set_item_alignment(AxisAlignment::Baseline)
    .add_child(
      Text::new("Notes")
        .set_font_size(size)
        .set_font_weight(FontWeight::BOLD),
    )
    .add_child(Text::new("· all notes").set_color(Palette::MUTED))
}

/// The text field and the Add button, side by side where the window is wide.
fn composer(notes: Notes) -> impl View {
  Row::new()
    .set_spacing(8)
    .set_item_alignment(AxisAlignment::End)
    .add_child(
      // A field's caption sits before it among its siblings: a column of its own stacks the
      // caption above the field.
      Flexible::new().set_flexibility(1).set_child(
        Column::new().set_spacing(4).add_child(
          TextField::new("New note")
            .set_placeholder("Title")
            .handle_activate(move || notes.add())
            .bind_text(notes.draft),
        ),
      ),
    )
    .add_child(Button::new("Add").handle_activate(move || notes.add()))
}

/// Every note, keyed by its id.
fn list(notes: Notes) -> impl View {
  Column::new().set_spacing(4).derive_children(
    move || notes.items.get(),
    |note| note.id,
    move |note| row(notes, note),
  )
}

/// One note: an activatable row (a click, `Enter`, `Space` or a reader's action selects it)
/// holding its title and its pinned switch, which owns its own press.
fn row(notes: Notes, note: Note) -> impl View {
  let id = note.id;

  Container::new()
    .derive_background(move || match notes.selected.get() == Some(id) {
      true => Palette::SELECTED,
      false => Palette::CARD,
    })
    .set_corner_radius(8)
    .set_padding(Inset::symmetric(12, 8))
    .set_label(note.title.clone())
    .handle_click(move || notes.select(id))
    .set_child(
      Row::new()
        .set_spacing(12)
        .set_item_alignment(AxisAlignment::Center)
        .add_child(
          Flexible::new()
            .set_flexibility(1)
            .set_child(Text::new(note.title.clone())),
        )
        .add_child(
          Toggle::new("Pinned")
            .set_accessible_name(format!("Pin {}", note.title))
            .derive_on(move || notes.is_pinned(id))
            .handle_change(move |on| notes.set_pinned(id, on)),
        ),
    )
}

/// The selected note.
fn detail(notes: Notes) -> impl View {
  Container::new()
    .set_background(Palette::CARD)
    .set_corner_radius(12)
    .set_padding(16)
    .set_label("Detail")
    .set_child(Derived::new(move || match notes.selected_note() {
      Some(note) => Column::new()
        .set_spacing(4)
        .add_child(
          Text::new(note.title)
            .set_font_size(20)
            .set_font_weight(FontWeight::SEMIBOLD),
        )
        .add_child(
          Text::new(match note.pinned {
            true => "Pinned",
            false => "Not pinned",
          })
          .set_color(Palette::MUTED),
        )
        .into_any(),
      None => Text::new("Select a note")
        .set_color(Palette::MUTED)
        .into_any(),
    }))
}

/// The settings row.
fn settings_row(settings: Settings) -> impl View {
  Toggle::new("Dark mode").bind_on(settings.dark)
}
