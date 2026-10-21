//! A radio group and its radios.

use std::{marker::PhantomData, rc::Rc};

use crate::{
  Alignment, AxisAlignment, Border, Dimension, Spacing, State, SystemColors, Value,
  behavior::{AnyChoice, Behavior, Choice, Controlled, RadioGroup as Group},
  layout::Axis,
  seam::{Realization, control::Part},
  view::{
    AnyKind, AnyView, AsPart, AsPartBase, Column, ColumnBase, Container, ContainerBase,
    ControlBase, ControlMembersBase, Cx, DecorationBase, Label, Node, Painted, PaintedBase,
    RunBase, Thin, View, Wraps,
    controls::{
      Pending,
      choice::{GroupOptions, HasChoice, HasOptions, OptionSegment},
      control_surfaces, focus_ring, marks,
    },
    wraps_itself,
  },
};

/// One option of a [`RadioGroup`], as its option builder makes it: a radio with its caption.
/// Its group gives it its behavior ([`crate::behavior::Radio`]) and its value as it builds it.
///
/// Its parts: its face ([`Radio::default_face`]) holding its mark ([`Radio::default_mark`]),
/// then its caption, 8 pixels after. Every member of a control applies to it, so one option
/// can be disabled.
#[must_use]
pub struct Radio<R: Realization> {
  /// The control it composes, captioned, its defaults set and its behavior given as its group
  /// builds it.
  control: crate::view::Control<Pending, R>,
  /// The face the mark sits in; the default where unset.
  face: Option<Container<R>>,
  /// The mark; the default where unset.
  mark: Option<AnyView<R>>,
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

  /// The control this radio is, over `behavior`, its defaults set.
  fn into_control<B: Behavior>(self, behavior: B) -> crate::view::Control<B, R> {
    let Self {
      control,
      face,
      mark,
    } = self;
    let defaults = crate::view::Control::new(Pending)
      .set_item_alignment(AxisAlignment::Center)
      .set_spacing(8);
    let mark = mark.unwrap_or_else(|| Self::default_mark().into_any());
    let face = face
      .unwrap_or_else(Self::default_face)
      .set_child(AsPart::new(Part::Mark).set_child(mark));

    control
      .over(defaults)
      .with_behavior(behavior)
      .add_part(Part::Face, face)
  }
}

/// What a [`Radio`] carries beside every control's members: its face and its mark.
pub trait RadioBase<R: Realization>: Wraps<Inner = Radio<R>> {
  /// A radio captioned by `label`: a text names it, a view by the texts it shows or through
  /// [`Label::set_name`].
  fn new(label: impl Into<Label<R>>) -> Self {
    Self::wrap(Radio {
      control: crate::view::Control::new(Pending).set_label(label),
      face: None,
      mark: None,
    })
  }

  /// The face, in place of [`Radio::default_face`]; the mark is placed in it, so a child it
  /// holds is replaced.
  fn set_face(mut self, face: impl Wraps<Inner = Container<R>>) -> Self {
    self.inner_mut().face = Some(face.into_inner());

    self
  }

  /// The mark, in place of [`Radio::default_mark`]: any view, drawn under the radio's states
  /// (`State::Checked` while chosen).
  fn set_mark(mut self, mark: impl View<R>) -> Self {
    self.inner_mut().mark = Some(mark.into_any());

    self
  }
}

impl<R: Realization> RadioBase<R> for Radio<R> {}

/// What makes a group's radio from its value and its caption.
type OptionBuilder<R> = Rc<dyn Fn(&AnyChoice, Label<R>) -> Radio<R>>;

/// One choice of a few, all shown, with its caption: Skene's radio group, drawn on every
/// platform. It holds a `T`, each option carrying one. It is a [`crate::view::Control`] over a
/// [`crate::behavior::RadioGroup`], its caption above a column of radios 6 pixels apart, each
/// from its option builder ([`RadioGroupBase::set_option_builder`]).
///
/// The group is one tab stop, on its chosen radio (its first enabled where none is); the
/// arrows move to the next or the previous enabled radio, wrapping, and choose it; `Space`
/// and a click choose. A disabled group disables its radios.
///
/// It is controlled: it shows what [`ChoiceBase::derive_chosen`] returns, asks
/// [`ChoiceBase::handle_choose`] for each choice, and shows the truth. A group that starts with
/// nothing chosen holds an `Option`, its options tagged `Some(..)`.
///
/// It is thin over a [`RadioGroupCore`], which holds its values with their type erased
/// ([`AnyChoice`]).
///
/// [`ChoiceBase::derive_chosen`]: crate::view::ChoiceBase::derive_chosen
/// [`ChoiceBase::handle_choose`]: crate::view::ChoiceBase::handle_choose
#[must_use]
pub struct RadioGroup<T: Choice, R: Realization> {
  /// The group, its values' type erased.
  core: RadioGroupCore<R>,
  /// The type of its values.
  value: PhantomData<fn(T) -> T>,
}

