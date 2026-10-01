//! Drag and drop: cards moving between two lists, each card offering itself as a drag
//! and each list taking the cards dropped on it.
//!
//! A box offers what a drag from it carries (`set_drag_data`), and a box takes the
//! drags carrying a value of its type (`handle_drop`). The card being dragged holds
//! `Dragged` and the list under the pointer that would take it `Targeted`, so both are
//! styled as any state is.

use skene_simple_web::prelude::*;
use skene_web::prelude::*;

/// A card on its way from one list to another: its title and the list it leaves.
#[derive(Clone)]
struct Moving {
  title: String,
  from: &'static str,
}

#[main]
fn app() -> impl View {
  let todo = Signal::tagged(
    "todo",
    ["Sketch the layout", "Pick the palette", "Write the copy"]
      .map(str::to_owned)
      .to_vec(),
  );
  let done = Signal::tagged("done", vec![String::from("Set up the project")]);

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_color(SimplePalette::BACKGROUND)
      .set_min_height(Dimension::Fraction(1.))
      .set_child(
        Column::new()
          .set_item_alignment(ItemAlignment::Center)
          .set_padding(Inset::symmetric(24., 48.))
          .add_child(
            Column::new().set_spacing(24.).set_children((
              heading(),
              Row::new()
                .set_spacing(16.)
                .set_children((list("To do", todo, done), list("Done", done, todo))),
            )),
          ),
      )
  })
}

fn heading() -> impl View {
  Column::new().set_spacing(6.).set_children((
    Text::new("DRAG AND DROP")
      .set_font_size(13.)
      .set_font_weight(500.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Text::new("Drag a card to the other list.").set_font_size(15.),
  ))
}

/// The cards in `own`, taking a card dropped on the list from `other`.
fn list(name: &'static str, own: Signal<Vec<String>>, other: Signal<Vec<String>>) -> impl View {
  Container::new()
    .set_width(220.)
    .set_padding(Inset::all(12.))
    .set_corner_radius(8.)
    .set_color(SimplePalette::CARD.when(State::Targeted, SimplePalette::SECONDARY_SURFACE))
    .handle_drop(move |card: &Moving| {
      if card.from != name {
        other.update(|cards| cards.retain(|title| *title != card.title));
        own.update(|cards| cards.push(card.title.clone()));
      }
    })
    .set_child(Column::new().set_spacing(8.).set_children((
      Text::new(name).set_font_weight(600.),
      Column::new().set_spacing(8.).derive_children(
        move || own.get(),
        Clone::clone,
        move |title| card(title, name),
      ),
    )))
}

/// One card, offering itself and its title as a drag.
fn card(title: String, list: &'static str) -> impl View {
  let moving = Moving {
    title: title.clone(),
    from: list,
  };

  Container::new()
    .set_padding(Inset::symmetric(12., 8.))
    .set_corner_radius(6.)
    .set_cursor(MouseCursor::Grab)
    .set_color(SimplePalette::BACKGROUND.when(State::Dragged, SimplePalette::SECONDARY_SURFACE))
    .set_drag_data(move || DragData::new(moving.clone()).with_text(moving.title.clone()))
    .set_child(Text::new(title))
}
