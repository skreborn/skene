//! Fine-grained store subscriptions: field-, index-, and key-level.

use std::{
  cell::{Cell, RefCell},
  collections::{BTreeMap, HashMap},
  rc::Rc,
};

use skene_reactive::{
  AtKey,
  Effect,
  Get,
  IndexedField,
  KeyedField,
  Owner,
  Snapshot,
  Store,
  StoreField,
};

#[derive(Store, Default, Snapshot)]
struct Settings {
  name: String,
  volume: u32,
  audio: Audio,
  favorites: Vec<String>,
  users: HashMap<u32, User>,
}

#[derive(Store, Default, Snapshot)]
struct Audio {
  gain: u32,
}

#[derive(Store, Default, Snapshot, Clone)]
struct User {
  score: u32,
}

/// An effect counting its runs while reading through `read`.
fn count_runs(read: impl Fn() + 'static) -> Rc<Cell<u32>> {
  let runs = Rc::new(Cell::new(0));

  {
    let runs = Rc::clone(&runs);

    Effect::new(move |_| {
      runs.set(runs.get() + 1);
      read();
    });
  }

  runs
}

/// A write to one field wakes that field's readers and nobody else's.
#[test]
fn field_writes_wake_only_their_readers() {
  let settings = Store::new(Settings::default());
  let volume_runs = count_runs(move || {
    let _ = settings.volume().get();
  });
  let name_runs = count_runs(move || {
    settings.name().with(|_| {});
  });

  assert_eq!((volume_runs.get(), name_runs.get()), (1, 1));

  settings.volume().update(|volume| *volume += 1);

  assert_eq!((volume_runs.get(), name_runs.get()), (2, 1));

  settings.name().set("Ada".to_owned());

  assert_eq!((volume_runs.get(), name_runs.get()), (2, 2));
  assert_eq!(settings.volume().get(), 1);
}

/// A whole-store reader wakes on any field write, and a field reader wakes when the
/// whole value is replaced.
#[test]
fn ancestors_and_descendants_wake() {
  let settings = Store::new(Settings::default());
  let whole_runs = count_runs(move || settings.with(|_| {}));
  let volume_runs = count_runs(move || {
    let _ = settings.volume().get();
  });

  settings.volume().set(3);

  assert_eq!((whole_runs.get(), volume_runs.get()), (2, 2));

  settings.set(Settings::default());

  assert_eq!((whole_runs.get(), volume_runs.get()), (3, 3));
  assert_eq!(settings.volume().get(), 0);
}

/// Nested struct fields compose lenses, with the same granularity.
#[test]
fn nested_fields_stay_fine_grained() {
  let settings = Store::new(Settings::default());
  let gain_runs = count_runs(move || {
    let _ = settings.audio().gain().get();
  });

  settings.volume().set(9);

  assert_eq!(gain_runs.get(), 1);

  settings.audio().gain().set(2);

  assert_eq!(gain_runs.get(), 2);
  assert_eq!(settings.audio().gain().get(), 2);
}

/// Vec elements address by position: element writes stay element-local, and a
/// structural change wakes every element reader.
#[test]
fn vec_indices_are_fine_grained_until_structure_changes() {
  let settings = Store::new(Settings {
    favorites: vec!["a".to_owned(), "b".to_owned()],
    ..Settings::default()
  });
  let first_runs = count_runs(move || {
    let _ = settings.favorites().at(0).try_get();
  });
  let second_runs = count_runs(move || {
    let _ = settings.favorites().at(1).try_get();
  });

  settings.favorites().at(1).set("B".to_owned());

  assert_eq!((first_runs.get(), second_runs.get()), (1, 2));

  settings
    .favorites()
    .update(|favorites| favorites.push("c".to_owned()));

  assert_eq!((first_runs.get(), second_runs.get()), (2, 3));
  assert_eq!(settings.favorites().at(1).get(), "B");
}

