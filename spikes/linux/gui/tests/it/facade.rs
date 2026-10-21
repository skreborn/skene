//! The facade's own surface on the real platform: integer lengths in every constructor,
//! `Apply` from the prelude, a field's keyboard and a button's shortcut, and a keyed list's
//! keys told apart by their own `Eq`.

use std::{
  cell::Cell,
  hash::{Hash, Hasher},
  rc::Rc,
};

use skene_gui::{
  prelude::*,
  testing::{self, Role},
};

fn page() -> impl View {
  let saved = Signal::new(0_u32);
  let email = Signal::new(String::new());
  let note: Option<&str> = Some("Applied");

  Column::new()
    .set_padding(Inset::new(16, 16, 16, 16))
    .set_spacing(Spacing::new(8, 12))
    .add_child(
      TextField::new("Email")
        .set_keyboard(Keyboard::Email)
        .bind_text(email),
    )
    .add_child(
      Button::new("Save")
        .set_shortcut(Shortcut::primary('s'))
        .handle_activate(move || saved.update(|saved| *saved += 1)),
    )
    .add_child(Text::derive(move || format!("Saved {}", saved.get())))
    .add_child(
      Grid::new()
        .set_columns([GridTrack::length(40), GridTrack::Flex(1.)])
        .add_child(
          Container::new()
            .set_label("Swatch")
            .set_width(Dimension::length(40))
            .set_height(24)
            .set_corner_radius(Corners::all(4)),
        ),
    )
    .apply(note, |column, note| column.add_child(Text::new(note)))
}

#[skene_gui::test]
fn a_shortcut_activates_its_button_and_a_reader_reads_it() {
  let screen = testing::render(page);
  let save = screen.find_by_role(Role::Button, "Save");

  assert_eq!(save.shortcut(), "<Control>s");
  screen
    .find_by_role(Role::TextField, "Email")
    .type_text("ada@example.org");
  screen.press("ctrl+s");
  screen.find_by_text("Saved 1");
  assert_eq!(
    screen.find_by_role(Role::TextField, "Email").text(),
    "ada@example.org",
    "the shortcut typed nothing"
  );
}

#[skene_gui::test]
fn integer_lengths_lay_out_as_pixels_and_apply_adds_what_is_given() {
  let screen = testing::render(page);
  let swatch = screen.find_by_role(Role::Region, "Swatch").read().bounds;

  assert_eq!((swatch.width, swatch.height), (40., 24.));
  assert_eq!(swatch.x, 16.);
  screen.find_by_text("Applied");
}

/// A row's key: its id and its name, hashed by its id alone, as `Hash` may be.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Coarse {
  id: u32,
  name: &'static str,
}

impl Hash for Coarse {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.id.hash(state);
  }
}

/// A keyed column of `items`, each row counted in `builds` as it is built.
fn keyed(items: Signal<Vec<(u32, &'static str)>>, builds: Rc<Cell<u32>>) -> impl View {
  Column::new().set_padding(16).derive_children(
    move || items.get(),
    |&(id, name)| Coarse { id, name },
    move |(id, name)| {
      builds.set(builds.get() + 1);

      Text::new(format!("Row {id} {name}"))
    },
  )
}

#[skene_gui::test]
fn a_keyed_list_tells_its_keys_apart_by_their_own_eq() {
  let items = Signal::new(vec![(1, "a"), (1, "b")]);
  let builds = Rc::new(Cell::new(0));
  let screen = testing::render({
    let builds = Rc::clone(&builds);

    move || keyed(items, builds)
  });

  screen.find_by_text("Row 1 a");
  screen.find_by_text("Row 1 b");
  assert_eq!(builds.get(), 2);

  items.set(vec![(1, "a"), (1, "b")]);
  screen.settle();
  items.set(vec![(1, "b"), (1, "a"), (2, "c")]);
  screen.settle();
  screen.find_by_text("Row 2 c");
  assert_eq!(
    builds.get(),
    3,
    "two keys hashing alike and unequal are two, each built once and kept"
  );
}
