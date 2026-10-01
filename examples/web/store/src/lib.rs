//! Stores: one plain value with field-level subscriptions, a list derived from its
//! entries, and a memoized read over it.
//!
//! The todo list is one `Store` holding a plain struct that snapshots as a unit. Each
//! row is built once from a lens onto its own entry, so checking a todo re-inks that
//! row and the count beneath the list, and nothing else runs. The list follows the
//! shape of the `Vec` alone—an addition builds one row, a removal destroys one, a
//! reorder moves them—keyed by the id each todo carries, so state under a row keeps
//! its identity wherever the row sits.

use skene_simple_web::prelude::*;
use skene_web::prelude::*;

#[derive(Store, Clone, Default, Snapshot)]
struct Todos {
  next_id: u32,
  items: Vec<Todo>,
}

#[derive(Store, Clone, Snapshot)]
struct Todo {
  id: u32,
  title: String,
  done: bool,
}

#[main]
fn app() -> impl View {
  let todos = Store::tagged("todos", Todos::default());

  Theme::new(SimplePalette::default()).set_child_builder(move || {
    Container::new()
      .set_background(SimplePalette::current().background)
      .set_child(
        Column::new()
          .set_arrangement(Arrangement::Center)
          .set_item_alignment(ItemAlignment::Center)
          .add_child(card(todos)),
      )
  })
}

fn card(todos: Store<Todos>) -> impl View {
  let draft = Signal::tagged("draft", String::new());
  let remaining = todos
    .items()
    .memo(|items| items.iter().filter(|todo| !todo.done).count());
  let add = move || {
    let title = draft.get();

    if title.trim().is_empty() {
      return;
    }

    let id = todos.next_id().get();

    todos.next_id().set(id + 1);
    todos.items().push(Todo {
      id,
      title,
      done: false,
    });
    draft.set(String::new());
  };

  SimpleCard::new().set_spacing(16.).set_children((
    Text::new("TODOS")
      .set_font_size(13.)
      .set_font_weight(500.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
    Row::new()
      .set_spacing(8.)
      .set_item_alignment(ItemAlignment::Center)
      .set_children((
        Flexible::new()
          .set_flexibility(Flexibility::grow(1.))
          .set_child(
            SimpleTextField::new()
              .set_placeholder("What needs doing?")
              .bind_value(draft),
          ),
        SimpleButton::new("Add").handle_activate(add),
      )),
    Column::new()
      .set_spacing(8.)
      .derive_entries(todos.items().keyed_by(|todo| todo.id), move |todo| {
        row(todos, todo)
      }),
    Text::derive(move || format!("{} left", remaining.get()))
      .set_font_size(13.)
      .set_color(SimplePalette::MUTED_FOREGROUND),
  ))
}

/// One todo's row, following its own entry of `todos`.
fn row(todos: Store<Todos>, todo: impl StoreField<Value = Todo> + Copy) -> impl View {
  Row::new()
    .set_spacing(8.)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      SimpleCheckbox::new()
        .derive_checked(move || todo.done().get())
        .handle_toggle(move |done| todo.done().set(done)),
      Flexible::new()
        .set_flexibility(Flexibility::grow(1.))
        .set_child(
          Text::derive(move || todo.title().get()).derive_color(move || {
            if todo.done().get() {
              SimplePalette::MUTED_FOREGROUND
            }
            else {
              SimplePalette::FOREGROUND
            }
          }),
        ),
      SimpleButton::new("×")
        .set_variant(SimpleVariant::Outline)
        .handle_activate(move || {
          let id = todo.id().get();

          let index = todos
            .items()
            .with(|items| items.iter().position(|todo| todo.id == id));

          if let Some(index) = index {
            let _ = todos.items().remove(index);
          }
        }),
    ))
}
