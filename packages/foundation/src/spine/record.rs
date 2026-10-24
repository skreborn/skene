//! A node's style as one record: every property it sets under Skene's name for it, the
//! value erased over the property, and its motion as data; and the record compiled to
//! the level a realization keeps.

use std::hash::{Hash, Hasher};

use super::{
  Animation,
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxStyle,
  Condition,
  Fingerprint,
  IntoBoxValue,
  IntoRunValue,
  KeptLeaf,
  Keyframe,
  Plain,
  PropertyKey,
  Resolver,
  RunDynamics,
  RunField,
  RunFieldVisitor,
  RunStyle,
  Scalar,
  Slot,
  SlotLeaf,
  SlotValue,
  Static,
  StaticLeaf,
  StaticValue,
  StyleExtension,
  Transition,
};

#[cfg(test)]
pub mod coverage;

/// One property's value in a [`Record`], erased over the property.
///
/// It is the property's [`StaticValue`] under the facade's conditions, each fixed leaf
/// erased into a [`SlotValue`], a token kept by its name, an expression by its tree and a
/// derived leaf as its slot.
pub type RecordValue<N, X = ()> = StaticValue<SlotValue<N, X>, Condition<N>>;

/// One entry of a [`Record`]: a property's name and its value.
pub type RecordEntry<N, X = ()> = (
  PropertyKey<<X as StyleExtension>::Property>,
  RecordValue<N, X>,
);

/// One entry of a [`Frame`]: a property's name and its leaf, erased over the property.
pub type FrameEntry<N, X = ()> = (PropertyKey<<X as StyleExtension>::Property>, SlotLeaf<N, X>);

/// A node's style as one record: each property it sets, keyed by its [`PropertyKey`] and
/// erased over the property, with the transitions and keyframe animations it declares.
///
/// A key names one property, of the box or of the run, so a box's and a run's entries
/// share the record without meeting.
///
/// It is written typed, by a property's [`set_box`](Self::set_box) or
/// [`set_run`](Self::set_run) or a whole style's projection
/// ([`add_box`](Self::add_box), [`add_run`](Self::add_run) and their dynamics), so a value
/// of another property's kind does not compile, and read erased, entry by entry in the
/// order the styles' visitors give. [`box_style`](Self::box_style) and
/// [`run_style`](Self::run_style) read the typed styles back, and
/// [`compile`](Self::compile) compiles it to the level a realization keeps.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Record<N: Scalar, X: StyleExtension = ()> {
  entries: Vec<RecordEntry<N, X>>,
  transitions: Vec<Transition<X::Property>>,
  animations: Vec<Animation<Frame<N, X>>>,
}

/// One keyframe of a [`Record`]'s animation: each property it sets, keyed by its
/// [`PropertyKey`], its leaf erased over the property.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct Frame<N: Scalar, X: StyleExtension = ()> {
  entries: Vec<FrameEntry<N, X>>,
}

/// What of a style a realization keeps unresolved, its toolkit resolving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub enum Keep {
  /// Nothing: the shared half resolves every value at the node, its tokens, expressions,
  /// media and container conditions, states and slots, and each keyframe's leaves; the
  /// transitions and the animations' timing cross as data.
  Nothing,
  /// Everything: a token by its name, which the realization keys by the theme scope
  /// defining it, never by its definition's content; an expression as its tree; media,
  /// container and state conditions as conditions; a derived leaf as the node's slot,
  /// written apart from the static part.
  Everything,
}

/// A [`Keep`] level with what the shared half resolves the rest through.
#[derive(Debug, Clone, Copy)]
pub enum Keeping<'a, 'r, N: Scalar, X: StyleExtension = ()> {
  /// [`Keep::Nothing`], every value resolved by `resolver` at its node.
  Nothing(&'a Resolver<'r, N, X>),
  /// [`Keep::Everything`], which resolves nothing.
  Everything,
}

impl<N: Scalar, X: StyleExtension> Keeping<'_, '_, N, X> {
  /// The level.
  #[inline(always)]
  pub const fn keep(&self) -> Keep {
    match self {
      Self::Nothing(_) => Keep::Nothing,
      Self::Everything => Keep::Everything,
    }
  }
}

/// A slot a [`CompiledStyle`]'s static part references: the property it belongs to and
/// the leaf's position in the property's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[must_use]
pub struct SlotBinding<P> {
  /// The slot.
  pub slot: Slot,
  /// The property the slot belongs to.
  pub property: PropertyKey<P>,
  /// The leaf's position: `0` the base, `i` the `i`-th variant.
  pub position: usize,
}

/// A [`Record`] compiled to the level its realization keeps ([`Record::compile`]).
///
/// Its static part, the entries and the motion, is a function of the record and, at
/// [`Keep::Nothing`], of what the resolver read; a derived leaf's value never reaches
/// it, a slot standing in its place at [`Keep::Everything`]. Its
/// [`address`](Self::address) is the static part's wide hash, a cache key alone: equal
/// static parts share an address, and a realization names nothing by it.
#[derive(Debug, Clone, PartialEq)]
#[must_use]
pub struct CompiledStyle<N: Scalar, X: StyleExtension = ()> {
  keep: Keep,
  entries: Vec<RecordEntry<N, X>>,
  transitions: Vec<Transition<X::Property>>,
  animations: Vec<Animation<Frame<N, X>>>,
  slots: Vec<SlotBinding<X::Property>>,
  address: u128,
}

/// A style resolved at a node: the box's and the run's properties, each a plain value.
#[derive(Debug, Clone, Default, PartialEq)]
#[must_use]
pub struct ResolvedStyle<N: Scalar, X: StyleExtension = ()> {
  /// The box properties.
  pub boxed: BoxStyle<N, Plain, X>,
  /// The run properties.
  pub run: RunStyle<Plain, X>,
}

