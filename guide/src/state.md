# State

State lives in signals, and everything that reads one follows it: a derived property,
a derived child, a memo, an effect. A write reaches exactly what read the value, and
nothing else runs.

```rust
use skene_reactive::{Effect, Get, Memo, Owner, Set, Signal, Update};

let owner = Owner::new();

owner.enter(|| {
  let count = Signal::new(1u64);
  let doubled = Memo::new(move |_| count.get() * 2);

  Effect::new(move |_| println!("doubled: {}", doubled.get()));

  count.set(2);
  count.update(|count| *count += 1);
});

owner.dispose();
```

Every signal, memo and effect belongs to the owner it was created under, and goes when
that owner is disposed. A view builds under the owner of its parent, so a subtree's
state goes with the subtree.

## State a shell shares

An app's logic sits in its shared crate, as types holding signals, knowing nothing of
any platform. Every shell builds its views over the same type:

```rust
use skene_reactive::{Get, Signal, Update};

/// A count that survives a dev reload.
#[derive(Clone, Copy)]
pub struct Counter {
  count: Signal<u64>,
}

impl Counter {
  /// A counter at zero.
  pub fn new() -> Self {
    Self {
      count: Signal::tagged("count", 0),
    }
  }

  /// The count, tracked.
  pub fn count(self) -> u64 {
    self.count.get()
  }

  /// Adds one.
  pub fn increment(self) {
    self.count.update(|count| *count += 1);
  }
}
```

## Stores

A `Store` holds one plain value, a struct deriving `Store`, and hands out a lens per
field, so a write to one field reaches what read that field alone. A list's entries are
lenses too, keyed by what identifies an entry:

```rust
use skene_web::prelude::*;

#[derive(Store, Clone, Default, Snapshot)]
struct Todos {
  items: Vec<Todo>,
}

#[derive(Store, Clone, Snapshot)]
struct Todo {
  id: u32,
  title: String,
  done: bool,
}

fn list(todos: Store<Todos>) -> impl View {
  Column::new().set_spacing(4).derive_entries(
    todos.items().keyed_by(|todo| todo.id),
    |todo| {
      Text::derive(move || todo.title().get()).derive_color(move || {
        if todo.done().get() {
          Color::srgb(0.5, 0.5, 0.5)
        }
        else {
          Color::srgb(0.1, 0.1, 0.1)
        }
      })
    },
  )
}
```

Checking one todo re-inks its row alone; adding one builds one row, and removing one
destroys one.

## Across a reload

A dev reload carries the signals an app opts in into the rebuilt app. `Signal::saved`
and `Store::saved` opt in by place: each is matched by its place among the saved and
tagged signals of its owner. `Signal::tagged` and `Store::tagged` opt in by name,
anchoring one by its tag, so one added before it shifts nothing; and a keyed child's
owner is matched by its key, so state stays with its item wherever the item moves. A
value restores through `Snapshot`, which the built-in types implement and a type of the
app derives. A plain `Signal::new` asks nothing of its type, takes no place and starts
afresh after a reload. None of this costs a release build anything.

```rust
use skene_reactive::{Signal, Snapshot};

#[derive(Snapshot, Default)]
struct Filter {
  query: String,
  done_only: bool,
}

let filter = Signal::saved(Filter::default());
let draft = Signal::tagged("draft", String::new());
let hovered = Signal::new(None::<u32>);
```
