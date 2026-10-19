//! A radio group and its radios.

use std::rc::Rc;

use crate::{
  Alignment, AxisAlignment, Border, Dimension, Signal, Spacing, State, SystemColors, Value,
  behavior::{self, RadioGroup as Group},
  layout::Axis,
  seam::{Realization, control::Part},
  view::{
    AnyKind, AnyView, AsPart, AsPartBase, Column, ColumnBase, Container, ContainerBase,
    ControlBase, ControlMembersBase, Cx, DecorationBase, Node, Painted, PaintedBase, RunBase, View,
    Wraps,
    controls::{control_surfaces, focus_ring, marks},
    wraps_itself,
  },
};

/// One option of a [`RadioGroup`], as its option builder makes it: a radio with its caption.
/// Its group gives it its behavior ([`crate::behavior::Radio`]) as it builds it.
///
/// Its parts: its face ([`Radio::default_face`]) holding its mark ([`Radio::default_mark`]),
/// then its caption, 8 pixels after. Every member of a control applies to it, so one option
/// can be disabled.
#[must_use]
pub struct Radio<R: Realization> {
  /// The control it composes, its defaults set, over a behavior its group replaces.
  control: crate::view::Control<behavior::Radio, R>,
  /// The face the mark sits in.
  face: Container<R>,
  /// The mark.
  mark: AnyView<R>,
}

wraps_itself!(Radio<R> where R: Realization);

control_surfaces!([R: Realization] Radio<R>, R => control);

impl<R: Realization> Radio<R> {
  /// The default face: a circle 18 pixels across, a border of 1.5 in
  /// [`SystemColors::BORDER`] around [`SystemColors::FIELD`] and its hover and pressed
  /// variants, the accent while chosen, its mark centered and inked in
  /// [`SystemColors::ON_ACCENT`]; a ring of 2 in the accent while focus shows.
  pub fn default_face() -> Container<R> {
    Container::new()
      .set_width(18)
      .set_height(18)
      .set_corner_radius(9)
      .set_border(Border::new(1.5))
      .set_alignment(Alignment::CENTER)
      .set_background(
        Value::from(SystemColors::FIELD)
          .when(State::Hovered, SystemColors::CONTROL_HOVER)
          .when(State::Pressed, SystemColors::CONTROL_PRESSED)
          .when(State::Disabled, SystemColors::FIELD)
          .when(State::Checked, SystemColors::ACCENT),
      )
      .set_border_color(
        Value::from(SystemColors::BORDER).when(State::Checked, SystemColors::ACCENT),
      )
      .set_color(SystemColors::ON_ACCENT)
      .set_ring(focus_ring())
      .set_ring_color(SystemColors::ACCENT)
  }

  /// The default mark: [`marks::dot`], filling the face.
  pub fn default_mark() -> Painted<R> {
    Painted::new(marks::dot)
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Fraction(1.))
  }

  /// The control this radio is, over `behavior`.
  fn into_control(self, behavior: behavior::Radio) -> crate::view::Control<behavior::Radio, R> {
    let face = self
      .face
      .set_child(AsPart::new(Part::Mark).set_child(self.mark));

    self
      .control
      .map_behavior(|_| behavior)
      .add_part(Part::Face, face)
  }
}

/// What a [`Radio`] carries beside every control's members: its face and its mark.
pub trait RadioBase<R: Realization>: Wraps<Inner = Radio<R>> {
  /// A radio labelled `label`.
  fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self::wrap(Radio {
      control: crate::view::Control::new(Group::new().member(0))
        .set_item_alignment(AxisAlignment::Center)
        .set_spacing(8)
        .set_label(label),
      face: Radio::<R>::default_face(),
      mark: Radio::<R>::default_mark().into_any(),
    })
  }

  /// The face, in place of [`Radio::default_face`]; the mark is placed in it, so a child it
  /// holds is replaced.
  fn set_face(mut self, face: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().face = face.into_inner();

    self
  }

  /// The mark, in place of [`Radio::default_mark`]: any view, drawn under the radio's states
  /// (`State::Checked` while chosen).
  fn set_mark(mut self, mark: impl View<R>) -> Self {
    self.inner_mut().mark = mark.into_any();

    self
  }
}

impl<R: Realization> RadioBase<R> for Radio<R> {}

/// What makes a group's radio from its index and its option's text.
type OptionBuilder<R> = Rc<dyn Fn(usize, &str) -> Radio<R>>;

