//! Fine-grained store subscriptions: field-, index-, and key-level.

use std::{
  cell::Cell,
  collections::HashMap,
  rc::Rc,
  time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use skene_reactive::{Effect, IndexedField, KeyedField, Owner, Store, StoreField};

#[derive(Store, Default, Serialize, Deserialize)]
struct Settings {
  name: String,
  volume: u32,
  audio: Audio,
  favorites: Vec<String>,
  users: HashMap<u32, User>,
}

#[derive(Store, Default, Serialize, Deserialize)]
struct Audio {
  gain: u32,
}

#[derive(Store, Default, Serialize, Deserialize, Clone)]
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

  let store = Store::opaque(entries);
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
  #[derive(Store, Serialize, Deserialize)]
  struct Pair<T> {
    first: T,
    #[store(tag = "other")]
    second: T,
    #[store(skip)]
    scratch: u8,
  }

  #[derive(Store, Serialize, Deserialize)]
  struct Point(i32, i32);

  #[derive(Store, Serialize, Deserialize)]
  struct Awkward {
    get: u32,
    set: u32,
    with: u32,
    update: u32,
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
    ),
    (10, 20, 3, 4)
  );
}

/// A store snapshots its whole value as one unit at its position and adopts it in the
/// next run.
#[test]
fn store_round_trips_through_a_snapshot() {
  let bytes = std::thread::spawn(|| {
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

/// The time a burst of row edits takes over a store of `rows` tracked cells.
fn edit_time(rows: usize, edits: u32) -> Duration {
  #[derive(Store, Clone, Default)]
  struct Row {
    label: u32,
    checked: bool,
  }

  let scope = Owner::new();
  let store = scope.enter(|| {
    let store = Store::opaque(vec![Row::default(); rows]);

    for index in 0..rows {
      Effect::new(move |_| {
        let _ = store.at(index).label().get();
      });
      Effect::new(move |_| {
        let _ = store.at(index).checked().get();
      });
    }

    store
  });
  let start = Instant::now();

  for edit in 0..edits {
    store.at(rows / 2).label().set(edit);
  }

  let elapsed = start.elapsed();

  scope.dispose();

  elapsed
}

/// Editing one row notifies in time independent of how many rows are tracked: ten
/// thousand tracked cells cost no more per edit than a hundred, within noise.
#[test]
fn row_edit_cost_is_independent_of_tracked_count() {
  let _warm = edit_time(100, 100);
  let small = edit_time(100, 2_000);
  let large = edit_time(10_000, 2_000);

  assert!(
    large < small * 8 + Duration::from_millis(20),
    "an edit over 10,000 tracked cells took {large:?} against {small:?} over 200"
  );
}