impl<N: Scalar, X: StyleExtension> Record<N, X> {
  /// A record setting nothing.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      entries: Vec::new(),
      transitions: Vec::new(),
      animations: Vec::new(),
    }
  }

  /// Sets the box property `F` to `value`, replacing its entry in place where it has one
  /// and appending one otherwise.
  ///
  /// The value is of the property's own type, so another property's does not compile:
  ///
  /// ```compile_fail,E0308
  /// use skene_foundation::{Color, Record, StaticValue, box_fields};
  ///
  /// let mut record = Record::<f32>::new();
  ///
  /// record.set_box::<box_fields::Width>(StaticValue::from(Color::TRANSPARENT));
  /// ```
  pub fn set_box<F: BoxField<N, X>>(&mut self, value: StaticValue<F::Value, Condition<N>>) {
    self.insert(
      F::KEY,
      value.map(|leaf| leaf.map(|fixed| SlotValue::Box(fixed.into_box_value()))),
    );
  }

  /// Sets the run property `F` to `value`, replacing its entry in place where it has one
  /// and appending one otherwise.
  pub fn set_run<F: RunField<X>>(&mut self, value: StaticValue<F::Value, Condition<N>>) {
    self.insert(
      F::KEY,
      value.map(|leaf| leaf.map(|fixed| SlotValue::Run(fixed.into_run_value()))),
    );
  }

  fn insert(&mut self, key: PropertyKey<X::Property>, value: RecordValue<N, X>) {
    match self.entries.iter_mut().find(|(entry, _)| *entry == key) {
      Some((_, entry)) => *entry = value,
      None => self.entries.push((key, value)),
    }
  }

  /// Sets every property `style` sets, in the order its visitor gives.
  pub fn add_box(&mut self, style: &BoxStyle<N, Static<Condition<N>>, X>) {
    BoxStyle::<N, Static<Condition<N>>, X>::for_each_field(&mut AddBox {
      record: self,
      style,
    });
  }

  /// Sets every property `style` sets, in the order its visitor gives.
  pub fn add_run(&mut self, style: &RunStyle<Static<Condition<N>>, X>) {
    RunStyle::<Static<Condition<N>>, X>::for_each_field(&mut AddRun {
      record: self,
      style,
    });
  }

  /// Appends a box's transitions and its animations, each keyframe as a [`Frame`].
  pub fn add_box_dynamics(&mut self, dynamics: &BoxDynamics<N, X>) {
    self.transitions.extend_from_slice(&dynamics.transitions);
    self.animations.extend(
      dynamics
        .animations
        .iter()
        .map(|animation| animation.map_keyframes(Frame::of_box)),
    );
  }

  /// Appends a run's transitions and its animations, each keyframe as a [`Frame`].
  pub fn add_run_dynamics(&mut self, dynamics: &RunDynamics<X>) {
    self.transitions.extend_from_slice(&dynamics.transitions);
    self.animations.extend(
      dynamics
        .animations
        .iter()
        .map(|animation| animation.map_keyframes(Frame::of_run)),
    );
  }

  /// Every property the record sets with its value, in the order they were first set.
  #[inline(always)]
  #[must_use]
  pub const fn entries(&self) -> &[RecordEntry<N, X>] {
    self.entries.as_slice()
  }

  /// The value of the property `key`; `None` where the record leaves it unset.
  #[must_use]
  pub fn get(&self, key: PropertyKey<X::Property>) -> Option<&RecordValue<N, X>> {
    self
      .entries
      .iter()
      .find(|(entry, _)| *entry == key)
      .map(|(_, value)| value)
  }

  /// Whether the record sets the property `key`.
  #[must_use]
  pub fn has(&self, key: PropertyKey<X::Property>) -> bool {
    self.get(key).is_some()
  }

  /// The property transitions, in declaration order.
  #[inline(always)]
  #[must_use]
  pub const fn transitions(&self) -> &[Transition<X::Property>] {
    self.transitions.as_slice()
  }

  /// The keyframe animations, in declaration order.
  #[inline(always)]
  #[must_use]
  pub const fn animations(&self) -> &[Animation<Frame<N, X>>] {
    self.animations.as_slice()
  }

  /// The box properties the record sets, as the typed style.
  pub fn box_style(&self) -> BoxStyle<N, Static<Condition<N>>, X> {
    let mut visitor = ReadBox {
      record: self,
      target: BoxStyle::default(),
    };

    BoxStyle::<N, Static<Condition<N>>, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// The run properties the record sets, as the typed style.
  pub fn run_style(&self) -> RunStyle<Static<Condition<N>>, X> {
    let mut visitor = ReadRun {
      record: self,
      target: RunStyle::default(),
    };

    RunStyle::<Static<Condition<N>>, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// The record compiled to the level `keeping` names, in the record's order.
  ///
  /// At [`Keep::Everything`] every entry and frame stays as the record holds it, and
  /// each slot an entry references is bound to its property and position. At
  /// [`Keep::Nothing`] each property is resolved as [`Resolver::box_value`] and
  /// [`Resolver::run_value`] resolve it, a fixed leaf under no condition, and each frame
  /// as [`Frame::compile`] resolves it; a property resolving to nothing is left out.
  pub fn compile(&self, keeping: Keeping<'_, '_, N, X>) -> CompiledStyle<N, X> {
    let entries = match keeping {
      Keeping::Everything => self.entries.clone(),
      Keeping::Nothing(resolver) => self
        .entries
        .iter()
        .zip(Resolve::all(
          &self.entries,
          resolver,
          vec![None; self.entries.len()],
        ))
        .filter_map(|((key, _), value)| Some((*key, StaticValue::from(value?))))
        .collect(),
    };
    let animations = self
      .animations
      .iter()
      .map(|animation| animation.map_keyframes(|frame| frame.compile(keeping)))
      .collect();

    CompiledStyle::new(
      keeping.keep(),
      entries,
      self.transitions.clone(),
      animations,
    )
  }

  /// The record compiled at [`Keep::Everything`], as [`compile`](Self::compile) compiles
  /// it there, its entries and motion moved rather than copied: the style the shared
  /// half hands a realization.
  pub fn into_kept(self) -> CompiledStyle<N, X> {
    CompiledStyle::new(
      Keep::Everything,
      self.entries,
      self.transitions,
      self.animations,
    )
  }
}

impl<N: Scalar, X: StyleExtension> Default for Record<N, X> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<N: Scalar, X: StyleExtension> Frame<N, X> {
  /// The frame of a box keyframe: every property it sets, in the order its visitor
  /// gives.
  pub fn of_box(keyframe: &BoxStyle<N, Keyframe, X>) -> Self {
    let mut visitor = FrameBox {
      frame: Self {
        entries: Vec::new(),
      },
      keyframe,
    };

    BoxStyle::<N, Keyframe, X>::for_each_field(&mut visitor);

    visitor.frame
  }

  /// The frame of a run keyframe: every property it sets, in the order its visitor
  /// gives.
  pub fn of_run(keyframe: &RunStyle<Keyframe, X>) -> Self {
    let mut visitor = FrameRun {
      frame: Self {
        entries: Vec::new(),
      },
      keyframe,
    };

    RunStyle::<Keyframe, X>::for_each_field(&mut visitor);

    visitor.frame
  }

  /// Every property the frame sets with its leaf, in the order its visitor gave.
  #[inline(always)]
  #[must_use]
  pub const fn entries(&self) -> &[FrameEntry<N, X>] {
    self.entries.as_slice()
  }

  fn get(&self, key: PropertyKey<X::Property>) -> Option<&SlotLeaf<N, X>> {
    self
      .entries
      .iter()
      .find(|(entry, _)| *entry == key)
      .map(|(_, leaf)| leaf)
  }

  /// The box properties the frame sets, as the typed keyframe.
  pub fn box_keyframe(&self) -> BoxStyle<N, Keyframe, X> {
    let mut visitor = ReadBoxFrame {
      frame: self,
      target: BoxStyle::default(),
    };

    BoxStyle::<N, Keyframe, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// The run properties the frame sets, as the typed keyframe.
  pub fn run_keyframe(&self) -> RunStyle<Keyframe, X> {
    let mut visitor = ReadRunFrame {
      frame: self,
      target: RunStyle::default(),
    };

    RunStyle::<Keyframe, X>::for_each_field(&mut visitor);

    visitor.target
  }

  /// The frame compiled to the level `keeping` names: as it is at
  /// [`Keep::Everything`]; at [`Keep::Nothing`] each leaf resolved as
  /// [`Resolver::keyframe_box_value`] and [`Resolver::keyframe_run_value`] resolve it, a
  /// fixed leaf, one resolving to nothing left out.
  pub fn compile(&self, keeping: Keeping<'_, '_, N, X>) -> Self {
    let Keeping::Nothing(resolver) = keeping
    else {
      return self.clone();
    };
    Self {
      entries: self
        .entries
        .iter()
        .zip(Resolve::all(
          &self.entries,
          resolver,
          vec![None; self.entries.len()],
        ))
        .filter_map(|((key, _), value)| Some((*key, StaticLeaf::Fixed(value?))))
        .collect(),
    }
  }

  /// The frame resolved at a node by `resolver`, every leaf as
  /// [`compile`](Self::compile) at [`Keep::Nothing`] resolves it, straight into plain
  /// values.
  pub fn resolve(&self, resolver: &Resolver<'_, N, X>) -> ResolvedStyle<N, X> {
    Resolve::all(&self.entries, resolver, ResolvedStyle::default())
  }

  /// The frame's fixed leaves as plain values: every leaf of a frame compiled at
  /// [`Keep::Nothing`].
  pub fn resolved(&self) -> ResolvedStyle<N, X> {
    let mut read = ReadResolved {
      fixed: |key| match self.get(key)? {
        StaticLeaf::Fixed(value) => Some(value.clone()),
        StaticLeaf::Token(_) | StaticLeaf::Expr(_) => None,
      },
      target: ResolvedStyle::default(),
    };

    BoxStyle::<N, Plain, X>::for_each_field(&mut read);
    RunStyle::<Plain, X>::for_each_field(&mut read);

    read.target
  }
}

impl<N: Scalar, X: StyleExtension> Fingerprint for Frame<N, X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    state.write_usize(self.entries.len());

    for (key, leaf) in &self.entries {
      key.hash(state);
      leaf.fingerprint(state);
    }
  }
}

