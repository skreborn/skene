//! The state a snapshot carries, value by value: the [`Snapshot`] trait every snapshot
//! signal's type implements, the lenient encoding it records into, and the reader that
//! restores from it.
//!
//! The encoding is self-describing—every value carries its kind—and a restore coerces
//! across kinds wherever a value plainly survives: a boolean and an integer are one
//! another, an integer and a float are, a missing field takes its type's absent value
//! and an unknown one is skipped. A snapshot therefore outlives the edit that produced
//! it: a `bool` turned `u32`, a field added or removed, a variant renamed, each restores
//! as far as it can and falls back to the initial value where it cannot.

use std::{
  collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
  hash::{BuildHasher, Hash},
  rc::Rc,
  sync::Arc,
  time::Duration,
};

/// A value that takes part in state snapshots: recorded into the lenient encoding and
/// restored from it across runs of the program.
///
/// Derive it with `#[derive(Snapshot)]` on a struct or an enum; every std value a
/// signal commonly holds implements it. A field's key is a hash of its name, so fields
/// may be reordered freely, and `#[snapshot(tag = "name")]` keys a field or a variant by
/// another name so it survives a rename; `#[snapshot(skip)]` leaves a field out, and
/// `#[snapshot(default)]` names the variant an enum restores to where its recorded one
/// is gone.
#[diagnostic::on_unimplemented(
  message = "`{Self}` cannot take part in state snapshots",
  label = "a snapshot signal's value implements `Snapshot`",
  note = "derive it with `#[derive(Snapshot)]`, or keep the value out of snapshots with `Signal::opaque`"
)]
pub trait Snapshot: Sized + 'static {
  /// Records this value.
  fn record(&self, recorder: &mut Recorder);

  /// Restores a value [`record`](Self::record) wrote, or one an earlier shape of the
  /// type wrote where the encoding still reads as this type; `None` where it cannot.
  fn restore(value: Value<'_>) -> Option<Self>;

  /// The value a place holds when the snapshot carries none for it: zero, empty,
  /// none; `None` where the type has no such value.
  #[inline(always)]
  #[must_use]
  fn absent() -> Option<Self> {
    None
  }

  /// Records a run of values; a run of bytes records as bytes.
  fn record_slice(items: &[Self], recorder: &mut Recorder) {
    recorder.seq(items.len(), |recorder| {
      for item in items {
        item.record(recorder);
      }
    });
  }

  /// Restores the run [`record_slice`](Self::record_slice) wrote, an item that no
  /// longer restores taking its absent value or being left out.
  #[must_use]
  fn restore_slice(value: Value<'_>) -> Option<Vec<Self>> {
    let restore = |item| Self::restore(item).or_else(Self::absent);

    match value {
      Value::Seq(items) => Some(items.filter_map(restore).collect()),
      Value::Bytes(bytes) => Some(
        bytes
          .iter()
          .filter_map(|byte| restore(Value::Uint(u64::from(*byte))))
          .collect(),
      ),
      _ => None,
    }
  }
}

/// The kind a recorded value carries.
mod tag {
  pub(super) const UNIT: u8 = 0;
  pub(super) const FALSE: u8 = 1;
  pub(super) const TRUE: u8 = 2;
  pub(super) const INT: u8 = 3;
  pub(super) const UINT: u8 = 4;
  pub(super) const WIDE: u8 = 5;
  pub(super) const FLOAT: u8 = 6;
  pub(super) const STR: u8 = 7;
  pub(super) const BYTES: u8 = 8;
  pub(super) const SEQ: u8 = 9;
  pub(super) const MAP: u8 = 10;
  pub(super) const RECORD: u8 = 11;
  pub(super) const VARIANT: u8 = 12;
}

/// The key a field or a variant is recorded under: FNV-1a over its name, stable under
/// reordering.
#[inline(always)]
#[must_use]
pub const fn key(name: &str) -> u32 {
  let bytes = name.as_bytes();
  let mut hash = 0x811c_9dc5_u32;
  let mut at = 0;

  while at < bytes.len() {
    hash = (hash ^ bytes[at] as u32).wrapping_mul(0x0100_0193);
    at += 1;
  }

  hash
}