/// A push wakes the length's and the whole vector's readers and no element's; an insert
/// and a removal wake the elements from their position on and none before it.
#[test]
fn shape_writes_wake_only_the_elements_they_move() {
  let settings = Store::new(Settings {
    favorites: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
    ..Settings::default()
  });
  let first_runs = count_runs(move || {
    let _ = settings.favorites().at(0).try_get();
  });
  let third_runs = count_runs(move || {
    let _ = settings.favorites().at(2).try_get();
  });
  let length_runs = count_runs(move || {
    let _ = settings.favorites().len();
  });
  let whole_runs = count_runs(move || {
    let _ = settings.favorites().get();
  });
  let runs = || {
    (
      first_runs.get(),
      third_runs.get(),
      length_runs.get(),
      whole_runs.get(),
    )
  };

  settings.favorites().push("d".to_owned());

  assert_eq!(runs(), (1, 1, 2, 2), "a push moves no element");
  assert_eq!(settings.favorites().len(), 4);

  settings.favorites().insert(1, "x".to_owned());

  assert_eq!(
    runs(),
    (1, 2, 3, 3),
    "an insert moves the elements after it"
  );
  assert_eq!(settings.favorites().at(2).get(), "b");

  assert_eq!(settings.favorites().remove(2).as_deref(), Some("b"));
  assert_eq!(
    runs(),
    (1, 3, 4, 4),
    "a removal moves the elements after it"
  );

  settings.favorites().at(0).set("A".to_owned());

  assert_eq!(
    runs(),
    (2, 3, 4, 5),
    "an element's write leaves the length asleep"
  );
}

/// A keyed reconcile wakes each element whose value changed where the keys keep their
/// order, and the shape with every moved position where they do not.
#[test]
fn a_keyed_reconcile_wakes_what_changed() {
  #[derive(Store, Clone, PartialEq)]
  struct Row {
    id: u32,
    label: &'static str,
  }

  let rows = Store::transient(vec![
    Row { id: 1, label: "a" },
    Row { id: 2, label: "b" },
    Row { id: 3, label: "c" },
  ]);
  let keyed = rows.keyed_by(|row| row.id);
  let label_runs = |id: u32| {
    count_runs(move || {
      let _ = keyed.key(id).try_with(|row| row.label);
    })
  };
  let (first, second, third) = (label_runs(1), label_runs(2), label_runs(3));
  let shape_runs = count_runs(move || {
    let _ = keyed.entries();
  });
  let runs = || (first.get(), second.get(), third.get(), shape_runs.get());

  keyed.reconcile(vec![
    Row { id: 1, label: "a" },
    Row { id: 2, label: "B" },
    Row { id: 3, label: "c" },
  ]);

  assert_eq!(runs(), (1, 2, 1, 1), "one changed value wakes one row");

  keyed.reconcile(vec![
    Row { id: 1, label: "a" },
    Row { id: 3, label: "c" },
    Row { id: 2, label: "B" },
  ]);

  assert_eq!(
    runs(),
    (1, 3, 2, 2),
    "a reorder wakes the shape and the moved rows"
  );
  assert_eq!(keyed.key(2).label().get(), "B");
}

/// A read of a key no element carries wakes once a push, an insert or a reconcile adds
/// one that does.
#[test]
fn an_absent_key_wakes_when_a_shape_write_adds_it() {
  #[derive(Store, Clone, PartialEq)]
  struct Row {
    id: u32,
    label: &'static str,
  }

  let rows = Store::transient(vec![Row { id: 1, label: "a" }]);
  let keyed = rows.keyed_by(|row| row.id);
  let seen = Rc::new(RefCell::new(Vec::new()));

  {
    let seen = Rc::clone(&seen);

    Effect::new(move |_| {
      seen
        .borrow_mut()
        .push([2, 3, 4].map(|id| keyed.key(id).try_with(|row| row.label)));
    });
  }

  rows.push(Row { id: 2, label: "b" });
  rows.insert(0, Row { id: 3, label: "c" });
  keyed.reconcile(vec![
    Row { id: 3, label: "c" },
    Row { id: 1, label: "a" },
    Row { id: 2, label: "b" },
    Row { id: 4, label: "d" },
  ]);

  assert_eq!(
    *seen.borrow(),
    [
      [None, None, None],
      [Some("b"), None, None],
      [Some("b"), Some("c"), None],
      [Some("b"), Some("c"), Some("d")],
    ]
  );
}