impl<N: Scalar, X: StyleExtension> CompiledStyle<N, X> {
  /// The style of `entries` and its motion at `keep`, its slots bound and its static
  /// part addressed.
  fn new(
    keep: Keep,
    entries: Vec<RecordEntry<N, X>>,
    transitions: Vec<Transition<X::Property>>,
    animations: Vec<Animation<Frame<N, X>>>,
  ) -> Self {
    let slots = entries
      .iter()
      .flat_map(|(key, value)| {
        value
          .leaves()
          .enumerate()
          .filter_map(|(position, (_, leaf))| match leaf {
            KeptLeaf::Slot(slot) => Some(SlotBinding {
              slot: *slot,
              property: *key,
              position,
            }),
            KeptLeaf::Static(_) => None,
          })
      })
      .collect();
    let mut style = Self {
      keep,
      entries,
      transitions,
      animations,
      slots,
      address: 0,
    };

    style.address = skene_reactive::hash::wide_of(&Fed(&StaticPart(&style)));

    style
  }

  /// The level the style was compiled at.
  #[inline(always)]
  pub const fn keep(&self) -> Keep {
    self.keep
  }

  /// Every property the style sets with its value, in the record's order.
  #[inline(always)]
  #[must_use]
  pub const fn entries(&self) -> &[RecordEntry<N, X>] {
    self.entries.as_slice()
  }

  /// The value of the property `key`; `None` where the style leaves it unset.
  #[must_use]
  pub fn get(&self, key: PropertyKey<X::Property>) -> Option<&RecordValue<N, X>> {
    self
      .entries
      .iter()
      .find(|(entry, _)| *entry == key)
      .map(|(_, value)| value)
  }

  /// Whether the style sets the property `key`.
  #[must_use]
  pub fn has(&self, key: PropertyKey<X::Property>) -> bool {
    self.get(key).is_some()
  }

  /// The property transitions, in declaration order.
  #[inline(always)]
  #[must_use]
  pub const fn transitions(&self) -> &[Transition<X::Property>] {
    self.transitions.as_slice()
  }

  /// The keyframe animations, in declaration order, each frame compiled at the style's
  /// level.
  #[inline(always)]
  #[must_use]
  pub const fn animations(&self) -> &[Animation<Frame<N, X>>] {
    self.animations.as_slice()
  }

  /// Each slot the entries reference, with its property and position, in the entries'
  /// order and each value's base first; none at [`Keep::Nothing`].
  #[inline(always)]
  #[must_use]
  pub const fn slots(&self) -> &[SlotBinding<X::Property>] {
    self.slots.as_slice()
  }

  /// The static part's 128-bit xxh3: the level, the entries and the motion, the slots'
  /// values apart.
  #[inline(always)]
  #[must_use]
  pub const fn address(&self) -> u128 {
    self.address
  }

  /// The style resolved at a node by `resolver`: every property as [`Record::compile`]
  /// at [`Keep::Nothing`] resolves it, straight into plain values, with no compiled
  /// style between; a property resolving to nothing is left unset.
  ///
  /// It is the entry a realization keeping nothing resolves a style kept at
  /// [`Keep::Everything`] through, at every restyle of its node, each frame of its
  /// animations through [`Frame::resolve`].
  pub fn resolve(&self, resolver: &Resolver<'_, N, X>) -> ResolvedStyle<N, X> {
    Resolve::all(&self.entries, resolver, ResolvedStyle::default())
  }

  /// The properties whose value is one fixed leaf under no condition, as plain values:
  /// every property of a style compiled at [`Keep::Nothing`].
  pub fn resolved(&self) -> ResolvedStyle<N, X> {
    let mut read = ReadResolved {
      fixed: |key| {
        let value = self.get(key)?;

        match (value.base(), value.variants().is_empty()) {
          (KeptLeaf::Static(StaticLeaf::Fixed(fixed)), true) => Some(fixed.clone()),
          _ => None,
        }
      },
      target: ResolvedStyle::default(),
    };

    BoxStyle::<N, Plain, X>::for_each_field(&mut read);
    RunStyle::<Plain, X>::for_each_field(&mut read);

    read.target
  }
}

/// A compiled style's static part, as its address hashes it.
struct StaticPart<'a, N: Scalar, X: StyleExtension>(&'a CompiledStyle<N, X>);

impl<N: Scalar, X: StyleExtension> Fingerprint for StaticPart<'_, N, X> {
  fn fingerprint<H: Hasher>(&self, state: &mut H) {
    let style = self.0;

    style.keep.hash(state);
    state.write_usize(style.entries.len());

    for (key, value) in &style.entries {
      key.hash(state);
      value.fingerprint(state);
    }

    style.transitions.fingerprint(state);
    style.animations.fingerprint(state);
  }
}

/// A value's fingerprint fed to a hasher as its hash.
struct Fed<'a, T>(&'a T);

impl<T: Fingerprint> Hash for Fed<'_, T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.fingerprint(state);
  }
}

/// A value a style's entry holds, which resolves at a node to its property's type.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is no entry of a style",
  label = "a style's entry holds a record's value or a frame's leaf"
)]
trait Resolvable<N: Scalar, X: StyleExtension> {
  /// The value of the box property `key`, resolved by `resolver` as its type `T`.
  fn resolve_box<T: IntoBoxValue<N, X>>(
    &self,
    resolver: &Resolver<'_, N, X>,
    key: PropertyKey<X::Property>,
  ) -> Option<T>;

  /// The value of a run property, resolved by `resolver` as its type `T`.
  fn resolve_run<T: IntoRunValue<X>>(&self, resolver: &Resolver<'_, N, X>) -> Option<T>;
}

impl<N: Scalar, X: StyleExtension> Resolvable<N, X> for RecordValue<N, X> {
  fn resolve_box<T: IntoBoxValue<N, X>>(
    &self,
    resolver: &Resolver<'_, N, X>,
    key: PropertyKey<X::Property>,
  ) -> Option<T> {
    resolver.erased_box_value(key, self)
  }

  fn resolve_run<T: IntoRunValue<X>>(&self, resolver: &Resolver<'_, N, X>) -> Option<T> {
    resolver.erased_run_value(self)
  }
}

impl<N: Scalar, X: StyleExtension> Resolvable<N, X> for SlotLeaf<N, X> {
  fn resolve_box<T: IntoBoxValue<N, X>>(
    &self,
    resolver: &Resolver<'_, N, X>,
    key: PropertyKey<X::Property>,
  ) -> Option<T> {
    resolver.erased_keyframe_box_value(key, self)
  }

