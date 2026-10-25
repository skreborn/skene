//! A radio group and its radios, drawn with the platform's colors.

use std::marker::PhantomData;

use skene_foundation::{
  AnyKind,
  Axis,
  BasicRadioGroupBase,
  Children,
  Choice,
  Condition,
  ControlBase,
  DeclaredBoxStyle,
  DecorationBase,
  FlexBase,
  FlexChild,
  HasBoxStyle,
  HasChildren,
  HasControl,
  HasLayoutBox,
  ItemAlignment,
  LayoutBox,
  MotionBase,
  MouseCursor,
  NodeId,
  Part,
  PlainChild,
  RadioGroup as Group,
  RadioGroupContext,
  RunBase,
  Seal,
  State,
  SystemColors,
  Text,
  View,
  When,
  Wraps,
};
use skene_reactive::use_context;

use crate::Active;

/// The platform's colors in cells.
type Colors = SystemColors<i32>;

/// A radio group: Skene's, drawn in cells, a [`Control`](crate::Control) over a
/// [`RadioGroup`](crate::behavior::RadioGroup) whose radios are tagged with values of the
/// app's own type `T`, one of them chosen.
///
/// Its radios are [`Radio`]s at any depth inside it, one tab stop; the arrows move to the
/// next or the previous enabled radio, wrapping, and choose it; `Space` and a click
/// choose. Controlled: the chosen radio is the one tagged with what
/// [`derive_chosen`](BasicRadioGroupBase::derive_chosen) reads, and a choice only calls
/// [`handle_choose`](BasicRadioGroupBase::handle_choose) with the value asked for;
/// [`bind_chosen`](BasicRadioGroupBase::bind_chosen) wires both to one signal. A group
/// that starts with nothing chosen holds an `Option`, its radios tagged `Some(..)`.
///
/// Its defaults: its caption ([`set_label`](ControlBase::set_label)) above its radios, at
/// the start, and its children ([`add_child`](HasChildren::add_child) and its kin) in a
/// column on rows of their own, a [`Flexible`](crate::Flexible) among them taking its
/// flexibility there. A body [`set_child`](ControlBase::set_child) supplies takes the
/// column's place, the children added to the column with it; every other default is the
/// control's.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// #[derive(Clone, Copy, PartialEq, Eq, Hash)]
/// enum Size {
///   Small,
///   Medium,
/// }
///
/// let size = Signal::new(Size::Medium);
/// let group = RadioGroup::new()
///   .set_label("Size")
///   .bind_chosen(size)
///   .set_children((
///     Radio::new(Size::Small).set_label("Small"),
///     Radio::new(Size::Medium).set_label("Medium"),
///   ));
/// ```
#[derive(Debug)]
#[must_use]
pub struct RadioGroup<T: Choice> {
  control: skene_foundation::Control<Active, Group>,
  radios: skene_foundation::Column<Active>,
  value: PhantomData<fn(T) -> T>,
}

impl<T: Choice> Default for RadioGroup<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T: Choice> RadioGroup<T> {
  /// The spacing between the radios, in cells.
  pub const SPACING: i32 = 0;

  /// A radio group with nothing chosen and no radios.
  pub fn new() -> Self {
    Self {
      control: skene_foundation::Control::new(Group::typed::<Option<T>>())
        .set_axis(Axis::Vertical)
        .set_item_alignment(ItemAlignment::Start),
      radios: skene_foundation::Column::new(),
      value: PhantomData,
    }
  }
}

impl<T: Choice> HasControl<Active, Group> for RadioGroup<T> {
  #[inline(always)]
  fn control(&self) -> &skene_foundation::Control<Active, Group> {
    &self.control
  }

  #[inline(always)]
  fn map_control(
    mut self,
    map: impl FnOnce(
      skene_foundation::Control<Active, Group>,
    ) -> skene_foundation::Control<Active, Group>,
  ) -> Self {
    self.control = map(self.control);

    self
  }
}

