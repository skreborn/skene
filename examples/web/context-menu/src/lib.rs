//! Context menus: a menu each note opts into, a field whose menu adds the clipboard's
//! standard items, and the browser's own menu everywhere else.
//!
//! A context menu is a view wrapping the one it serves, so it is opted into per view:
//! a right press, `Shift-F10` or the menu key on the wrapped view opens it at the
//! pointer in place of the browser's, and a view outside any wrapper keeps the
//! browser's menu. The standard items follow the clipboard: `Copy` is listed only where
//! something is selected, and `Cut` and `Paste` only inside a text entry.

use skene_simple_web::prelude::*;
use skene_web::prelude::*;

#[main]
fn app() -> impl View {
  let notes = Signal::tagged(
    "notes",
    ["Groceries", "Travel plans", "Reading list"]
      .map(str::to_owned)
      .to_vec(),
  );
  let last = Signal::tagged("last", String::from("nothing yet"));
  let draft = Signal::tagged("draft", String::from("Paste here, or cut this."));

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_background(SimplePalette::BACKGROUND)
      .set_min_height(Dimension::Fraction(1.))
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24, 48))
          .add_child(
            Container::new().set_max_width(460).set_child(
              Column::new().set_spacing(24).set_children((
                heading(),
                notes_card(notes, last),
                draft_card(draft),
                Text::new("Right-click the page around the cards: the browser's own menu.")
                  .set_font_size(13.)
                  .set_color(SimplePalette::MUTED_FOREGROUND),
              )),
            ),
          ),
      )
  })
}

fn heading() -> impl View {
  Column::new().set_spacing(6).set_children((
    Text::new("CONTEXT MENU")
      .set_font_size(13.)
      .set_font_weight(500.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("A menu is a view wrapping the one it serves.").set_font_size(15.),
  ))
}

/// The notes, each wrapped in a menu of its own commands, and the last command run.
fn notes_card(notes: Signal<Vec<String>>, last: Signal<String>) -> impl View {
  SimpleCard::new().set_spacing(12).set_children((
    Text::new("Notes").set_font_weight(600.),
    Text::new("Right-click a note, or focus one and press Shift-F10.")
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Column::new().set_spacing(4).derive_children(
      move || notes.get(),
      Clone::clone,
      move |name| note(name, notes, last),
    ),
    Text::new("Last command:").set_font_size(13.),
    Text::derive(move || last.get())
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
  ))
}

/// One note under a menu of its commands.
fn note(name: String, notes: Signal<Vec<String>>, last: Signal<String>) -> impl View {
  let ran = {
    let name = name.clone();

    move |verb: &'static str| {
      let name = name.clone();

      move || last.set(format!("{verb} {name}"))
    }
  };
  let record = ran("Deleted");
  let deleted = name.clone();

  SimpleContextMenu::new()
    .set_items([
      MenuEntry::new("Open")
        .set_shortcut(Shortcut::new(Modifiers::NONE, "Enter"))
        .handle_activate(ran("Opened")),
      MenuEntry::new("Rename")
        .set_shortcut(Shortcut::new(Modifiers::NONE, "F2"))
        .handle_activate(ran("Renamed")),
      MenuEntry::separator(),
      MenuEntry::new("Delete")
        .set_shortcut(Shortcut::new(Modifiers::NONE, "Delete"))
        .handle_activate(move || {
          notes.update(|notes| notes.retain(|held| *held != *deleted));
          record();
        }),
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
        .set_child(Text::new(name)),
    )
}

/// A field whose menu carries the clipboard's standard items beside a command of its own.
fn draft_card(draft: Signal<String>) -> impl View {
  SimpleCard::new().set_spacing(12).set_children((
    Text::new("Draft").set_font_weight(600.),
    Text::new("Cut, Copy and Paste follow the selection and the clipboard.")
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    SimpleContextMenu::new()
      .set_items([
        MenuEntry::cut(),
        MenuEntry::copy(),
        MenuEntry::paste(),
        MenuEntry::separator(),
        MenuEntry::new("Clear").handle_activate(move || draft.set(String::new())),
      ])
      .set_child(
        SimpleTextField::new()
          .set_placeholder("A draft")
          .bind_value(draft),
      ),
  ))
}