/// Map entries address by key; a missing key reads `None` and writes nothing.
#[test]
fn map_keys_are_fine_grained_and_fallible() {
  let mut users = HashMap::new();

  users.insert(1, User { score: 10 });
  users.insert(2, User { score: 20 });

  let settings = Store::new(Settings {
    users,
    ..Settings::default()
  });
  let first_runs = count_runs(move || {
    let _ = settings.users().key(1).try_with(|user| user.score);
  });

  settings.users().key(2).score().set(25);

  assert_eq!(first_runs.get(), 1);

  settings.users().key(1).score().update(|score| *score += 1);

  assert_eq!(first_runs.get(), 2);
  assert_eq!(settings.users().key(1).score().get(), 11);

  assert_eq!(settings.users().key(9).try_with(|_| ()), None);
  assert_eq!(settings.users().key(9).try_update(|_| ()), None);

  settings.users().update(|users| {
    users.remove(&1);
  });

  assert_eq!(settings.users().key(1).try_with(|user| user.score), None);
}

/// Two keys with the same hash address distinct locations: the key itself, not its
/// hash, is the identity.
#[test]
fn colliding_key_hashes_never_alias() {
  #[derive(Clone, PartialEq, Eq)]
  struct Colliding(u8);

  impl std::hash::Hash for Colliding {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
      0u8.hash(state);
    }
  }

  let mut entries = HashMap::new();

  entries.insert(Colliding(1), 1u32);
  entries.insert(Colliding(2), 2u32);

  let store = Store::transient(entries);
  let first_runs = count_runs(move || {
    let _ = store.key(Colliding(1)).try_get();
  });

  store.key(Colliding(2)).set(20);

  assert_eq!(first_runs.get(), 1);
  assert_eq!(store.key(Colliding(1)).get(), 1);
  assert_eq!(store.key(Colliding(2)).get(), 20);
}

/// The derive covers generics, tuple structs, skipped and tagged fields, and fields
/// named after the location surface.
#[test]
fn derive_surface() {
  #[derive(Store, Snapshot)]
  struct Pair<T> {
    first: T,
    #[store(tag = "other")]
    second: T,
    #[store(skip)]
    scratch: u8,
  }

  #[derive(Store, Snapshot)]
  struct Point(i32, i32);

  #[derive(Store, Snapshot)]
  struct Awkward {
    get: u32,
    set: u32,
    with: u32,
    update: u32,
    memo: u32,
  }

  let pair = Store::new(Pair {
    first: 1u32,
    second: 2,
    scratch: 0,
  });
  let point = Store::new(Point(3, 4));
  let awkward = Store::new(Awkward {
    get: 1,
    set: 2,
    with: 3,
    update: 4,
    memo: 5,
  });

  pair.first().set(10);
  point.field_1().set(40);
  awkward.get_field().set(10);
  awkward.set_field().set(20);

  assert_eq!((pair.first().get(), pair.second().get()), (10, 2));
  assert_eq!((point.field_0().get(), point.field_1().get()), (3, 40));
  assert_eq!(
    (
      awkward.get_field().get(),
      awkward.set_field().get(),
      awkward.with_field().get(),
      awkward.update_field().get(),
      awkward.memo_field().get(),
    ),
    (10, 20, 3, 4, 5)
  );
}

/// A location's memo recomputes on a write there, and its readers wake only when the
/// result changed.
#[test]
fn a_location_memo_wakes_readers_only_on_a_changed_result() {
  let settings = Store::new(Settings {
    favorites: vec!["a".to_owned(), "b".to_owned()],
    ..Settings::default()
  });
  let count = settings.favorites().memo(Vec::len);
  let reader_runs = count_runs(move || {
    let _ = count.get();
  });

  settings.favorites().at(0).set("A".to_owned());

  assert_eq!((count.get(), reader_runs.get()), (2, 1));

  settings
    .favorites()
    .update(|favorites| favorites.push("c".to_owned()));

  assert_eq!((count.get(), reader_runs.get()), (3, 2));
}

/// A fallible memo is `None` once its location is gone.
#[test]
fn a_fallible_memo_is_none_once_the_location_is_gone() {
  let mut users = HashMap::new();

  users.insert(1, User { score: 10 });

  let settings = Store::new(Settings {
    users,
    ..Settings::default()
  });
  let score = settings.users().key(1).try_memo(|user| user.score);

  assert_eq!(score.get(), Some(10));

  settings.users().key(1).score().set(11);

  assert_eq!(score.get(), Some(11));

  settings.users().update(|users| {
    users.remove(&1);
  });

  assert_eq!(score.get(), None);
}