  fn resolve_run<T: IntoRunValue<X>>(&self, resolver: &Resolver<'_, N, X>) -> Option<T> {
    resolver.erased_keyframe_run_value(self)
  }
}

/// Where a resolution writes each property's resolved value, by the property and its
/// entry's index.
#[diagnostic::on_unimplemented(
  message = "`{Self}` holds no resolution of a style",
  label = "a resolution is written into the entries' places or into the plain styles"
)]
trait Resolution<N: Scalar, X: StyleExtension> {
  /// Writes the box property `F`'s value, the entry at `index`'s.
  fn set_box<F: BoxField<N, X>>(&mut self, index: usize, value: Option<F::Value>);

  /// Writes the run property `F`'s value, the entry at `index`'s.
  fn set_run<F: RunField<X>>(&mut self, index: usize, value: Option<F::Value>);
}

impl<N: Scalar, X: StyleExtension> Resolution<N, X> for Vec<Option<SlotValue<N, X>>> {
  fn set_box<F: BoxField<N, X>>(&mut self, index: usize, value: Option<F::Value>) {
    self[index] = value.map(|value| SlotValue::Box(value.into_box_value()));
  }

  fn set_run<F: RunField<X>>(&mut self, index: usize, value: Option<F::Value>) {
    self[index] = value.map(|value| SlotValue::Run(value.into_run_value()));
  }
}

impl<N: Scalar, X: StyleExtension> Resolution<N, X> for ResolvedStyle<N, X> {
  fn set_box<F: BoxField<N, X>>(&mut self, _index: usize, value: Option<F::Value>) {
    *F::get_mut(&mut self.boxed) = value;
  }

  fn set_run<F: RunField<X>>(&mut self, _index: usize, value: Option<F::Value>) {
    *F::get_mut(&mut self.run) = value;
  }
}

/// Resolves each of a style's entries at a node, writing every value into a
/// [`Resolution`].
struct Resolve<'a, 'r, N: Scalar, X: StyleExtension, E, R> {
  entries: &'a [(PropertyKey<X::Property>, E)],
  resolver: &'a Resolver<'r, N, X>,
  into: R,
}

impl<'a, 'r, N: Scalar, X: StyleExtension, E: Resolvable<N, X>, R: Resolution<N, X>>
  Resolve<'a, 'r, N, X, E, R>
{
  /// `into` with every one of `entries` resolved by `resolver` written into it, the box
  /// properties then the run properties, each in its visitor's order.
  fn all(
    entries: &'a [(PropertyKey<X::Property>, E)],
    resolver: &'a Resolver<'r, N, X>,
    into: R,
  ) -> R {
    let mut resolve = Self {
      entries,
      resolver,
      into,
    };

    BoxStyle::<N, Plain, X>::for_each_field(&mut resolve);
    RunStyle::<Plain, X>::for_each_field(&mut resolve);

    resolve.into
  }

  fn index(&self, key: PropertyKey<X::Property>) -> Option<usize> {
    self.entries.iter().position(|(entry, _)| *entry == key)
  }
}

impl<N: Scalar, X: StyleExtension, E: Resolvable<N, X>, R: Resolution<N, X>> BoxFieldVisitor<N, X>
  for Resolve<'_, '_, N, X, E, R>
{
  fn field<F: BoxField<N, X>>(&mut self) {
    let Some(index) = self.index(F::KEY)
    else {
      return;
    };
    let value = self.entries[index].1.resolve_box(self.resolver, F::KEY);

    self.into.set_box::<F>(index, value);
  }
}

impl<N: Scalar, X: StyleExtension, E: Resolvable<N, X>, R: Resolution<N, X>> RunFieldVisitor<X>
  for Resolve<'_, '_, N, X, E, R>
{
  fn field<F: RunField<X>>(&mut self) {
    let Some(index) = self.index(F::KEY)
    else {
      return;
    };
    let value = self.entries[index].1.resolve_run(self.resolver);

    self.into.set_run::<F>(index, value);
  }
}

struct ReadResolved<N: Scalar, X: StyleExtension, R> {
  fixed: R,
  target: ResolvedStyle<N, X>,
}

impl<N: Scalar, X: StyleExtension, R: Fn(PropertyKey<X::Property>) -> Option<SlotValue<N, X>>>
  BoxFieldVisitor<N, X> for ReadResolved<N, X, R>
{
  fn field<F: BoxField<N, X>>(&mut self) {
    *F::get_mut(&mut self.target.boxed) =
      (self.fixed)(F::KEY).and_then(SlotValue::read_box::<F::Value>);
  }
}

impl<N: Scalar, X: StyleExtension, R: Fn(PropertyKey<X::Property>) -> Option<SlotValue<N, X>>>
  RunFieldVisitor<X> for ReadResolved<N, X, R>
{
  fn field<F: RunField<X>>(&mut self) {
    *F::get_mut(&mut self.target.run) =
      (self.fixed)(F::KEY).and_then(SlotValue::read_run::<F::Value>);
  }
}

struct AddBox<'a, N: Scalar, X: StyleExtension> {
  record: &'a mut Record<N, X>,
  style: &'a BoxStyle<N, Static<Condition<N>>, X>,
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for AddBox<'_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    if let Some(value) = F::get(self.style) {
      self.record.set_box::<F>(value.clone());
    }
  }
}

struct AddRun<'a, N: Scalar, X: StyleExtension> {
  record: &'a mut Record<N, X>,
  style: &'a RunStyle<Static<Condition<N>>, X>,
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for AddRun<'_, N, X> {
  fn field<F: RunField<X>>(&mut self) {
    if let Some(value) = F::get(self.style) {
      self.record.set_run::<F>(value.clone());
    }
  }
}

struct ReadBox<'a, N: Scalar, X: StyleExtension> {
  record: &'a Record<N, X>,
  target: BoxStyle<N, Static<Condition<N>>, X>,
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for ReadBox<'_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    *F::get_mut(&mut self.target) = self.record.get(F::KEY).and_then(|value| {
      value
        .clone()
        .try_map(|leaf| leaf.try_map(SlotValue::read_box::<F::Value>))
    });
  }
}

struct ReadRun<'a, N: Scalar, X: StyleExtension> {
  record: &'a Record<N, X>,
  target: RunStyle<Static<Condition<N>>, X>,
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for ReadRun<'_, N, X> {
  fn field<F: RunField<X>>(&mut self) {
    *F::get_mut(&mut self.target) = self.record.get(F::KEY).and_then(|value| {
      value
        .clone()
        .try_map(|leaf| leaf.try_map(SlotValue::read_run::<F::Value>))
    });
  }
}

struct FrameBox<'a, N: Scalar, X: StyleExtension> {
  frame: Frame<N, X>,
  keyframe: &'a BoxStyle<N, Keyframe, X>,
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for FrameBox<'_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    if let Some(leaf) = F::get(self.keyframe) {
      self.frame.entries.push((
        F::KEY,
        leaf
          .clone()
          .map(|fixed| SlotValue::Box(fixed.into_box_value())),
      ));
    }
  }
}

struct FrameRun<'a, N: Scalar, X: StyleExtension> {
  frame: Frame<N, X>,
  keyframe: &'a RunStyle<Keyframe, X>,
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for FrameRun<'_, N, X> {
  fn field<F: RunField<X>>(&mut self) {
    if let Some(leaf) = F::get(self.keyframe) {
      self.frame.entries.push((
        F::KEY,
        leaf
          .clone()
          .map(|fixed| SlotValue::Run(fixed.into_run_value())),
      ));
    }
  }
}

struct ReadBoxFrame<'a, N: Scalar, X: StyleExtension> {
  frame: &'a Frame<N, X>,
  target: BoxStyle<N, Keyframe, X>,
}