/// Records values into the encoding.
#[derive(Debug, Default)]
#[must_use]
pub struct Recorder {
  out: Vec<u8>,
}

impl Recorder {
  /// A recorder with nothing recorded.
  #[inline(always)]
  pub fn new() -> Self {
    Self::default()
  }

  /// The bytes recorded so far.
  #[inline(always)]
  #[must_use]
  pub fn finish(self) -> Vec<u8> {
    self.out
  }

  #[expect(
    clippy::cast_possible_truncation,
    reason = "each byte carries the value's next seven low bits"
  )]
  fn varint(&mut self, mut value: u64) {
    while value >= 0x80 {
      self.out.push((value as u8) | 0x80);
      value >>= 7;
    }

    self.out.push(value as u8);
  }

  /// Records the unit value.
  #[inline(always)]
  pub fn unit(&mut self) {
    self.out.push(tag::UNIT);
  }

  /// Records a boolean.
  #[inline(always)]
  pub fn bool(&mut self, value: bool) {
    self.out.push(if value { tag::TRUE } else { tag::FALSE });
  }

  /// Records a signed integer.
  #[inline(always)]
  pub fn int(&mut self, value: i64) {
    self.out.push(tag::INT);
    self.varint(((value << 1) ^ (value >> 63)).cast_unsigned());
  }

  /// Records an unsigned integer.
  #[inline(always)]
  pub fn uint(&mut self, value: u64) {
    self.out.push(tag::UINT);
    self.varint(value);
  }

  /// Records a 128-bit integer by its bits.
  #[inline(always)]
  pub fn wide(&mut self, bits: u128) {
    self.out.push(tag::WIDE);
    self.out.extend_from_slice(&bits.to_le_bytes());
  }

  /// Records a float.
  #[inline(always)]
  pub fn float(&mut self, value: f64) {
    self.out.push(tag::FLOAT);
    self.out.extend_from_slice(&value.to_le_bytes());
  }

  /// Records a string.
  pub fn str(&mut self, value: &str) {
    self.out.push(tag::STR);
    self.varint(value.len() as u64);
    self.out.extend_from_slice(value.as_bytes());
  }

  /// Records a run of bytes.
  pub fn bytes(&mut self, value: &[u8]) {
    self.out.push(tag::BYTES);
    self.varint(value.len() as u64);
    self.out.extend_from_slice(value);
  }

  /// Records a container: its tag, then its payload by length.
  fn container(&mut self, kind: u8, payload: impl FnOnce(&mut Self)) {
    let mut inner = Self::new();

    payload(&mut inner);
    self.out.push(kind);
    self.varint(inner.out.len() as u64);
    self.out.extend_from_slice(&inner.out);
  }

  /// Records a sequence of `count` values, which `items` records in order.
  pub fn seq(&mut self, count: usize, items: impl FnOnce(&mut Self)) {
    self.container(tag::SEQ, |inner| {
      inner.varint(count as u64);
      items(inner);
    });
  }

  /// Records a map of `count` entries, which `entries` records as key then value, in
  /// order.
  pub fn map(&mut self, count: usize, entries: impl FnOnce(&mut Self)) {
    self.container(tag::MAP, |inner| {
      inner.varint(count as u64);
      entries(inner);
    });
  }

  /// Records a struct: its fields by key, in any order.
  pub fn record(&mut self, fields: impl FnOnce(&mut Fields)) {
    self.container(tag::RECORD, |inner| {
      let mut fields_out = Fields {
        recorder: Self::new(),
        count: 0,
      };

      fields(&mut fields_out);
      inner.varint(fields_out.count);
      inner.out.extend_from_slice(&fields_out.recorder.out);
    });
  }

  /// Records an enum: the variant's key and the one value it carries.
  pub fn variant(&mut self, key: u32, payload: impl FnOnce(&mut Self)) {
    self.container(tag::VARIANT, |inner| {
      inner.varint(u64::from(key));
      payload(inner);
    });
  }
}