/// The entries of a keyed location follow its shape alone: an entry written beneath
/// wakes the entry's readers and not the iteration, an insertion wakes the iteration.
#[test]
fn entries_follow_the_shape_and_each_lens_its_entry() {
  let mut users = BTreeMap::new();

  users.insert(1, User { score: 10 });
  users.insert(2, User { score: 20 });

  let store = Store::transient(users);
  let seen = Rc::new(RefCell::new(Vec::new()));
  let entries_runs = {
    let seen = Rc::clone(&seen);

    count_runs(move || {
      *seen.borrow_mut() = store.entries().iter().map(AtKey::entry_key).collect();
    })
  };
  let first = store.entries().into_iter().next().expect("the first entry");
  let first_runs = count_runs(move || {
    let _ = first.score().get();
  });

  assert_eq!(*seen.borrow(), [1, 2]);

  store.key(2).score().set(25);

  assert_eq!((entries_runs.get(), first_runs.get()), (1, 1));

  store.key(1).score().set(15);

  assert_eq!((entries_runs.get(), first_runs.get()), (1, 2));

  store.update(|users| {
    users.insert(3, User { score: 30 });
  });

  assert_eq!((entries_runs.get(), first_runs.get()), (2, 3));
  assert_eq!(*seen.borrow(), [1, 2, 3]);
}

/// A `Vec` keyed by its elements reads by key across a reorder, and its entries come
/// in the `Vec`'s order.
#[test]
fn a_vec_keyed_by_its_elements_reads_by_key_across_a_reorder() {
  #[derive(Store, Clone)]
  struct Row {
    id: u32,
    label: &'static str,
  }

  let rows = Store::transient(vec![Row { id: 1, label: "a" }, Row { id: 2, label: "b" }]);
  let keyed = rows.keyed_by(|row| row.id);
  let order = keyed.memo(|rows| rows.iter().map(|row| row.id).collect::<Vec<_>>());
  let second_runs = count_runs(move || {
    let _ = keyed.key(2).label().get();
  });

  assert_eq!(
    keyed
      .entries()
      .iter()
      .map(AtKey::entry_key)
      .collect::<Vec<_>>(),
    [1, 2]
  );

  rows.update(|rows| rows.swap(0, 1));

  assert_eq!(order.get(), [2, 1]);
  assert_eq!(
    keyed
      .entries()
      .iter()
      .map(AtKey::entry_key)
      .collect::<Vec<_>>(),
    [2, 1]
  );
  assert_eq!(keyed.key(2).label().get(), "b");
  assert_eq!(second_runs.get(), 2);

  keyed.key(2).label().set("B");

  assert_eq!((second_runs.get(), rows.at(0).label().get()), (3, "B"));
  assert_eq!(keyed.key(9).try_with(|_| ()), None);
}

/// A keyed read follows every change to the vector: a push, a removal, and an element
/// whose key itself changed.
#[test]
fn keyed_reads_follow_pushes_removals_and_changed_keys() {
  #[derive(Store, Clone)]
  struct Row {
    id: u32,
    label: &'static str,
  }

  let rows = Store::transient(vec![Row { id: 1, label: "a" }, Row { id: 2, label: "b" }]);
  let keyed = rows.keyed_by(|row| row.id);

  assert_eq!(keyed.key(2).label().get(), "b");

  rows.update(|rows| rows.push(Row { id: 3, label: "c" }));

  assert_eq!(keyed.key(3).label().get(), "c");

  rows.update(|rows| {
    rows.remove(0);
  });

  assert_eq!(keyed.key(2).label().get(), "b");
  assert_eq!(keyed.key(1).try_with(|_| ()), None);

  keyed.key(2).id().set(7);

  assert_eq!(keyed.key(7).label().get(), "b");
  assert_eq!(keyed.key(2).try_with(|_| ()), None);
}