impl<N: Scalar, X: StyleExtension> BoxFieldVisitor<N, X> for ReadBoxFrame<'_, N, X> {
  fn field<F: BoxField<N, X>>(&mut self) {
    *F::get_mut(&mut self.target) = self
      .frame
      .get(F::KEY)
      .and_then(|leaf| leaf.clone().try_map(SlotValue::read_box::<F::Value>));
  }
}

struct ReadRunFrame<'a, N: Scalar, X: StyleExtension> {
  frame: &'a Frame<N, X>,
  target: RunStyle<Keyframe, X>,
}

impl<N: Scalar, X: StyleExtension> RunFieldVisitor<X> for ReadRunFrame<'_, N, X> {
  fn field<F: RunField<X>>(&mut self) {
    *F::get_mut(&mut self.target) = self
      .frame
      .get(F::KEY)
      .and_then(|leaf| leaf.clone().try_map(SlotValue::read_run::<F::Value>));
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use super::{CompiledStyle, Frame, Keep, Keeping, Record, SlotBinding, coverage::Medium};
  use crate::spine::{
    Animation,
    BoxDynamics,
    BoxField,
    BoxFieldVisitor,
    BoxStyle,
    BoxValue,
    BoxVisitor,
    Color,
    ColorScheme,
    Condition,
    Conditional,
    Dimension,
    Environment,
    Expr,
    Fingerprint,
    Flexibility,
    Inset,
    Interaction,
    IntoBoxValue,
    IntoRunValue,
    ItemAlignment,
    KeptLeaf,
    Keyframe,
    Length,
    Media,
    PointerPrecision,
    Property,
    PropertyKey,
    PropertyName,
    Resolved,
    Resolver,
    RunDynamics,
    RunField,
    RunFieldVisitor,
    RunStyle,
    RunValue,
    RunVisitor,
    Scalar,
    Shape,
    Size,
    Slot,
    SlotLeaf,
    SlotValue,
    State,
    Static,
    StaticLeaf,
    StaticValue,
    StyleExtension,
    StyleProperty,
    TextDecoration,
    TokenName,
    TokenRef,
    Transition,
    box_fields,
    run_fields,
  };

  type Pixel = Condition<f32>;

  /// A medium of the tests' own: a box property and a run property beside the shared.
  #[derive(Debug, Clone, Copy, Default, PartialEq)]
  struct Probe;

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  enum ProbeProperty {
    Tint,
    Bold,
  }

  impl PropertyName for ProbeProperty {
    fn name(&self) -> &'static str {
      match self {
        Self::Tint => "tint",
        Self::Bold => "bold",
      }
    }
  }

  #[derive(Debug, Clone, PartialEq)]
  enum ProbeValue {
    Tint(u16),
    Bold(bool),
  }

  impl Fingerprint for ProbeValue {
    fn fingerprint<H: std::hash::Hasher>(&self, state: &mut H) {
      match self {
        Self::Tint(tint) => tint.fingerprint(state),
        Self::Bold(bold) => bold.fingerprint(state),
      }
    }
  }

  #[derive(Debug, Clone, Default, PartialEq)]
  struct ProbeBox<S: Shape> {
    tint: S::Of<u16>,
  }

  #[derive(Debug, Clone, Default, PartialEq)]
  struct ProbeRun<S: Shape> {
    bold: S::Of<bool>,
  }

  impl<N: Scalar> IntoBoxValue<N, Probe> for u16 {
    fn into_box_value(self) -> BoxValue<N, Probe> {
      BoxValue::Extension(ProbeValue::Tint(self))
    }

    fn from_box_value(value: BoxValue<N, Probe>) -> Option<Self> {
      match value {
        BoxValue::Extension(ProbeValue::Tint(tint)) => Some(tint),
        _ => None,
      }
    }
  }

  impl IntoRunValue<Probe> for bool {
    fn into_run_value(self) -> RunValue<Probe> {
      RunValue::Extension(ProbeValue::Bold(self))
    }

    fn from_run_value(value: RunValue<Probe>) -> Option<Self> {
      match value {
        RunValue::Extension(ProbeValue::Bold(bold)) => Some(bold),
        _ => None,
      }
    }
  }

  struct Tint;

  impl<N: Scalar> BoxField<N, Probe> for Tint {
    type Value = u16;

    const KEY: PropertyKey<ProbeProperty> = PropertyKey::Extension(ProbeProperty::Tint);

    fn get<S: Shape>(style: &BoxStyle<N, S, Probe>) -> &S::Of<u16> {
      &style.ext.tint
    }

    fn get_mut<S: Shape>(style: &mut BoxStyle<N, S, Probe>) -> &mut S::Of<u16> {
      &mut style.ext.tint
    }
  }

  struct Bold;

  impl RunField<Probe> for Bold {
    type Value = bool;

    const KEY: PropertyKey<ProbeProperty> = PropertyKey::Extension(ProbeProperty::Bold);

    fn get<S: Shape>(style: &RunStyle<S, Probe>) -> &S::Of<bool> {
      &style.ext.bold
    }

    fn get_mut<S: Shape>(style: &mut RunStyle<S, Probe>) -> &mut S::Of<bool> {
      &mut style.ext.bold
    }
  }

  impl StyleExtension for Probe {
    type BoxFields<S: Shape> = ProbeBox<S>;
    type Property = ProbeProperty;
    type RunFields<S: Shape> = ProbeRun<S>;
    type Value = ProbeValue;

    fn for_each_box_field<N: Scalar, V: BoxFieldVisitor<N, Self>>(visitor: &mut V) {
      visitor.field::<Tint>();
    }

    fn for_each_run_field<V: RunFieldVisitor<Self>>(visitor: &mut V) {
      visitor.field::<Bold>();
    }
  }

  /// A fixed leaf.
  fn fixed<T, C>(value: T) -> StaticValue<T, C> {
    StaticValue::from(value)
  }

  /// A static leaf where a slot may stand.
  fn kept<T>(leaf: impl Into<StaticLeaf<T>>) -> KeptLeaf<T> {
    KeptLeaf::Static(leaf.into())
  }

  /// A box style holding a fixed, a token, an expression and a slot leaf, each under
  /// variants of every condition kind.
  fn box_style<X: StyleExtension>() -> BoxStyle<f32, Static<Pixel>, X> {
    BoxStyle {
      width: Some(fixed(Dimension::Length(10.)).when(
        Media::min_width(900).into(),
        kept(StaticLeaf::Token(TokenName::new("wide"))),
      )),
      padding: Some(
        StaticValue::from(StaticLeaf::from(TokenRef::<Length>::new("gutter"))).when(
          State::Hovered.into(),
          kept(Expr::<Length>::font_size(0.5) + 2.),
        ),
      ),
      aspect_ratio: Some(StaticValue::from(StaticLeaf::from(
        Expr::from(TokenRef::<f32>::new("ratio")) * 2.,
      ))),
      background: Some(Conditional::new(KeptLeaf::Slot(Slot(0))).when(
        Condition::and([Media::Dark.into(), State::Pressed.into()]),
        KeptLeaf::Static(StaticLeaf::Fixed(Color::srgb(0., 0., 1.))),
      )),
      self_alignment: Some(fixed(ItemAlignment::Center).when(
        Condition::or([Media::Hover.into(), State::Focused.into()]),
        kept(ItemAlignment::End),
      )),
      flex: Some(fixed(Flexibility::grow(1.))),
      margin: Some(fixed(Inset::all(Dimension::Fraction(0.25)))),
      ..Default::default()
    }
  }

  /// A run style holding a token, a slot and a fixed leaf.
  fn run_style<X: StyleExtension>() -> RunStyle<Static<Pixel>, X> {
    RunStyle {
      color: Some(
        StaticValue::from(StaticLeaf::from(TokenRef::<Color>::new("ink")))
          .when(State::Hovered.into(), KeptLeaf::Slot(Slot(1))),
      ),
      decoration: Some(fixed(TextDecoration::Underline)),
      ..Default::default()
    }
  }

  /// The keys a style's visitor visits for the fields it sets, in order.
  struct SetKeys<P>(Vec<PropertyKey<P>>);

  impl<X: StyleExtension> BoxVisitor<f32, Static<Pixel>, X> for SetKeys<X::Property> {
    fn value<T: IntoBoxValue<f32, X>>(
      &mut self,
      key: PropertyKey<X::Property>,
      value: &Option<StaticValue<T, Pixel>>,
    ) {
      if value.is_some() {
        self.0.push(key);
      }
    }
  }

  impl<X: StyleExtension> RunVisitor<Static<Pixel>, X> for SetKeys<X::Property> {
    fn value<T: IntoRunValue<X>>(
      &mut self,
      key: PropertyKey<X::Property>,
      value: &Option<StaticValue<T, Pixel>>,
    ) {
      if value.is_some() {
        self.0.push(key);
      }
    }
  }

  /// A record reads back exactly the styles projected into it, every leaf kind under
  /// every condition kind included, over no extension and over one.
  #[test]
  fn styles_read_back_whole() {
    let mut record = Record::<f32>::new();

    record.add_box(&box_style());
    record.add_run(&run_style());

    assert_eq!(record.box_style(), box_style());
    assert_eq!(record.run_style(), run_style());

    let mut probe_box = box_style::<Probe>();
    let mut probe_run = run_style::<Probe>();

    probe_box.ext.tint = Some(fixed(7).when(Media::Light.into(), KeptLeaf::Slot(Slot(2))));
    probe_run.ext.bold = Some(fixed(true).when(State::Disabled.into(), kept(false)));

    let mut record = Record::<f32, Probe>::new();

    record.add_box(&probe_box);
    record.add_run(&probe_run);

    assert_eq!(record.box_style(), probe_box);
    assert_eq!(record.run_style(), probe_run);
    assert!(record.has(PropertyKey::Extension(ProbeProperty::Tint)));
    assert_eq!(
      record
        .get(PropertyKey::Extension(ProbeProperty::Bold))
        .map(|value| value.base().clone()),
      Some(KeptLeaf::Static(StaticLeaf::Fixed(SlotValue::Run(
        RunValue::Extension(ProbeValue::Bold(true))
      ))))
    );
  }

  /// Entries follow the order the styles' visitors give, the declaration order, the
  /// medium's own last.
  #[test]
  fn entries_follow_the_visitors_order() {
    let mut boxed = box_style::<Probe>();
    let mut run = run_style::<Probe>();

    boxed.ext.tint = Some(fixed(3));
    run.ext.bold = Some(fixed(true));
    run.selection_color = Some(fixed(Color::srgb(0., 0., 0.)));

    let mut box_record = Record::<f32, Probe>::new();
    let mut run_record = Record::<f32, Probe>::new();
    let mut box_keys = SetKeys(Vec::new());
    let mut run_keys = SetKeys(Vec::new());

    box_record.add_box(&boxed);
    run_record.add_run(&run);
    boxed.visit(&mut box_keys);
    run.visit(&mut run_keys);

    let names = |record: &Record<f32, Probe>| {
      record
        .entries()
        .iter()
        .map(|(key, _)| key.name())
        .collect::<Vec<_>>()
    };

    assert_eq!(
      box_record
        .entries()
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>(),
      box_keys.0
    );
    assert_eq!(
      run_record
        .entries()
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>(),
      run_keys.0
    );
    assert_eq!(
      names(&box_record),
      [
        "width",
        "aspect_ratio",
        "margin",
        "padding",
        "background",
        "flex",
        "self_alignment",
        "tint"
      ]
    );
    assert_eq!(
      names(&run_record),
      ["text_color", "selection_color", "decoration", "bold"]
    );
    assert!(!box_record.has(PropertyKey::Shared(Property::Height)));
  }

  /// A typed write replaces its property's entry where it stands and appends a new one.
  #[test]
  fn typed_writes_replace_in_place() {
    let mut record = Record::<f32>::new();

    record.set_box::<box_fields::Width>(fixed(Dimension::Length(4.)));
    record.set_box::<box_fields::Padding>(fixed(Inset::all(2)));
    record.set_box::<box_fields::Width>(fixed(Dimension::Auto));

    assert_eq!(
      record
        .entries()
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>(),
      [
        PropertyKey::Shared(Property::Width),
        PropertyKey::Shared(Property::Padding)
      ]
    );
    assert_eq!(record.box_style().width, Some(fixed(Dimension::Auto)));

    let mut record = Record::<f32>::new();

    record.set_run::<run_fields::TextColor>(fixed(Color::srgb(1., 0., 0.)));
    record.set_run::<run_fields::Decoration>(fixed(TextDecoration::Underline));
    record.set_run::<run_fields::TextColor>(
      fixed(Color::srgb(0., 1., 0.)).when(State::Hovered.into(), KeptLeaf::Slot(Slot(3))),
    );

    assert_eq!(
      record
        .entries()
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>(),
      [
        PropertyKey::Shared(Property::TextColor),
        PropertyKey::Shared(Property::Decoration)
      ]
    );
    assert_eq!(
      record.run_style().color,
      Some(fixed(Color::srgb(0., 1., 0.)).when(State::Hovered.into(), KeptLeaf::Slot(Slot(3))))
    );
  }

  /// A run's layout lands after the run's own entries, as a run is compiled.
  #[test]
  fn a_run_keeps_its_entries_before_its_layout() {
    let layout = BoxStyle::<f32, Static<Pixel>> {
      flex: Some(fixed(Flexibility::grow(2.))),
      self_alignment: Some(fixed(ItemAlignment::Start)),
      ..Default::default()
    };
    let mut record = Record::<f32>::new();

    record.add_run(&run_style());
    record.add_box(&layout);

    assert_eq!(
      record
        .entries()
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>(),
      [
        PropertyKey::Shared(Property::TextColor),
        PropertyKey::Shared(Property::Decoration),
        PropertyKey::Shared(Property::Flex),
        PropertyKey::Shared(Property::SelfAlignment),
      ]
    );
    assert_eq!(record.box_style(), layout);
    assert_eq!(record.run_style(), run_style());
  }

  /// A box keyframe of a fixed, a token and an expression leaf.
  fn box_keyframe(width: f32) -> BoxStyle<f32, Keyframe, Probe> {
    BoxStyle {
      width: Some(StaticLeaf::Fixed(Dimension::Length(width))),
      background: Some(StaticLeaf::Token(TokenName::new("accent"))),
      padding: Some(StaticLeaf::from(Expr::<Length>::viewport_width(0.1))),
      ext: ProbeBox {
        tint: Some(StaticLeaf::Fixed(9)),
      },
      ..Default::default()
    }
  }

  fn box_dynamics() -> BoxDynamics<f32, Probe> {
    BoxDynamics {
      transitions: vec![
        Transition::new(Property::Background, 0.2),
        Transition::new(StyleProperty::Extension(ProbeProperty::Tint), 0.1).set_delay(0.05),
      ],
      animations: vec![
        Animation::new(1., [(0., box_keyframe(0.)), (100., box_keyframe(20.))]).set_iterations(3),
      ],
    }
  }

  fn run_dynamics() -> RunDynamics<Probe> {
    RunDynamics {
      transitions: vec![Transition::new(StyleProperty::All, 0.3)],
      animations: vec![Animation::new(
        0.5,
        [(
          50.,
          RunStyle {
            color: Some(StaticLeaf::Fixed(Color::srgb(1., 0., 0.))),
            ext: ProbeRun {
              bold: Some(StaticLeaf::Fixed(true)),
            },
            ..Default::default()
          },
        )],
      )],
    }
  }

  /// A record's transitions are the dynamics' own, and its animations' frames read back
  /// as their keyframes.
  #[test]
  fn dynamics_read_back() {
    let mut record = Record::<f32, Probe>::new();

    record.add_box_dynamics(&box_dynamics());
    record.add_run_dynamics(&run_dynamics());

    let transitions = [box_dynamics().transitions, run_dynamics().transitions].concat();

    assert_eq!(record.transitions(), transitions.as_slice());
    assert_eq!(record.animations().len(), 2);
    assert_eq!(
      record.animations()[0].map_keyframes(Frame::box_keyframe),
      box_dynamics().animations[0]
    );
    assert_eq!(
      record.animations()[1].map_keyframes(Frame::run_keyframe),
      run_dynamics().animations[0]
    );
    assert_eq!(record.animations()[0].iterations, 3.into());
    assert_eq!(
      Frame::of_box(&box_keyframe(5.))
        .entries()
        .iter()
        .map(|(key, _)| key.name())
        .collect::<Vec<_>>(),
      ["width", "padding", "background", "tint"]
    );
    assert_eq!(
      Frame::of_box(&box_keyframe(5.)).entries()[0].1,
      SlotLeaf::<f32, Probe>::Fixed(SlotValue::Box(BoxValue::Dimension(Dimension::Length(5.))))
    );
  }

  /// A record compiled at `Keep::Nothing` resolves as its typed styles do through the
  /// same resolver, slots and tokens and expressions alike, and a frame as its keyframe
  /// does.
  #[test]
  fn resolution_is_the_typed_styles_own() {
    let environment = Environment {
      viewport: Size {
        width: 1000.,
        height: 800.,
      },
      scheme: ColorScheme::Dark,
      reduced_motion: false,
      reduced_transparency: false,
      high_contrast: false,
      pointer: PointerPrecision::Fine,
      hover: true,
      safe_area: Inset::all(0),
    };
    let token = |name: TokenName| match name.as_str() {
      "wide" => Some(Resolved::Dimension(Dimension::Length(640.))),
      "gutter" => Some(Resolved::Length(12.)),
      "ratio" => Some(Resolved::Number(1.5)),
      "ink" | "accent" => Some(Resolved::Color(Color::srgb(0., 1., 0.))),
      _ => None,
    };
    let slot = |slot: Slot| match slot {
      Slot(0) => Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Color(
        Color::srgb(1., 1., 1.),
      )))),
      Slot(1) => Some(StaticLeaf::Token(TokenName::new("accent"))),
      Slot(2) => Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Extension(
        ProbeValue::Tint(4),
      )))),
      _ => None,
    };
    let read = Cell::new(false);

    for interaction in [
      Interaction::default(),
      Interaction {
        hovered: true,
        pressed: true,
        ..Default::default()
      },
    ] {
      let resolver = Resolver::<f32, Probe> {
        environment,
        interaction,
        interaction_read: &read,
        containers: &[],
        font_size: 16.,
        token: &token,
        slot: &slot,
      };
      let mut boxed = box_style::<Probe>();

      boxed.ext.tint = Some(fixed(1).when(State::Hovered.into(), KeptLeaf::Slot(Slot(2))));

      let mut record = Record::<f32, Probe>::new();

      record.add_box(&boxed);
      record.add_run(&run_style());

      let style = record.compile(Keeping::Nothing(&resolver)).resolved();

      assert_eq!(style.boxed, resolver.box_style(&boxed));
      assert_eq!(style.run, resolver.run_style(&run_style()));
      assert_eq!(
        style.boxed.padding,
        Some(Inset::all(if interaction.hovered { 10 } else { 12 }))
      );
      assert_eq!(style.boxed.width, Some(Dimension::Length(640.)));
      assert_eq!(style.boxed.aspect_ratio.map(|ratio| ratio.0), Some(3.));
      assert_eq!(
        style.boxed.background,
        Some(if interaction.pressed {
          Color::srgb(0., 0., 1.)
        }
        else {
          Color::srgb(1., 1., 1.)
        })
      );
      assert_eq!(
        style.boxed.ext.tint,
        Some(if interaction.hovered { 4 } else { 1 })
      );
      assert_eq!(style.run.color, Some(Color::srgb(0., 1., 0.)));

      let frame = Frame::of_box(&box_keyframe(8.))
        .compile(Keeping::Nothing(&resolver))
        .resolved();
      let run_frame = Frame::of_run(&run_dynamics().animations[0].keyframes[0].1)
        .compile(Keeping::Nothing(&resolver))
        .resolved();

      assert_eq!(frame.boxed, resolver.box_keyframe(&box_keyframe(8.)));
      assert_eq!(frame.boxed.padding, Some(Inset::all(100)));
      assert_eq!(
        run_frame.run,
        resolver.run_keyframe(&run_dynamics().animations[0].keyframes[0].1)
      );
      assert_eq!(run_frame.run.ext.bold, Some(true));
    }

    assert!(read.get());
  }

  /// The record of the tests' box and run styles over the probe medium, with their
  /// motion.
  fn probe_record() -> Record<f32, Probe> {
    let mut boxed = box_style::<Probe>();

    boxed.ext.tint = Some(fixed(7).when(Media::Light.into(), KeptLeaf::Slot(Slot(2))));

    let mut record = Record::<f32, Probe>::new();

    record.add_box(&boxed);
    record.add_run(&run_style());
    record.add_box_dynamics(&box_dynamics());
    record.add_run_dynamics(&run_dynamics());

    record
  }

  /// At `Keep::Everything` a compiled style keeps the record whole, binds each slot to
  /// its property and position in the entries' order, and is addressed by its static
  /// part alone.
  #[test]
  fn everything_keeps_the_record_and_binds_its_slots() {
    let record = probe_record();
    let compiled = record.compile(Keeping::Everything);

    assert_eq!(compiled.keep(), Keep::Everything);
    assert_eq!(compiled.entries(), record.entries());
    assert_eq!(compiled.transitions(), record.transitions());
    assert_eq!(compiled.animations(), record.animations());
    assert_eq!(
      compiled.slots(),
      [
        SlotBinding {
          slot: Slot(0),
          property: PropertyKey::Shared(Property::Background),
          position: 0,
        },
        SlotBinding {
          slot: Slot(2),
          property: PropertyKey::Extension(ProbeProperty::Tint),
          position: 1,
        },
        SlotBinding {
          slot: Slot(1),
          property: PropertyKey::Shared(Property::TextColor),
          position: 1,
        },
      ]
    );
    assert_eq!(
      compiled.resolved().boxed,
      BoxStyle {
        flex: Some(Flexibility::grow(1.)),
        margin: Some(Inset::all(Dimension::Fraction(0.25))),
        ..Default::default()
      },
      "only a fixed leaf under no condition reads as resolved"
    );
    assert_eq!(
      compiled.address(),
      probe_record().compile(Keeping::Everything).address()
    );
    assert_eq!(
      probe_record().into_kept(),
      compiled,
      "a record kept by value compiles as one compiled at `Keep::Everything`"
    );

    let mut moved = probe_record();

    moved.set_box::<box_fields::Width>(fixed(Dimension::Length(11.)));

    let mut timed = probe_record();

    timed.add_run_dynamics(&RunDynamics {
      transitions: vec![Transition::new(Property::TextColor, 0.1)],
      animations: Vec::new(),
    });

    assert_eq!(
      timed.clone().into_kept(),
      timed.compile(Keeping::Everything),
      "a record kept by value keeps its motion"
    );

    let mut rebound = Record::<f32, Probe>::new();

    rebound.set_run::<run_fields::TextColor>(
      fixed(Color::TRANSPARENT).when(State::Hovered.into(), KeptLeaf::Slot(Slot(4))),
    );

    let mut bound = Record::<f32, Probe>::new();

    bound.set_run::<run_fields::TextColor>(
      fixed(Color::TRANSPARENT).when(State::Hovered.into(), KeptLeaf::Slot(Slot(5))),
    );

    let addresses = [
      compiled.address(),
      moved.compile(Keeping::Everything).address(),
      timed.compile(Keeping::Everything).address(),
      rebound.compile(Keeping::Everything).address(),
      bound.compile(Keeping::Everything).address(),
      Record::<f32, Probe>::new()
        .compile(Keeping::Everything)
        .address(),
    ];

    for (index, address) in addresses.iter().enumerate() {
      assert!(
        !addresses[..index].contains(address),
        "a style differing in an entry, a slot or its motion has an address of its own"
      );
    }
  }

  /// At `Keep::Nothing` every entry is its resolved value, a fixed leaf under no
  /// condition in the record's order, no slot is bound, every frame's leaves are fixed,
  /// and the address follows the values resolved.
  #[test]
  fn nothing_resolves_every_entry_in_place() {
    let environment = Environment {
      viewport: Size {
        width: 1000.,
        height: 800.,
      },
      scheme: ColorScheme::Light,
      reduced_motion: false,
      reduced_transparency: false,
      high_contrast: false,
      pointer: PointerPrecision::Fine,
      hover: true,
      safe_area: Inset::all(0),
    };
    let token = |name: TokenName| match name.as_str() {
      "wide" => Some(Resolved::Dimension(Dimension::Length(640.))),
      "gutter" => Some(Resolved::Length(12.)),
      "ink" | "accent" => Some(Resolved::Color(Color::srgb(0., 1., 0.))),
      _ => None,
    };
    let slot = |slot: Slot| match slot {
      Slot(0) => Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Color(
        Color::srgb(1., 1., 1.),
      )))),
      Slot(2) => Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Extension(
        ProbeValue::Tint(4),
      )))),
      _ => None,
    };
    let read = Cell::new(false);
    let compile = |hovered: bool| -> CompiledStyle<f32, Probe> {
      let resolver = Resolver::<f32, Probe> {
        environment,
        interaction: Interaction {
          hovered,
          ..Default::default()
        },
        interaction_read: &read,
        containers: &[],
        font_size: 16.,
        token: &token,
        slot: &slot,
      };

      probe_record().compile(Keeping::Nothing(&resolver))
    };
    let idle = compile(false);

    assert_eq!(idle.keep(), Keep::Nothing);
    assert_eq!(idle.slots(), []);
    assert!(read.get(), "the states were read");
    assert_eq!(
      idle
        .entries()
        .iter()
        .map(|(key, _)| key.name())
        .collect::<Vec<_>>(),
      [
        "width",
        "margin",
        "padding",
        "background",
        "flex",
        "self_alignment",
        "tint",
        "text_color",
        "decoration"
      ],
      "the ratio, of an undefined token, resolves to nothing and is left out"
    );
    assert!(idle.entries().iter().all(|(_, value)| {
      value.variants().is_empty() && matches!(value.base(), KeptLeaf::Static(StaticLeaf::Fixed(_)))
    }));
    assert_eq!(idle.resolved().boxed.ext.tint, Some(4));
    assert_eq!(idle.resolved().boxed.width, Some(Dimension::Length(640.)));
    assert_eq!(idle.transitions(), probe_record().transitions());
    assert!(idle.animations().iter().all(|animation| {
      animation.keyframes.iter().all(|(_, frame)| {
        frame
          .entries()
          .iter()
          .all(|(_, leaf)| matches!(leaf, StaticLeaf::Fixed(_)))
      })
    }));
    assert_eq!(
      idle.animations()[0].keyframes[0].1.resolved().boxed.padding,
      Some(Inset::all(100))
    );
    assert_eq!(idle.address(), compile(false).address());
    assert_ne!(idle.address(), compile(true).address());
    assert_ne!(
      idle.address(),
      probe_record().compile(Keeping::Everything).address()
    );
  }

  /// A style kept at `Keep::Everything` resolves at a node as the record compiled there
  /// at `Keep::Nothing` reads, and each of its frames as the frame compiled there does,
  /// reading the interaction alike.
  #[test]
  fn a_kept_style_resolves_as_one_compiled_at_nothing() {
    let environment = Environment {
      viewport: Size {
        width: 1000.,
        height: 800.,
      },
      scheme: ColorScheme::Dark,
      reduced_motion: false,
      reduced_transparency: false,
      high_contrast: false,
      pointer: PointerPrecision::Fine,
      hover: true,
      safe_area: Inset::all(0),
    };
    let token = |name: TokenName| match name.as_str() {
      "wide" => Some(Resolved::Dimension(Dimension::Length(640.))),
      "gutter" => Some(Resolved::Length(12.)),
      "ratio" => Some(Resolved::Number(1.5)),
      "ink" | "accent" => Some(Resolved::Color(Color::srgb(0., 1., 0.))),
      _ => None,
    };
    let slot = |slot: Slot| match slot {
      Slot(0) => Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Color(
        Color::srgb(1., 1., 1.),
      )))),
      Slot(1) => Some(StaticLeaf::Token(TokenName::new("accent"))),
      Slot(2) => Some(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Extension(
        ProbeValue::Tint(4),
      )))),
      _ => None,
    };
    let record = probe_record();
    let kept = record.compile(Keeping::Everything);

    assert_eq!(kept.animations().len(), 2, "a box's and a run's animation");

    for interaction in [
      Interaction::default(),
      Interaction {
        hovered: true,
        pressed: true,
        ..Default::default()
      },
    ] {
      let read = Cell::new(false);
      let resolver = Resolver::<f32, Probe> {
        environment,
        interaction,
        interaction_read: &read,
        containers: &[],
        font_size: 16.,
        token: &token,
        slot: &slot,
      };
      let compiled = record.compile(Keeping::Nothing(&resolver));
      let compiled_read = read.replace(false);
      let direct = kept.resolve(&resolver);

      assert_eq!(direct, compiled.resolved());
      assert!(compiled_read, "a state condition reads the interaction");
      assert_eq!(read.get(), compiled_read, "the interaction is read alike");
      assert_eq!(
        direct.boxed.ext.tint,
        Some(7),
        "a dark scheme keeps the base"
      );

      for (kept, compiled) in kept.animations().iter().zip(compiled.animations()) {
        for ((_, kept), (_, compiled)) in kept.keyframes.iter().zip(&compiled.keyframes) {
          assert_eq!(kept.resolve(&resolver), compiled.resolved());
          assert_eq!(compiled.resolve(&resolver), compiled.resolved());
        }
      }
    }
  }

  /// Every shared property, condition, expression and leaf kind crosses a record whole and
  /// resolves through it as the typed style does, in pixels and in cells.
  #[test]
  fn every_kind_crosses_the_record() {
    Medium::<f32, ()>::shared().check();
    Medium::<i32, ()>::shared().check();
  }
}