/// A struct's fields as they are recorded.
#[derive(Debug)]
#[must_use]
pub struct Fields {
  recorder: Recorder,
  count: u64,
}

impl Fields {
  /// Records `value` under `key`.
  pub fn field(&mut self, key: u32, value: &impl Snapshot) {
    self.recorder.varint(u64::from(key));
    value.record(&mut self.recorder);
    self.count += 1;
  }
}

/// Reads values back from the encoding.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct Reader<'a> {
  bytes: &'a [u8],
}

impl<'a> Reader<'a> {
  /// A reader over `bytes`.
  #[inline(always)]
  pub const fn new(bytes: &'a [u8]) -> Self {
    Self { bytes }
  }

  fn byte(&mut self) -> Option<u8> {
    let (first, rest) = self.bytes.split_first()?;

    self.bytes = rest;

    Some(*first)
  }

  const fn take(&mut self, count: usize) -> Option<&'a [u8]> {
    if self.bytes.len() < count {
      return None;
    }

    let (taken, rest) = self.bytes.split_at(count);

    self.bytes = rest;

    Some(taken)
  }

  fn varint(&mut self) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0u32;

    loop {
      let byte = self.byte()?;

      value |= u64::from(byte & 0x7f).checked_shl(shift)?;

      if byte & 0x80 == 0 {
        return Some(value);
      }

      shift += 7;

      if shift > 63 {
        return None;
      }
    }
  }

  fn length(&mut self) -> Option<usize> {
    usize::try_from(self.varint()?).ok()
  }

  /// The next value, or `None` at the end or on bytes that are not the encoding.
  pub fn value(&mut self) -> Option<Value<'a>> {
    let value = match self.byte()? {
      tag::UNIT => Value::Unit,
      tag::FALSE => Value::Bool(false),
      tag::TRUE => Value::Bool(true),
      tag::INT => {
        let raw = self.varint()?;

        Value::Int((raw >> 1).cast_signed() ^ -(raw & 1).cast_signed())
      }
      tag::UINT => Value::Uint(self.varint()?),
      tag::WIDE => Value::Wide(u128::from_le_bytes(self.take(16)?.try_into().ok()?)),
      tag::FLOAT => Value::Float(f64::from_le_bytes(self.take(8)?.try_into().ok()?)),
      tag::STR => {
        let length = self.length()?;

        Value::Str(std::str::from_utf8(self.take(length)?).ok()?)
      }
      tag::BYTES => {
        let length = self.length()?;

        Value::Bytes(self.take(length)?)
      }
      kind @ (tag::SEQ | tag::MAP | tag::RECORD | tag::VARIANT) => {
        let length = self.length()?;
        let mut inner = Self::new(self.take(length)?);

        match kind {
          tag::SEQ => Value::Seq(Seq {
            remaining: inner.length()?,
            reader: inner,
          }),
          tag::MAP => Value::Map(Seq {
            remaining: inner.length()?.checked_mul(2)?,
            reader: inner,
          }),
          tag::RECORD => Value::Record(Record {
            remaining: inner.length()?,
            reader: inner,
          }),
          _ => Value::Variant(u32::try_from(inner.varint()?).ok()?, inner),
        }
      }
      _ => return None,
    };

    Some(value)
  }
}