impl<T: Choice> HasBoxStyle<Active> for RadioGroup<T> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<Active> {
    self.control.box_style_mut()
  }
}

impl<T: Choice> HasLayoutBox<Active> for RadioGroup<T> {
  type Kind = PlainChild;

  #[inline(always)]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<Active, PlainChild>) -> LayoutBox<Active, PlainChild>,
  ) -> Self {
    self.map_control(|control| control.map_layout_box(map))
  }
}

impl<T: Choice> HasChildren<Active> for RadioGroup<T> {
  type Kind = FlexChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<Active, FlexChild> {
    self.radios.children_mut(seal)
  }
}

impl<T: Choice> ControlBase<Active, Group> for RadioGroup<T> {}

impl<T: Choice> DecorationBase<Active> for RadioGroup<T> {}

impl<T: Choice> MotionBase<Active> for RadioGroup<T> {}

impl<T: Choice> BasicRadioGroupBase<Active, T> for RadioGroup<T> {}

impl<T: Choice> RadioGroup<T> {
  crate::views::terminal_decoration!(box_style_mut);
}

impl<T: Choice> AnyKind for RadioGroup<T> {}

impl<T: Choice> View<Active> for RadioGroup<T> {
  fn build(self) -> NodeId {
    let defaults = skene_foundation::Control::new(Group::typed::<Option<T>>())
      .set_child(self.radios.set_spacing(Self::SPACING));

    self.control.over(defaults).build()
  }
}

/// A radio: Skene's, drawn in cells, a [`Control`](crate::Control) over a
/// [`Radio`](crate::behavior::Radio) tagged with a value, what its [`RadioGroup`] is
/// asked to choose when it is chosen.
///
/// Its mark is its [`Part::Mark`], its caption trailing it
/// ([`set_label`](ControlBase::set_label)). Its default mark, which
/// [`set_part(Part::Mark, ..)`](ControlBase::set_part) replaces: the glyph `○`, and `◉`
/// while chosen, in [`SystemColors::INK`], the accent while focus shows and
/// [`SystemColors::MUTED_INK`] while disabled. Its default cursor is the click cursor,
/// the default one while disabled; every other default is the control's. Outside a group
/// it draws, is never chosen and takes focus like any control.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// let small = Radio::new(1).set_label("Small");
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct Radio(skene_foundation::Control<Active, skene_foundation::Radio>);

impl Radio {
  /// A radio tagged with `value`, with no caption.
  #[inline(always)]
  pub fn new(value: impl Choice) -> Self {
    Self(skene_foundation::Control::new(
      skene_foundation::Radio::new(value),
    ))
  }
}

impl Radio {
  crate::views::terminal_decoration!(box_style_mut);
}

impl Radio {
  /// The default mark: its glyph as whether the group around it has chosen `value`,
  /// inked in the accent while focus shows.
  fn mark(value: skene_foundation::AnyChoice) -> Text<Active> {
    let group = use_context::<RadioGroupContext>();

    Text::<Active>::derive(move || {
      let chosen = group.as_ref().is_some_and(|group| group.is_chosen(&value));

      if chosen { "\u{25c9}" } else { "\u{25cb}" }.to_owned()
    })
    .set_color(
      Colors::INK
        .when(Condition::from(State::FocusVisible), Colors::ACCENT)
        .when(State::Disabled.into(), Colors::MUTED_INK),
    )
  }
}

impl ControlBase<Active, skene_foundation::Radio> for Radio {}

impl DecorationBase<Active> for Radio {}

impl MotionBase<Active> for Radio {}

impl AnyKind for Radio {}

impl View<Active> for Radio {
  fn build(self) -> NodeId {
    let value = self.behavior().value().clone();
    let defaults = skene_foundation::Control::new(skene_foundation::Radio::new(value.clone()))
      .set_cursor(MouseCursor::Click.when(State::Disabled, MouseCursor::Default))
      .set_part(Part::Mark, Self::mark(value));

    self.0.over(defaults).build()
  }
}