/// A [`RadioGroup`] with its values' type erased: what every radio group builds.
#[must_use]
pub struct RadioGroupCore<R: Realization> {
  /// The control it composes, captioned, its defaults set and its behavior given as it is
  /// built.
  control: crate::view::Control<Pending, R>,
  /// What it has chosen.
  chosen: Controlled<AnyChoice>,
  /// Its options' runs.
  segments: Vec<OptionSegment<R>>,
  /// The space between the options; 6 where unset.
  option_spacing: Option<Value<Spacing>>,
  /// What makes each option's radio; [`Radio::new`] where unset.
  builder: Option<OptionBuilder<R>>,
}

wraps_itself!(RadioGroup<T, R> where T: Choice, R: Realization);

control_surfaces!([T: Choice, R: Realization] RadioGroup<T, R>, R => core.control);

impl<T: Choice, R: Realization> HasChoice<T> for RadioGroup<T, R> {
  fn change_chosen(&mut self, change: impl FnOnce(&mut Controlled<T>)) {
    let mut typed = Controlled::new();

    change(&mut typed);

    self
      .core
      .chosen
      .merge(typed.map(AnyChoice::new, |value: AnyChoice| {
        value.value::<T>().clone()
      }));
  }
}

impl<T: Choice, R: Realization> HasOptions<R> for RadioGroup<T, R> {
  fn segments_mut(&mut self) -> &mut Vec<OptionSegment<R>> {
    &mut self.core.segments
  }
}

impl<R: Realization> RadioGroupCore<R> {
  /// A radio group labelled `label`, with no options.
  pub fn new(label: Label<R>) -> Self {
    Self {
      control: crate::view::Control::new(Pending).set_label(label),
      chosen: Controlled::new(),
      segments: Vec::new(),
      option_spacing: None,
      builder: None,
    }
  }
}

/// What a [`RadioGroup`] carries beside every control's members, its choice and its options:
/// the space between its options and what makes each.
pub trait RadioGroupBase<T: Choice, R: Realization>: Wraps<Inner = RadioGroup<T, R>> {
  /// A radio group labelled `label`, with no options.
  fn new(label: impl Into<Label<R>>) -> Self {
    Self::wrap(RadioGroup {
      core: RadioGroupCore::new(label.into()),
      value: PhantomData,
    })
  }

  /// The space between the options, in pixels; 6 where unset.
  fn set_option_spacing(mut self, spacing: impl Into<Value<Spacing>>) -> Self {
    self.inner_mut().core.option_spacing = Some(spacing.into());

    self
  }

  /// The space between the options, in pixels, kept current; 6 where unset.
  fn derive_option_spacing<V: Into<Value<Spacing>>>(
    self,
    spacing: impl Fn() -> V + 'static,
  ) -> Self {
    self.set_option_spacing(Value::derived(spacing))
  }

  /// Makes each option's radio with `builder`, from its value and its caption: a styled
  /// [`Radio`]; `Radio::new(label)` where unset.
  fn set_option_builder<O: Wraps<Inner = Radio<R>>>(
    mut self,
    builder: impl Fn(&T, Label<R>) -> O + 'static,
  ) -> Self {
    self.inner_mut().core.builder = Some(Rc::new(move |value, label| {
      builder(value.value::<T>(), label).into_inner()
    }));

    self
  }
}

impl<T: Choice, R: Realization> RadioGroupBase<T, R> for RadioGroup<T, R> {}

impl<T: Choice, R: Realization> AnyKind for RadioGroup<T, R> {}

impl<T: Choice, R: Realization> Thin<R> for RadioGroup<T, R> {
  type Core = RadioGroupCore<R>;

  fn into_core(self) -> RadioGroupCore<R> {
    self.core
  }
}

impl<T: Choice, R: Realization> View<R> for RadioGroup<T, R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    self.core.build(cx)
  }
}

impl<R: Realization> AnyKind for RadioGroupCore<R> {}

impl<R: Realization> View<R> for RadioGroupCore<R> {
  fn build(self, cx: &mut Cx<R>) -> Node {
    let Self {
      control,
      chosen,
      segments,
      option_spacing,
      builder,
    } = self;
    let group = Group::new();
    let roster = group.roster();
    let options = GroupOptions::new(segments);
    let defaults = crate::view::Control::new(Pending).set_spacing(6);
    let control = control
      .over(defaults)
      .with_behavior(group)
      .set_axis(Axis::Vertical);
    let option_spacing = option_spacing.unwrap_or_else(|| Value::from(6));
    let builder: OptionBuilder<R> =
      builder.unwrap_or_else(|| Rc::new(|_, label| Radio::new(label)));

    roster.change_chosen(|controlled| *controlled = chosen);
    roster.set_options(Rc::new(options.clone()));

    let radios = Column::new().set_spacing(option_spacing).derive_children(
      {
        let options = options.clone();

        move || options.values()
      },
      Clone::clone,
      move |value: AnyChoice| {
        let member = Group::with_roster(Rc::clone(&roster)).member(value.clone());

        builder(&value, options.caption(&value)).into_control(member)
      },
    );

    control.set_child(radios).build(cx)
  }
}