/// One recorded value, read back.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub enum Value<'a> {
  /// The unit value, and what an absent option recorded.
  Unit,
  /// A boolean.
  Bool(bool),
  /// A signed integer.
  Int(i64),
  /// An unsigned integer.
  Uint(u64),
  /// A 128-bit integer, by its bits.
  Wide(u128),
  /// A float.
  Float(f64),
  /// A string.
  Str(&'a str),
  /// A run of bytes.
  Bytes(&'a [u8]),
  /// A sequence of values.
  Seq(Seq<'a>),
  /// A map: keys and values alternating.
  Map(Seq<'a>),
  /// A struct: values by field key.
  Record(Record<'a>),
  /// An enum: the variant's key, and a reader over the one value it carries.
  Variant(u32, Reader<'a>),
}

impl Value<'_> {
  /// This value as an integer, where its kind admits one; a float by truncation,
  /// saturating at the bounds.
  #[must_use]
  #[expect(
    clippy::cast_possible_truncation,
    reason = "a float restores as an integer by truncation"
  )]
  pub fn as_i128(&self) -> Option<i128> {
    match *self {
      Self::Bool(value) => Some(i128::from(value)),
      Self::Int(value) => Some(i128::from(value)),
      Self::Uint(value) => Some(i128::from(value)),
      Self::Wide(bits) => Some(bits.cast_signed()),
      Self::Float(value) if value.is_finite() => Some(value.trunc() as i128),
      _ => None,
    }
  }

  /// This value as a float, where its kind admits one; an integer as the nearest
  /// float.
  #[must_use]
  #[expect(
    clippy::cast_precision_loss,
    reason = "an integer restores as the nearest float"
  )]
  pub fn as_f64(&self) -> Option<f64> {
    match *self {
      Self::Bool(value) => Some(f64::from(u8::from(value))),
      Self::Int(value) => Some(value as f64),
      Self::Uint(value) => Some(value as f64),
      Self::Wide(bits) => Some(bits.cast_signed() as f64),
      Self::Float(value) => Some(value),
      _ => None,
    }
  }
}

/// A recorded sequence, read one value at a time.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct Seq<'a> {
  remaining: usize,
  reader: Reader<'a>,
}

impl Seq<'_> {
  /// The number of values not yet read.
  #[inline(always)]
  #[must_use]
  pub const fn len(&self) -> usize {
    self.remaining
  }

  /// Whether every value has been read.
  #[inline(always)]
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.remaining == 0
  }
}

#[expect(
  clippy::copy_iterator,
  reason = "a sequence is a cursor over borrowed bytes; a copy reads the same values"
)]
impl<'a> Iterator for Seq<'a> {
  type Item = Value<'a>;

  fn next(&mut self) -> Option<Value<'a>> {
    if self.remaining == 0 {
      return None;
    }

    self.remaining -= 1;

    self.reader.value()
  }
}

/// A recorded struct: its fields by key.
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct Record<'a> {
  remaining: usize,
  reader: Reader<'a>,
}

impl<'a> Record<'a> {
  /// The value recorded under `key`, if any.
  #[must_use]
  pub fn field(&self, key: u32) -> Option<Value<'a>> {
    self
      .fields()
      .find(|(found, _)| *found == key)
      .map(|(_, value)| value)
  }

  /// Every field, as key and value, in recording order.
  pub fn fields(&self) -> impl Iterator<Item = (u32, Value<'a>)> + 'a {
    let mut reader = self.reader;
    let mut remaining = self.remaining;

    std::iter::from_fn(move || {
      if remaining == 0 {
        return None;
      }

      remaining -= 1;

      let key = u32::try_from(reader.varint()?).ok()?;

      Some((key, reader.value()?))
    })
  }
}

/// Records `value` as the bytes a snapshot carries.
#[must_use]
pub fn encode(value: &impl Snapshot) -> Vec<u8> {
  let mut recorder = Recorder::new();

  value.record(&mut recorder);

  recorder.finish()
}

/// Restores a value from the bytes [`encode`] produced, or from an earlier shape of the
/// type where the encoding still reads as one.
#[must_use]
pub fn decode<T: Snapshot>(bytes: &[u8]) -> Option<T> {
  T::restore(Reader::new(bytes).value()?)
}

impl Snapshot for () {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.unit();
  }

  #[inline(always)]
  fn restore(_: Value<'_>) -> Option<Self> {
    Some(())
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(())
  }
}

impl Snapshot for bool {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.bool(*self);
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    match value {
      Value::Bool(value) => Some(value),
      Value::Str(text) => match text {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
      },
      other => other.as_i128().map(|value| value != 0),
    }
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(false)
  }
}

