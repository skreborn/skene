//! A node's style as one record: every property it sets under Skene's name for it, the
//! value erased over the property, and its motion as data.

use super::{
  Animation,
  BoxDynamics,
  BoxField,
  BoxFieldVisitor,
  BoxStyle,
  Condition,
  IntoBoxValue,
  IntoRunValue,
  Keyframe,
  Plain,
  PropertyKey,
  Resolver,
  RunDynamics,
  RunField,
  RunFieldVisitor,
  RunStyle,
  Scalar,
  SlotLeaf,
  SlotValue,
  Static,
  StaticValue,
  StyleExtension,
  Transition,
};

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
/// It is written typed, by a property's [`set_box`](Self::set_box) or
/// [`set_run`](Self::set_run) or a whole style's projection
/// ([`add_box`](Self::add_box), [`add_run`](Self::add_run) and their dynamics), so a value
/// of another property's kind does not compile, and read erased, entry by entry in the
/// order the styles' visitors give. [`box_style`](Self::box_style) and
/// [`run_style`](Self::run_style) read the typed styles back, and
/// [`resolve`](Self::resolve) resolves them through the one [`Resolver`].
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

  /// Every property the record sets, resolved at the node `resolver` reads: the typed
  /// styles read back, resolved as [`Resolver::box_style`] and [`Resolver::run_style`]
  /// resolve them.
  pub fn resolve(&self, resolver: &Resolver<'_, N, X>) -> ResolvedStyle<N, X> {
    ResolvedStyle {
      boxed: resolver.box_style(&self.box_style()),
      run: resolver.run_style(&self.run_style()),
    }
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

  /// Every leaf of the frame resolved at the node `resolver` reads: the typed keyframes
  /// read back, resolved as [`Resolver::box_keyframe`] and [`Resolver::run_keyframe`]
  /// resolve them.
  pub fn resolve(&self, resolver: &Resolver<'_, N, X>) -> ResolvedStyle<N, X> {
    ResolvedStyle {
      boxed: resolver.box_keyframe(&self.box_keyframe()),
      run: resolver.run_keyframe(&self.run_keyframe()),
    }
  }
}

/// A box value's reading of an erased one.
fn box_reading<N: Scalar, X: StyleExtension, T: IntoBoxValue<N, X>>(
  value: SlotValue<N, X>,
) -> Option<T> {
  match value {
    SlotValue::Box(value) => T::from_box_value(value),
    SlotValue::Run(_) => None,
  }
}

/// A run value's reading of an erased one.
fn run_reading<N: Scalar, X: StyleExtension, T: IntoRunValue<X>>(
  value: SlotValue<N, X>,
) -> Option<T> {
  match value {
    SlotValue::Run(value) => T::from_run_value(value),
    SlotValue::Box(_) => None,
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
        .try_map(|leaf| leaf.try_map(box_reading::<N, X, F::Value>))
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
        .try_map(|leaf| leaf.try_map(run_reading::<N, X, F::Value>))
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
      .and_then(|leaf| leaf.clone().try_map(box_reading::<N, X, F::Value>));
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
      .and_then(|leaf| leaf.clone().try_map(run_reading::<N, X, F::Value>));
  }
}

#[cfg(test)]
mod tests {
  use std::cell::Cell;

  use super::{Frame, Record};
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
        Media::MinWidth(900.).into(),
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

  impl<X: StyleExtension> crate::spine::RunVisitor<Static<Pixel>, X> for SetKeys<X::Property> {
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

  /// Entries follow the order the styles' visitors give, the medium's own last.
  #[test]
  fn entries_follow_the_visitors_order() {
    let mut style = box_style::<Probe>();

    style.ext.tint = Some(fixed(3));

    let mut record = Record::<f32, Probe>::new();
    let mut keys = SetKeys(Vec::new());

    record.add_box(&style);
    style.visit(&mut keys);

    assert_eq!(
      record
        .entries()
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>(),
      keys.0
    );
    assert_eq!(
      keys.0.last(),
      Some(&PropertyKey::Extension(ProbeProperty::Tint))
    );
    assert!(!record.has(PropertyKey::Shared(Property::Height)));
  }

  /// A typed write replaces its property's entry where it stands and appends a new one.
  #[test]
  fn set_box_replaces_in_place() {
    let mut record = Record::<f32>::new();

    record.set_box::<box_fields::Width>(fixed(Dimension::Length(4.)));
    record.set_box::<box_fields::Padding>(fixed(Inset::all(2.)));
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

  /// A record resolves as its typed styles do through the same resolver, slots and
  /// tokens and expressions alike, and a frame as its keyframe does.
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
      safe_area: Inset::all(0.),
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

      let style = record.resolve(&resolver);

      assert_eq!(style.boxed, resolver.box_style(&boxed));
      assert_eq!(style.run, resolver.run_style(&run_style()));
      assert_eq!(
        style.boxed.padding,
        Some(Inset::all(if interaction.hovered { 10. } else { 12. }))
      );

      let frame = Frame::of_box(&box_keyframe(8.));

      assert_eq!(
        frame.resolve(&resolver).boxed,
        resolver.box_keyframe(&box_keyframe(8.))
      );
    }

    assert!(read.get());
  }
}
