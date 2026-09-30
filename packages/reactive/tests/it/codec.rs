//! The snapshot encoding under arbitrary input: whatever bytes a restore reads, it
//! returns a value or none and never panics, and a recorded value restores as itself.

#![cfg(not(target_arch = "wasm32"))]

use std::{collections::BTreeMap, time::Duration};

use proptest::prelude::*;
use skene_reactive::{Snapshot, decode, encode, restore};

/// A derived record, its fields keyed by name.
#[derive(Debug, Clone, PartialEq, Snapshot)]
struct Settings {
  name: String,
  volume: u8,
  tags: Vec<String>,
  mode: Mode,
}

/// A derived enum, its variants keyed by name.
#[derive(Debug, Clone, PartialEq, Snapshot)]
enum Mode {
  #[snapshot(default)]
  Light,
  Dark,
  Custom(i32, bool),
}

/// Any settings.
fn settings() -> impl Strategy<Value = Settings> {
  (
    any::<String>(),
    any::<u8>(),
    proptest::collection::vec(any::<String>(), 0..4),
    prop_oneof![
      Just(Mode::Light),
      Just(Mode::Dark),
      (any::<i32>(), any::<bool>()).prop_map(|(hue, bold)| Mode::Custom(hue, bold)),
    ],
  )
    .prop_map(|(name, volume, tags, mode)| Settings {
      name,
      volume,
      tags,
      mode,
    })
}

proptest! {
  /// Arbitrary bytes decode to a value or to none, as every type and as a whole
  /// snapshot, and never panic.
  #[test]
  fn arbitrary_bytes_decode_or_refuse(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
    let _ = decode::<u64>(&bytes);
    let _ = decode::<String>(&bytes);
    let _ = decode::<Vec<Option<i32>>>(&bytes);
    let _ = decode::<BTreeMap<String, (bool, f64)>>(&bytes);
    let _ = decode::<Settings>(&bytes);
    let _ = decode::<Duration>(&bytes);

    restore(&bytes);
  }

  /// Any seconds and nanoseconds restore as a duration or as none: a duration past the
  /// longest one refuses rather than panics.
  #[test]
  fn a_duration_restores_or_refuses(seconds in any::<u64>(), nanoseconds in any::<u32>()) {
    let restored = decode::<Duration>(&encode(&(seconds, nanoseconds)));
    let expected = Duration::from_secs(seconds).checked_add(Duration::from_nanos(u64::from(nanoseconds)));

    prop_assert_eq!(restored, expected);
  }

  /// A recorded value restores as itself.
  #[test]
  fn a_recorded_value_restores_as_itself(
    number in any::<(i64, u64, i128, char)>(),
    text in any::<(String, Vec<u8>, Option<bool>)>(),
    map in proptest::collection::btree_map(any::<String>(), proptest::collection::vec(any::<i32>(), 0..4), 0..4),
    settings in settings(),
  ) {
    prop_assert_eq!(decode(&encode(&number)), Some(number));
    prop_assert_eq!(decode(&encode(&text)), Some(text));
    prop_assert_eq!(decode(&encode(&map)), Some(map));
    prop_assert_eq!(decode(&encode(&settings)), Some(settings));
  }

  /// A finite float restores to the same bits.
  #[test]
  fn a_float_restores_to_its_bits(value in any::<f64>().prop_filter("finite", |value| value.is_finite())) {
    prop_assert_eq!(decode::<f64>(&encode(&value)).map(f64::to_bits), Some(value.to_bits()));
  }
}

/// A present option is tagged, so an option of an option and an option of the unit
/// restore as themselves, and a bare value, as an earlier encoding recorded one,
/// restores as present.
#[test]
fn a_present_option_is_tagged() {
  assert_eq!(
    decode::<Option<Option<u8>>>(&encode(&Some(None::<u8>))),
    Some(Some(None))
  );
  assert_eq!(decode::<Option<()>>(&encode(&Some(()))), Some(Some(())));
  assert_eq!(decode::<Option<u8>>(&encode(&5u8)), Some(Some(5)));
}