macro_rules! signed {
  ($($ty:ty),*) => {$(
    impl Snapshot for $ty {
      #[inline(always)]
      fn record(&self, recorder: &mut Recorder) {
        recorder.int(i64::from(*self));
      }

      fn restore(value: Value<'_>) -> Option<Self> {
        Self::try_from(value.as_i128()?).ok()
      }

      #[inline(always)]
      fn absent() -> Option<Self> {
        Some(0)
      }
    }
  )*};
}

macro_rules! unsigned {
  ($($ty:ty),*) => {$(
    impl Snapshot for $ty {
      #[inline(always)]
      fn record(&self, recorder: &mut Recorder) {
        recorder.uint(u64::from(*self));
      }

      fn restore(value: Value<'_>) -> Option<Self> {
        Self::try_from(value.as_i128()?).ok()
      }

      #[inline(always)]
      fn absent() -> Option<Self> {
        Some(0)
      }
    }
  )*};
}

signed!(i8, i16, i32, i64);
unsigned!(u16, u32, u64);

impl Snapshot for u8 {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.uint(u64::from(*self));
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    Self::try_from(value.as_i128()?).ok()
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(0)
  }

  #[inline(always)]
  fn record_slice(items: &[Self], recorder: &mut Recorder) {
    recorder.bytes(items);
  }

  fn restore_slice(value: Value<'_>) -> Option<Vec<Self>> {
    match value {
      Value::Bytes(bytes) => Some(bytes.to_vec()),
      Value::Str(text) => Some(text.as_bytes().to_vec()),
      Value::Seq(items) => Some(items.filter_map(|item| Self::restore(item)).collect()),
      _ => None,
    }
  }
}

impl Snapshot for isize {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.int(*self as i64);
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    Self::try_from(value.as_i128()?).ok()
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(0)
  }
}

impl Snapshot for usize {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.uint(*self as u64);
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    Self::try_from(value.as_i128()?).ok()
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(0)
  }
}

impl Snapshot for i128 {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.wide(self.cast_unsigned());
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    value.as_i128()
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(0)
  }
}

impl Snapshot for u128 {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.wide(*self);
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    match value {
      Value::Wide(bits) => Some(bits),
      other => Self::try_from(other.as_i128()?).ok(),
    }
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(0)
  }
}

impl Snapshot for f32 {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.float(f64::from(*self));
  }

  #[expect(
    clippy::cast_possible_truncation,
    reason = "a float restores at its nearest single precision"
  )]
  fn restore(value: Value<'_>) -> Option<Self> {
    value.as_f64().map(|value| value as Self)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(0.)
  }
}

impl Snapshot for f64 {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.float(*self);
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    value.as_f64()
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(0.)
  }
}

impl Snapshot for char {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.uint(u64::from(u32::from(*self)));
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    match value {
      Value::Str(text) => {
        let mut chars = text.chars();
        let first = chars.next()?;

        chars.next().is_none().then_some(first)
      }
      other => Self::from_u32(u32::try_from(other.as_i128()?).ok()?),
    }
  }
}

impl Snapshot for String {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    recorder.str(self);
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    match value {
      Value::Str(text) => Some(text.to_owned()),
      Value::Bytes(bytes) => std::str::from_utf8(bytes).ok().map(str::to_owned),
      _ => None,
    }
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::new())
  }
}

impl<T: Snapshot> Snapshot for Option<T> {
  fn record(&self, recorder: &mut Recorder) {
    match self {
      Some(value) => value.record(recorder),
      None => recorder.unit(),
    }
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    match value {
      Value::Unit => Some(None),
      other => Some(T::restore(other)),
    }
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(None)
  }
}

impl<T: Snapshot> Snapshot for Vec<T> {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    T::record_slice(self, recorder);
  }

  #[inline(always)]
  fn restore(value: Value<'_>) -> Option<Self> {
    T::restore_slice(value)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::new())
  }
}

