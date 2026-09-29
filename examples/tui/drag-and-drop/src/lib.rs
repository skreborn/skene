//! Drag and drop, in cells: cards moving between two lists, each card offering itself
//! as a drag and each list taking the cards dropped on it.
//!
//! A box offers what a drag from it carries (`set_drag_data`), and a box takes the
//! drags carrying a value of its type (`handle_drop`). A drag starts once a press's
//! pointer leaves its cell; the card being dragged holds `Dragged` and the list under
//! the pointer that would take it `Targeted`, so both are styled as any state is.

#![cfg(not(target_arch = "wasm32"))]

use skene_simple::tui::prelude::*;
use skene_tui::prelude::*;

/// A card on its way from one list to another: its title and the list it leaves.
#[derive(Clone)]
struct Moving {
  title: String,
  from: &'static str,
}

/// The app: two lists of cards, each card draggable to the other list.
#[must_use]
pub fn app() -> impl IntoView {
  let todo = Signal::tagged(
    "todo",
    ["Sketch the layout", "Pick the palette", "Write the copy"]
      .map(str::to_owned)
      .to_vec(),
  );
  let done = Signal::tagged("done", vec![String::from("Set up the project")]);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_overflow(Overflow::Auto)
      .set_color(SimplePalette::BACKGROUND)
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_child(
            Container::new().set_max_width(56).set_child(
              Column::new()
                .set_spacing(1)
                .set_padding(Inset::symmetric(2, 1))
                .set_children((
                  heading(),
                  Row::new()
                    .set_spacing(2)
                    .set_children((list("To do", todo, done), list("Done", done, todo))),
                )),
            ),
          ),
      )
  })
}

fn heading() -> impl IntoView {
  Column::new().set_children((
    Text::new("DRAG AND DROP")
      .set_bold(true)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("Drag a card to the other list.").set_color(SimplePalette::FOREGROUND),
  ))
}

/// The cards in `own`, taking a card dropped on the list from `other`.
fn list(name: &'static str, own: Signal<Vec<String>>, other: Signal<Vec<String>>) -> impl IntoView {
  Container::new()
    .set_width(24)
    .set_padding(Inset::symmetric(1, 0))
    .set_color(SimplePalette::CARD.when(State::Targeted, SimplePalette::SECONDARY_SURFACE))
    .handle_drop(move |card: &Moving| {
      if card.from != name {
        other.update(|cards| cards.retain(|title| *title != card.title));
        own.update(|cards| cards.push(card.title.clone()));
      }
    })
    .set_child(
      Column::new().set_children((
        Text::new(name)
          .set_bold(true)
          .set_color(SimplePalette::FOREGROUND),
        Column::new().derive_children(
          move || own.get(),
          Clone::clone,
          move |title| card(title, name),
        ),
      )),
    )
}

/// One card, offering itself and its title as a drag.
fn card(title: String, list: &'static str) -> impl IntoView {
  let moving = Moving {
    title: title.clone(),
    from: list,
  };

  Container::new()
    .set_cursor(MouseCursor::Grab)
    .set_color(SimplePalette::CARD.when(State::Dragged, SimplePalette::SECONDARY_SURFACE))
    .set_drag_data(move || DragData::new(moving.clone()).with_text(moving.title.clone()))
    .set_child(Text::new(title).set_color(SimplePalette::FOREGROUND))
}