/// One choice of a few, all shown, with its caption: Skene's radio group, drawn on every
/// platform. It is a [`crate::view::Control`] over a [`crate::behavior::RadioGroup`], its
/// caption above a column of radios 6 pixels apart, each from its option builder
/// ([`RadioGroupBase::set_option_builder`]).
///
/// The group is one tab stop, on its chosen radio (its first enabled where none is); the
/// arrows move to the next or the previous enabled radio, wrapping, and choose it; `Space`
/// and a click choose. A disabled group disables its radios.
///
/// It is controlled: it shows what [`RadioGroupBase::derive_chosen`] returns, asks
/// [`RadioGroupBase::handle_choose`] for each choice, and shows the truth.
#[must_use]
pub struct RadioGroup<R: Realization> {
  /// The control it composes, its defaults set.
  control: crate::view::Control<Group, R>,
  /// The options' texts.
  options: Rc<dyn Fn() -> Vec<String>>,
  /// The space between the options.
  option_spacing: Value<Spacing>,
  /// What makes each option's radio.
  builder: OptionBuilder<R>,
}

wraps_itself!(RadioGroup<R> where R: Realization);

control_surfaces!([R: Realization] RadioGroup<R>, R => control);

/// What a [`RadioGroup`] carries beside every control's members: its choice, its options, the
/// space between them and what makes each.
pub trait RadioGroupBase<R: Realization>: Wraps<Inner = RadioGroup<R>> {
  /// A radio group labelled `label`, with no options.
  fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self::wrap(RadioGroup {
      control: crate::view::Control::new(Group::new())
        .set_axis(Axis::Vertical)
        .set_spacing(6)
        .set_label(label),
      options: Rc::new(Vec::new),
      option_spacing: Value::from(6),
      builder: Rc::new(|_, name| Radio::new(name)),
    })
  }

  /// The chosen option's index, or none.
  fn set_chosen(self, chosen: Option<usize>) -> Self {
    self.map_inner(|mut group| {
      group.control = group
        .control
        .map_behavior(|behavior| behavior.set_chosen(chosen));

      group
    })
  }

  /// The chosen option's index, or none, kept current.
  fn derive_chosen(self, chosen: impl Fn() -> Option<usize> + 'static) -> Self {
    self.map_inner(|mut group| {
      group.control = group
        .control
        .map_behavior(|behavior| behavior.derive_chosen(chosen));

      group
    })
  }

  /// Runs `choose` with the option the reader chose.
  fn handle_choose(self, choose: impl Fn(Option<usize>) + 'static) -> Self {
    self.map_inner(|mut group| {
      group.control = group
        .control
        .map_behavior(|behavior| behavior.handle_choose(choose));

      group
    })
  }

  /// Shows `chosen` and writes each choice into it.
  fn bind_chosen(self, chosen: Signal<Option<usize>>) -> Self {
    self.map_inner(|mut group| {
      group.control = group
        .control
        .map_behavior(|behavior| behavior.bind_chosen(chosen));

      group
    })
  }

  /// The space between the options, in pixels; 6 where unset.
  fn set_option_spacing(mut self, spacing: impl Into<Value<Spacing>>) -> Self {
    self.inner_mut().option_spacing = spacing.into();

    self
  }

  /// The space between the options, in pixels, kept current; 6 where unset.
  fn derive_option_spacing<V: Into<Value<Spacing>>>(
    self,
    spacing: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_option_spacing(Value::derived(spacing))
  }

  /// Makes each option's radio with `builder`, from its index and its text: a styled
  /// [`Radio`]; `Radio::new(text)` where unset.
  fn set_option_builder<O: Wraps<Inner = Radio<R>>>(
    mut self,
    builder: impl Fn(usize, &str) -> O + 'static,
  ) -> Self {
    self.inner_mut().builder = Rc::new(move |index, name| builder(index, name).into_inner());

    self
  }

  /// The options' texts.
  fn set_options<S: Into<String>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Vec<String> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.clone())
  }

  /// The options' texts, kept current: a radio whose text changes is built again.
  fn derive_options<S: Into<String>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.inner_mut().options = Rc::new(move || options().into_iter().map(Into::into).collect());

    self
  }
}

impl<R: Realization> RadioGroupBase<R> for RadioGroup<R> {}

impl<R: Realization> AnyKind for RadioGroup<R> {}

impl<R: Realization> View<R> for RadioGroup<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let RadioGroup {
      control,
      options,
      option_spacing,
      builder,
    } = self;
    let roster = control.behavior().roster();
    let radios = Column::new().set_spacing(option_spacing).derive_children(
      move || options().into_iter().enumerate().collect(),
      |(index, name)| (*index, name.clone()),
      move |(index, name): (usize, String)| {
        let member = Group::with_roster(Rc::clone(&roster)).member(index);

        builder(index, &name).into_control(member)
      },
    );

    control.set_child(radios).build(cx)
  }
}
