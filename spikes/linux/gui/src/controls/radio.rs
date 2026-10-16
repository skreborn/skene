//! A radio group and its radios.

use std::rc::Rc;

use skene_foundation::{
  Alignment, AxisAlignment, Border, Dimension, Spacing, State, SystemColors, Value,
  behavior::{self, RadioGroup as Group},
  layout::Axis,
  seam::control::Part,
};

use crate::{
  AsPart, Column, Container, Control, Painted, Signal,
  control::{Members, control_members},
  controls::{focus_ring, marks},
  view::{AnyView, Cx, Node, View},
};

/// One option of a [`RadioGroup`], as its option builder makes it: a radio with its caption.
/// Its group gives it its behavior ([`skene_foundation::behavior::Radio`]) as it builds it.
///
/// Its parts: its face ([`Radio::default_face`]) holding its mark ([`Radio::default_mark`]),
/// then its caption, 8 pixels after. Every member of a control applies to it, so one option
/// can be disabled.
#[must_use]
pub struct Radio {
  pub(super) members: Members,
  pub(super) face: Container,
  mark: AnyView,
}

impl Radio {
  /// A radio labelled `label`.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self {
      members: Members::default(),
      face: Self::default_face(),
      mark: Self::default_mark().into_any(),
    }
    .set_item_alignment(AxisAlignment::Center)
    .set_spacing(8)
    .set_label(label)
  }

  /// The default face: a circle 18 pixels across, a border of 1.5 in
  /// [`SystemColors::BORDER`] around [`SystemColors::FIELD`] and its hover and pressed
  /// variants, the accent while chosen, its mark centered and inked in
  /// [`SystemColors::ON_ACCENT`]; a ring of 2 in the accent while focus shows.
  pub fn default_face() -> Container {
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
  pub fn default_mark() -> Painted {
    Painted::new(marks::dot)
      .set_width(Dimension::Fraction(1.))
      .set_height(Dimension::Fraction(1.))
  }

  /// The face, in place of [`Radio::default_face`]; the mark is placed in it, so a child it
  /// holds is replaced.
  pub fn set_face(mut self, face: Container) -> Self {
    self.face = face;

    self
  }

  /// The mark, in place of [`Radio::default_mark`]: any view, drawn under the radio's states
  /// (`State::Checked` while chosen).
  pub fn set_mark(mut self, mark: impl View) -> Self {
    self.mark = mark.into_any();

    self
  }

  control_members!(members);

  /// The control this radio is, over `behavior`.
  fn into_control(self, behavior: behavior::Radio) -> Control<behavior::Radio> {
    let face = self
      .face
      .set_child(AsPart::new(Part::Mark).set_child(self.mark));

    Control::with_members(behavior, self.members).add_part(Part::Face, face)
  }
}

/// What makes a group's radio from its index and its option's text.
type OptionBuilder = Rc<dyn Fn(usize, &str) -> Radio>;

/// One choice of a few, all shown, with its caption: Skene's radio group, drawn on every
/// platform. It is a [`Control`] over a [`skene_foundation::behavior::RadioGroup`], its
/// caption above a column of radios 6 pixels apart, each from its option builder
/// ([`RadioGroup::set_option_builder`]).
///
/// The group is one tab stop, on its chosen radio (its first enabled where none is); the
/// arrows move to the next or the previous enabled radio, wrapping, and choose it; `Space`
/// and a click choose.
///
/// It is controlled: it shows what [`RadioGroup::derive_chosen`] returns, asks
/// [`RadioGroup::handle_choose`] for each choice, and shows the truth.
#[must_use]
pub struct RadioGroup {
  pub(super) control: Control<Group>,
  options: Rc<dyn Fn() -> Vec<String>>,
  option_spacing: Value<Spacing>,
  builder: OptionBuilder,
}

impl RadioGroup {
  /// A radio group labelled `label`, with no options.
  pub fn new(label: impl Into<String>) -> Self {
    let label: String = label.into();

    Self {
      control: Control::new(Group::new())
        .set_axis(Axis::Vertical)
        .set_spacing(6)
        .set_label(label),
      options: Rc::new(Vec::new),
      option_spacing: Value::from(6),
      builder: Rc::new(|_, name| Radio::new(name)),
    }
  }

  /// The options' texts.
  pub fn set_options<S: Into<String>>(self, options: impl IntoIterator<Item = S>) -> Self {
    let options: Vec<String> = options.into_iter().map(Into::into).collect();

    self.derive_options(move || options.clone())
  }

  /// The options' texts, kept current: a radio whose text changes is built again.
  pub fn derive_options<S: Into<String>, I: IntoIterator<Item = S>>(
    mut self,
    options: impl Fn() -> I + 'static,
  ) -> Self {
    self.options = Rc::new(move || options().into_iter().map(Into::into).collect());

    self
  }

  /// The chosen option's index, or none.
  pub fn set_chosen(mut self, chosen: Option<usize>) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.set_chosen(chosen));

    self
  }

  /// The chosen option's index, or none, kept current.
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<usize> + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.derive_chosen(chosen));

    self
  }

  /// Runs `choose` with the option the reader chose.
  pub fn handle_choose(mut self, choose: impl Fn(Option<usize>) + 'static) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.handle_choose(choose));

    self
  }

  /// Shows `chosen` and writes each choice into it.
  pub fn bind_chosen(mut self, chosen: Signal<Option<usize>>) -> Self {
    self.control = self
      .control
      .map_behavior(|behavior| behavior.bind_chosen(chosen));

    self
  }

  /// The space between the options, in pixels; 6 where unset.
  pub fn set_option_spacing(mut self, spacing: impl Into<Value<Spacing>>) -> Self {
    self.option_spacing = spacing.into();

    self
  }

  /// The space between the options, in pixels, kept current; 6 where unset.
  pub fn derive_option_spacing<V: Into<Value<Spacing>>>(
    self,
    spacing: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_option_spacing(Value::derived(spacing))
  }

  /// Makes each option's radio with `builder`, from its index and its text: a styled
  /// [`Radio`]; `Radio::new(text)` where unset.
  pub fn set_option_builder(mut self, builder: impl Fn(usize, &str) -> Radio + 'static) -> Self {
    self.builder = Rc::new(builder);

    self
  }

  control_members!(control.members);
}

impl View for RadioGroup {
  fn build(self, cx: &mut Cx) -> Node {
    let Self {
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
