//! Fine-grained store subscriptions: field-, index-, and key-level.

use std::{cell::Cell, collections::HashMap, rc::Rc};

use skene_reactive::{Effect, IndexedField, KeyedField, Store, StoreField};

#[derive(Store, Default)]
struct Settings {
  name: String,
  volume: u32,
  audio: Audio,
  favorites: Vec<String>,
  users: HashMap<u32, User>,
}

#[derive(Store, Default)]
struct Audio {
  gain: u32,
}

#[derive(Store, Default)]
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

/// A tagged store snapshots its whole value under one tag and adopts it in the next
/// run.
#[cfg(feature = "snapshot")]
#[test]
fn tagged_store_round_trips_through_a_snapshot() {
  let bytes = {
    let scores = Store::tagged("store-scores", vec![0u32; 3]);

    scores.at(1).set(7);

    skene_reactive::snapshot()
  };

  let restored = std::thread::spawn(move || {
    skene_reactive::restore(&bytes);

    let scores = Store::tagged("store-scores", Vec::<u32>::new());

    scores.at(1).get()
  })
  .join()
  .expect("the next run");

  assert_eq!(restored, 7);
}