/// An element addressed by key and the same element addressed by position are one
/// location: a write through either wakes the readers through both, before and after a
/// reorder moves the element.
#[test]
fn a_keyed_element_and_its_position_are_one_location() {
  #[derive(Store, Clone)]
  struct Row {
    id: u32,
    label: &'static str,
  }

  let rows = Store::transient(vec![Row { id: 1, label: "a" }, Row { id: 2, label: "b" }]);
  let keyed = rows.keyed_by(|row| row.id);
  let by_key_runs = count_runs(move || {
    let _ = keyed.key(2).label().get();
  });
  let by_position_runs = count_runs(move || {
    let _ = rows.at(1).label().get();
  });
  let other_key_runs = count_runs(move || {
    let _ = keyed.key(1).label().get();
  });

  rows.at(1).label().set("B");

  assert_eq!(
    (
      by_key_runs.get(),
      by_position_runs.get(),
      other_key_runs.get()
    ),
    (2, 2, 1),
    "a write by position woke the reader by key of the same element, and no other"
  );

  keyed.key(2).label().set("bee");

  assert_eq!(
    (
      by_key_runs.get(),
      by_position_runs.get(),
      other_key_runs.get()
    ),
    (3, 3, 1),
    "a write by key woke the reader by position of the same element, and no other"
  );

  rows.update(|rows| rows.swap(0, 1));

  assert_eq!(
    (
      by_key_runs.get(),
      by_position_runs.get(),
      other_key_runs.get()
    ),
    (4, 4, 2),
    "a structural write woke every reader"
  );

  keyed.key(2).label().set("BEE");

  assert_eq!(
    (
      by_key_runs.get(),
      by_position_runs.get(),
      other_key_runs.get()
    ),
    (5, 4, 2),
    "after the reorder the key's location moved with its element, away from position 1"
  );

  rows.at(0).label().set("z");

  assert_eq!(
    (
      by_key_runs.get(),
      by_position_runs.get(),
      other_key_runs.get()
    ),
    (6, 4, 2),
    "and position 0, where the element now sits, wakes the reader by key"
  );
  assert_eq!(keyed.key(9).try_with(|_| ()), None);
}

/// A store snapshots its whole value as one unit at its position and adopts it in the
/// next run.
#[test]
fn store_round_trips_through_a_snapshot() {
  let bytes = std::thread::spawn(|| {
    skene_reactive::record_snapshots();

    let scores = Store::new(vec![0u32; 3]);

    scores.at(1).set(7);

    skene_reactive::snapshot()
  })
  .join()
  .expect("the first run");

  let restored = std::thread::spawn(move || {
    skene_reactive::restore(&bytes);

    let scores = Store::new(Vec::<u32>::new());

    scores.at(1).get()
  })
  .join()
  .expect("the next run");

  assert_eq!(restored, 7);
}

/// A tagged store snapshots under its tag, whatever its position.
#[test]
fn tagged_store_round_trips_through_a_snapshot() {
  let bytes = std::thread::spawn(|| {
    skene_reactive::record_snapshots();

    let scores = Store::tagged("store-scores", vec![0u32; 3]);

    scores.at(1).set(7);

    skene_reactive::snapshot()
  })
  .join()
  .expect("the first run");

  let restored = std::thread::spawn(move || {
    skene_reactive::restore(&bytes);

    let _shift = Store::new(0u8);
    let scores = Store::tagged("store-scores", Vec::<u32>::new());

    scores.at(1).get()
  })
  .join()
  .expect("the next run");

  assert_eq!(restored, 7);
}

/// Editing one row of ten thousand tracked rows reruns that row's effect on the edited
/// field and no other.
#[test]
fn a_row_edit_reruns_only_its_own_effect() {
  #[derive(Store, Clone, Default)]
  struct Row {
    label: u32,
    checked: bool,
  }

  const ROWS: usize = 10_000;

  let runs = Rc::new(Cell::new(0usize));
  let scope = Owner::new();
  let store = scope.enter(|| {
    let store = Store::transient(vec![Row::default(); ROWS]);

    for index in 0..ROWS {
      let label = Rc::clone(&runs);
      let checked = Rc::clone(&runs);

      Effect::new(move |_| {
        let _ = store.at(index).label().get();

        label.set(label.get() + 1);
      });
      Effect::new(move |_| {
        let _ = store.at(index).checked().get();

        checked.set(checked.get() + 1);
      });
    }

    store
  });

  runs.set(0);

  for edit in 1..=100 {
    store.at(ROWS / 2).label().set(edit);
  }

  assert_eq!(runs.get(), 100);

  scope.dispose();
}
