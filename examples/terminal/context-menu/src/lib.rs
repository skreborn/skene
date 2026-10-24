//! Context menus, in cells: a menu each note opts into, a field whose menu adds the
//! clipboard's standard items, and the terminal's standard menu everywhere else.
//!
//! A context menu is a view wrapping the one it serves, so it is opted into per view:
//! a right press, `Shift-F10` or the menu key on the wrapped view opens it at the
//! pointer in place of the standard one, and a view outside any wrapper gets the
//! standard menu the terminal serves from the root. The standard items follow the
//! clipboard: `Copy` is listed only where something is selected, and `Cut` and `Paste`
//! only inside a text entry.
//!
//! `Tab` moves focus, a right click or `Shift-F10` opens a menu, `Ctrl-Q` quits.

#![cfg(skene_platform = "terminal")]

use skene_simple_terminal::prelude::*;
use skene_terminal::prelude::*;

/// The app: the notes with their menus, the draft with the clipboard's, and the rest
/// under the standard menu.
#[main]
#[must_use]
pub fn app() -> impl View {
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
      .set_overflow(Overflow::ScrollIfNeeded)
      .set_background(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .add_child(
            Container::new().set_max_width(56).set_child(
              Column::new()
                .set_spacing(1)
                .set_padding(Inset::symmetric(2, 1))
                .set_children((
                  heading(),
                  notes_card(notes, last),
                  draft_card(draft),
                  Text::new("Right-click around the cards: the standard menu.")
                    .set_selectable(true)
                    .set_color(SimplePalette::MUTED_FOREGROUND),
                )),
            ),
          ),
      )
  })
}

fn heading() -> impl View {
  Column::new().set_children((
    Text::new("CONTEXT MENU")
      .set_bold(true)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("A menu is a view wrapping the one it serves.").set_color(SimplePalette::FOREGROUND),
  ))
}

/// The notes, each wrapped in a menu of its own commands, and the last command run.
fn notes_card(notes: Signal<Vec<String>>, last: Signal<String>) -> impl View {
  SimpleCard::new().set_spacing(1).set_children((
    Text::new("Notes")
      .set_bold(true)
      .set_color(SimplePalette::FOREGROUND),
    Text::new("Right-click a note, or focus one and press Shift-F10.")
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Column::new().derive_children(
      move || notes.get(),
      Clone::clone,
      move |name| note(name, notes, last),
    ),
    Row::new().set_spacing(1).set_children((
      Text::new("Last command:").set_color(SimplePalette::FOREGROUND),
      Text::derive(move || last.get()).set_color(SimplePalette::MUTED_FOREGROUND),
    )),
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
        .set_padding(Inset::symmetric(1, 0))
        .set_focus_order(FocusOrder::Auto)
        .set_background(
          SimplePalette::CARD
            .when(State::Hovered, SimplePalette::SECONDARY_SURFACE)
            .when(State::Focused, SimplePalette::SECONDARY_SURFACE),
        )
        .set_child(Text::new(name).set_color(SimplePalette::FOREGROUND)),
    )
}

/// A field whose menu carries the clipboard's standard items beside a command of its own.
fn draft_card(draft: Signal<String>) -> impl View {
  SimpleCard::new().set_spacing(1).set_children((
    Text::new("Draft")
      .set_bold(true)
      .set_color(SimplePalette::FOREGROUND),
    Text::new("Cut, Copy and Paste follow the selection and the clipboard.")
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