impl<T: Snapshot> Snapshot for VecDeque<T> {
  fn record(&self, recorder: &mut Recorder) {
    recorder.seq(self.len(), |recorder| {
      for item in self {
        item.record(recorder);
      }
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    T::restore_slice(value).map(Self::from)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::new())
  }
}

impl<T: Snapshot, const N: usize> Snapshot for [T; N] {
  fn record(&self, recorder: &mut Recorder) {
    T::record_slice(self, recorder);
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let mut items = T::restore_slice(value)?.into_iter();

    let restored: Vec<T> = (0..N)
      .map(|_| items.next().or_else(T::absent))
      .collect::<Option<_>>()?;

    restored.try_into().ok()
  }
}

impl<T: Snapshot> Snapshot for Box<T> {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    T::record(self, recorder);
  }

  #[inline(always)]
  fn restore(value: Value<'_>) -> Option<Self> {
    T::restore(value).map(Self::new)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    T::absent().map(Self::new)
  }
}

impl<T: Snapshot> Snapshot for Rc<T> {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    T::record(self, recorder);
  }

  #[inline(always)]
  fn restore(value: Value<'_>) -> Option<Self> {
    T::restore(value).map(Self::new)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    T::absent().map(Self::new)
  }
}

impl<T: Snapshot> Snapshot for Arc<T> {
  #[inline(always)]
  fn record(&self, recorder: &mut Recorder) {
    T::record(self, recorder);
  }

  #[inline(always)]
  fn restore(value: Value<'_>) -> Option<Self> {
    T::restore(value).map(Self::new)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    T::absent().map(Self::new)
  }
}

