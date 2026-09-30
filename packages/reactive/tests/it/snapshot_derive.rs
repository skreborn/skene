//! The `Snapshot` derive: a struct restores by field key across reordering, renaming
//! through a tag, an added or removed field and a changed kind; an enum restores by
//! variant, falling back to its default variant where the recorded one is gone.

use skene_reactive::{Snapshot, decode, encode};

#[derive(Snapshot, Debug, PartialEq)]
struct Settings {
  volume: u32,
  muted: bool,
  name: String,
  #[snapshot(skip)]
  scratch: u64,
}

/// The same settings after an edit: fields reordered, one renamed under its old tag,
/// one added, one removed, one changed in kind.
#[derive(Snapshot, Debug, PartialEq)]
struct SettingsLater {
  #[snapshot(tag = "name")]
  label: String,
  muted: u8,
  added: Option<f32>,
  volume: f64,
}

#[derive(Snapshot, Debug, PartialEq)]
enum Mode {
  #[snapshot(default)]
  Idle,
  Counting(u32),
  Named {
    first: String,
    second: bool,
  },
}

#[derive(Snapshot, Debug, PartialEq)]
enum ModeLater {
  #[snapshot(default)]
  Idle,
  Named {
    first: String,
    third: u16,
  },
}

#[derive(Snapshot, Debug, PartialEq)]
struct Wrapper<T>(T, Vec<T>);

/// A struct round-trips, its skipped field absent.
#[test]
fn a_struct_round_trips() {
  let settings = Settings {
    volume: 7,
    muted: true,
    name: "left".to_owned(),
    scratch: 99,
  };

  assert_eq!(
    decode::<Settings>(&encode(&settings)),
    Some(Settings {
      scratch: 0,
      ..settings
    })
  );
}

/// An edited struct restores what survives: by tag, by coercion, absent where new.
#[test]
fn an_edited_struct_restores_what_survives() {
  let settings = Settings {
    volume: 7,
    muted: true,
    name: "left".to_owned(),
    scratch: 0,
  };

  assert_eq!(
    decode::<SettingsLater>(&encode(&settings)),
    Some(SettingsLater {
      label: "left".to_owned(),
      muted: 1,
      added: None,
      volume: 7.,
    })
  );
}

/// An enum round-trips through every shape of variant.
#[test]
fn an_enum_round_trips() {
  for mode in [
    Mode::Idle,
    Mode::Counting(4),
    Mode::Named {
      first: "a".to_owned(),
      second: true,
    },
  ] {
    let bytes = encode(&mode);

    assert_eq!(decode::<Mode>(&bytes), Some(mode));
  }
}

/// A variant that is gone restores to the default variant; one that changed restores
/// its surviving fields.
#[test]
fn an_edited_enum_falls_back() {
  assert_eq!(
    decode::<ModeLater>(&encode(&Mode::Counting(4))),
    Some(ModeLater::Idle)
  );
  assert_eq!(
    decode::<ModeLater>(&encode(&Mode::Named {
      first: "a".to_owned(),
      second: true,
    })),
    Some(ModeLater::Named {
      first: "a".to_owned(),
      third: 0,
    })
  );
  assert_eq!(<ModeLater as Snapshot>::absent(), Some(ModeLater::Idle));
}

/// A generic struct derives with its parameter bounded.
#[test]
fn a_generic_struct_derives() {
  let wrapper = Wrapper(3u8, vec![1, 2]);

  assert_eq!(decode::<Wrapper<u8>>(&encode(&wrapper)), Some(wrapper));
  assert_eq!(
    decode::<Wrapper<u32>>(&encode(&Wrapper(3u8, vec![1, 2]))),
    Some(Wrapper(3u32, vec![1, 2]))
  );
}

/// A value that takes no part in snapshots, only a default.
#[derive(Debug, Default, PartialEq)]
struct Cache(Vec<u8>);

#[derive(Snapshot, Debug, PartialEq)]
struct Cached {
  count: u32,
  #[snapshot(skip)]
  cache: Cache,
}

/// A skipped field needs only a default, never `Snapshot`, and restores as it.
#[test]
fn a_skipped_field_restores_its_default() {
  let cached = Cached {
    count: 4,
    cache: Cache(vec![1]),
  };

  assert_eq!(
    decode::<Cached>(&encode(&cached)),
    Some(Cached {
      count: 4,
      cache: Cache::default(),
    })
  );
}

/// The derive names `Option` and its variants in full, so a scope shadowing `Some` and
/// `None` derives as any other.
mod shadowing {
  use skene_reactive::{Snapshot, decode, encode};

  #[derive(Debug, PartialEq)]
  enum Presence {
    None,
    Some,
  }

  use Presence::{None, Some};

  #[derive(Snapshot, Debug, PartialEq)]
  struct Point {
    x: i32,
  }

  #[derive(Snapshot, Debug, PartialEq)]
  enum Switch {
    #[snapshot(default)]
    Off,
    On(u8),
  }

  #[test]
  fn a_scope_shadowing_the_option_variants_derives() {
    assert_ne!(None, Some);
    assert_eq!(
      decode::<Point>(&encode(&Point { x: 3 })),
      Option::Some(Point { x: 3 })
    );
    assert_eq!(
      decode::<Switch>(&encode(&Switch::On(2))),
      Option::Some(Switch::On(2))
    );
  }
}