impl<K: Snapshot + Eq + Hash, V: Snapshot, S: BuildHasher + Default + 'static> Snapshot
  for HashMap<K, V, S>
{
  fn record(&self, recorder: &mut Recorder) {
    recorder.map(self.len(), |recorder| {
      for (key, value) in self {
        key.record(recorder);
        value.record(recorder);
      }
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let Value::Map(mut entries) = value
    else {
      return None;
    };
    let mut map = Self::default();

    while let Some(key) = entries.next() {
      let value = entries.next()?;

      if let (Some(key), Some(value)) = (K::restore(key), V::restore(value).or_else(V::absent)) {
        map.insert(key, value);
      }
    }

    Some(map)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::default())
  }
}

impl<K: Snapshot + Ord, V: Snapshot> Snapshot for BTreeMap<K, V> {
  fn record(&self, recorder: &mut Recorder) {
    recorder.map(self.len(), |recorder| {
      for (key, value) in self {
        key.record(recorder);
        value.record(recorder);
      }
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let Value::Map(mut entries) = value
    else {
      return None;
    };
    let mut map = Self::new();

    while let Some(key) = entries.next() {
      let value = entries.next()?;

      if let (Some(key), Some(value)) = (K::restore(key), V::restore(value).or_else(V::absent)) {
        map.insert(key, value);
      }
    }

    Some(map)
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::new())
  }
}

impl<T: Snapshot + Eq + Hash, S: BuildHasher + Default + 'static> Snapshot for HashSet<T, S> {
  fn record(&self, recorder: &mut Recorder) {
    recorder.seq(self.len(), |recorder| {
      for item in self {
        item.record(recorder);
      }
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let Value::Seq(items) = value
    else {
      return None;
    };

    Some(items.filter_map(T::restore).collect())
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::default())
  }
}

impl<T: Snapshot + Ord> Snapshot for BTreeSet<T> {
  fn record(&self, recorder: &mut Recorder) {
    recorder.seq(self.len(), |recorder| {
      for item in self {
        item.record(recorder);
      }
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let Value::Seq(items) = value
    else {
      return None;
    };

    Some(items.filter_map(T::restore).collect())
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::new())
  }
}

impl Snapshot for Duration {
  fn record(&self, recorder: &mut Recorder) {
    recorder.seq(2, |recorder| {
      recorder.uint(self.as_secs());
      recorder.uint(u64::from(self.subsec_nanos()));
    });
  }

  fn restore(value: Value<'_>) -> Option<Self> {
    let Value::Seq(mut items) = value
    else {
      return None;
    };
    let seconds = u64::restore(items.next()?)?;
    let nanoseconds = items.next().and_then(u32::restore).unwrap_or(0);

    Some(Self::new(seconds, nanoseconds))
  }

  #[inline(always)]
  fn absent() -> Option<Self> {
    Some(Self::ZERO)
  }
}

macro_rules! tuples {
  ($(($count:expr; $($name:ident $index:tt),+)),*) => {$(
    impl<$($name: Snapshot),+> Snapshot for ($($name,)+) {
      fn record(&self, recorder: &mut Recorder) {
        recorder.seq($count, |recorder| {
          $(self.$index.record(recorder);)+
        });
      }

      fn restore(value: Value<'_>) -> Option<Self> {
        let Value::Seq(mut items) = value
        else {
          return None;
        };

        Some(($(
          match items.next() {
            Some(item) => $name::restore(item).or_else($name::absent)?,
            None => $name::absent()?,
          },
        )+))
      }

      fn absent() -> Option<Self> {
        Some(($($name::absent()?,)+))
      }
    }
  )*};
}

tuples!(
  (1; A 0),
  (2; A 0, B 1),
  (3; A 0, B 1, C 2),
  (4; A 0, B 1, C 2, D 3),
  (5; A 0, B 1, C 2, D 3, E 4),
  (6; A 0, B 1, C 2, D 3, E 4, F 5)
);

#[cfg(test)]
mod tests {
  use super::*;

  /// Every kind round-trips through its own type.
  #[test]
  fn values_round_trip() {
    assert_eq!(decode::<bool>(&encode(&true)), Some(true));
    assert_eq!(decode::<i32>(&encode(&-7)), Some(-7));
    assert_eq!(decode::<u64>(&encode(&u64::MAX)), Some(u64::MAX));
    assert_eq!(decode::<u128>(&encode(&(1u128 << 127))), Some(1u128 << 127));
    assert_eq!(decode::<f32>(&encode(&1.5f32)), Some(1.5));
    assert_eq!(
      decode::<String>(&encode(&"héllo".to_owned())),
      Some("héllo".to_owned())
    );
    assert_eq!(
      decode::<Vec<u8>>(&encode(&vec![1u8, 2, 3])),
      Some(vec![1, 2, 3])
    );
    assert_eq!(decode::<Vec<i32>>(&encode(&vec![1, -2])), Some(vec![1, -2]));
    assert_eq!(decode::<Option<u8>>(&encode(&None::<u8>)), Some(None));
    assert_eq!(decode::<Option<u8>>(&encode(&Some(9u8))), Some(Some(9)));
    assert_eq!(
      decode::<(u8, String)>(&encode(&(4u8, "x".to_owned()))),
      Some((4, "x".to_owned()))
    );
    assert_eq!(
      decode::<Duration>(&encode(&Duration::new(3, 40))),
      Some(Duration::new(3, 40))
    );
  }

  /// A value read as another kind coerces where it plainly survives.
  #[test]
  fn kinds_coerce() {
    assert_eq!(decode::<u32>(&encode(&true)), Some(1));
    assert_eq!(decode::<bool>(&encode(&0u32)), Some(false));
    assert_eq!(decode::<f64>(&encode(&3i32)), Some(3.));
    assert_eq!(decode::<i32>(&encode(&2.9f64)), Some(2));
    assert_eq!(decode::<u8>(&encode(&300i32)), None);
    assert_eq!(decode::<String>(&encode(&5u8)), None);
    assert_eq!(decode::<Vec<u8>>(&encode(&vec![1u32, 2])), Some(vec![1, 2]));
    assert_eq!(decode::<Option<u8>>(&encode(&())), Some(None));
  }

  /// A record restores by key: missing fields are absent, unknown ones are skipped.
  #[test]
  fn records_restore_by_key() {
    let mut recorder = Recorder::new();

    recorder.record(|fields| {
      fields.field(key("name"), &"a".to_owned());
      fields.field(key("gone"), &1u8);
    });

    let bytes = recorder.finish();
    let Some(Value::Record(record)) = Reader::new(&bytes).value()
    else {
      panic!("not a record");
    };

    assert_eq!(
      String::restore(record.field(key("name")).unwrap()),
      Some("a".to_owned())
    );
    assert!(record.field(key("count")).is_none());
    assert_eq!(record.fields().count(), 2);
  }
}
