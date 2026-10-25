//! A listbox and its options, drawn with the platform's colors.

use std::marker::PhantomData;

use skene_foundation::{
  AnyKind,
  Axis,
  BasicListboxBase,
  Children,
  Chosen,
  Color,
  Condition,
  ControlBase,
  DeclaredBoxStyle,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  FlexBase,
  FlexChild,
  HasBoxStyle,
  HasChild,
  HasChildren,
  HasControl,
  HasLayoutBox,
  Inset,
  ItemAlignment,
  LayoutBox,
  Listbox as Collection,
  MotionBase,
  MouseCursor,
  NodeId,
  Part,
  PlainChild,
  RunBase,
  Seal,
  State,
  SystemColors,
  Text,
  Value,
  View,
  When,
  Wraps,
};

use crate::Active;

/// The platform's colors in logical pixels.
type Colors = SystemColors<f32>;

/// A listbox: Skene's, drawn on every engine, a [`Control`](crate::Control) over a
/// [`Listbox`](crate::behavior::Listbox) whose options are tagged with values of the
/// app's own type, what it has chosen a [`Chosen`] of them, `Option<T>` for one choice.
///
/// Its options are [`ListboxOption`]s at any depth inside it, one tab stop; the vertical
/// arrows, `Home`, `End` and typing move focus, and `Enter`, `Space` or a click choose.
/// Controlled: what is chosen is what
/// [`derive_chosen`](BasicListboxBase::derive_chosen) reads, and a choice only calls
/// [`handle_choose`](BasicListboxBase::handle_choose) with what it asks for;
/// [`bind_chosen`](BasicListboxBase::bind_chosen) wires both to one signal.
///
/// Its defaults, each a value its setters replace: its caption
/// ([`set_label`](ControlBase::set_label)) above its options, at the start, and its
/// children ([`add_child`](HasChildren::add_child) and its kin) in a column 2 pixels
/// apart, in a box ([`set_list`](Self::set_list)) padded 4 inside a border of 1 in
/// [`SystemColors::BORDER`], its corners rounded by 6, filled with
/// [`SystemColors::FIELD`]. A body [`set_child`](ControlBase::set_child)
/// supplies takes the box's place, and the children given to the column are dropped
/// with it; every other default is the control's.
///
/// ```
/// use skene_web::prelude::*;
///
/// let fruit = Signal::new(Some(2));
/// let listbox = Listbox::new()
///   .set_label("Fruit")
///   .bind_chosen(fruit)
///   .set_children((
///     ListboxOption::new(1).set_label("Apple"),
///     ListboxOption::new(2).set_label("Banana"),
///   ));
/// ```
#[derive(Debug)]
#[must_use]
pub struct Listbox<C: Chosen> {
  control: skene_foundation::Control<Active, Collection>,
  list: Option<skene_foundation::Container<Active>>,
  options: skene_foundation::Column<Active>,
  chosen: PhantomData<fn(C) -> C>,
}

impl<C: Chosen> Default for Listbox<C> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<C: Chosen> Listbox<C> {
  /// The spacing between the options, in logical pixels.
  pub const SPACING: f32 = 2.;

  /// A listbox with nothing chosen and no options.
  pub fn new() -> Self {
    Self {
      control: skene_foundation::Control::new(Collection::typed::<C>())
        .set_axis(Axis::Vertical)
        .set_item_alignment(ItemAlignment::Start),
      list: None,
      options: skene_foundation::Column::new(),
      chosen: PhantomData,
    }
  }

  /// Sets the box the options sit in, in place of the default one: its decoration and
  /// its sizing; the options are placed in it, so a child it holds is replaced.
  pub fn set_list(mut self, list: impl Wraps<Inner = skene_foundation::Container<Active>>) -> Self {
    self.list = Some(list.into_inner());

    self
  }
}

impl<C: Chosen> HasControl<Active, Collection> for Listbox<C> {
  #[inline(always)]
  fn control(&self) -> &skene_foundation::Control<Active, Collection> {
    &self.control
  }

  #[inline(always)]
  fn map_control(
    mut self,
    map: impl FnOnce(
      skene_foundation::Control<Active, Collection>,
    ) -> skene_foundation::Control<Active, Collection>,
  ) -> Self {
    self.control = map(self.control);

    self
  }
}

impl<C: Chosen> HasBoxStyle<Active> for Listbox<C> {
  #[inline(always)]
  fn box_style_mut(&mut self) -> &mut DeclaredBoxStyle<Active> {
    self.control.box_style_mut()
  }
}

impl<C: Chosen> HasLayoutBox<Active> for Listbox<C> {
  type Kind = PlainChild;

  #[inline(always)]
  fn map_layout_box(
    self,
    map: impl FnOnce(LayoutBox<Active, PlainChild>) -> LayoutBox<Active, PlainChild>,
  ) -> Self {
    self.map_control(|control| control.map_layout_box(map))
  }
}

impl<C: Chosen> HasChildren<Active> for Listbox<C> {
  type Kind = FlexChild;

  #[inline(always)]
  fn children_mut(&mut self, seal: Seal) -> &mut Children<Active, FlexChild> {
    self.options.children_mut(seal)
  }
}

impl<C: Chosen> ControlBase<Active, Collection> for Listbox<C> {}

impl<C: Chosen> DecorationBase<Active> for Listbox<C> {}

impl<C: Chosen> MotionBase<Active> for Listbox<C> {}

impl<C: Chosen> BasicListboxBase<Active, C> for Listbox<C> {}

impl<C: Chosen> AnyKind for Listbox<C> {}

impl<C: Chosen> View<Active> for Listbox<C> {
  fn build(self) -> NodeId {
    let list = self.list.unwrap_or_else(|| {
      skene_foundation::Container::new()
        .set_padding(Inset::all(4))
        .set_border_width(Inset::all(1))
        .set_border_color(Colors::BORDER)
        .set_corner_radius(6)
        .set_background(Colors::FIELD)
    });
    let defaults = skene_foundation::Control::new(Collection::new())
      .set_child(list.set_child(self.options.set_spacing(Self::SPACING)));

    self.control.over(defaults).build()
  }
}

/// An option of a [`Listbox`] or a [`Picker`](crate::Picker): Skene's, drawn on every
/// engine, a [`Control`](crate::Control) over a
/// [`ListboxOption`](crate::behavior::ListboxOption) tagged with a value, what its list
/// is asked to choose when it is chosen.
///
/// Its mark is its [`Part::Mark`], its caption trailing it
/// ([`set_label`](ControlBase::set_label)), which also names it for type-ahead. Its
/// defaults, each a value its setters replace: as wide as its list, at least 32 pixels
/// tall, padded 4 above and below and 8 at the sides inside a border of 2, invisible but
/// while focus shows, a ring in the accent, its corners rounded by 4; filled with
/// [`SystemColors::CONTROL_HOVER`] while hovered and [`SystemColors::CONTROL`] while
/// focused; the click cursor, the default one while disabled. Its default mark, which
/// [`set_part(Part::Mark, ..)`](ControlBase::set_part) replaces: a check in
/// [`SystemColors::INK`], [`SystemColors::MUTED_INK`] while disabled, showing while
/// chosen and keeping its place otherwise.
///
/// ```
/// use skene_web::prelude::*;
///
/// let apple = ListboxOption::new(1).set_label("Apple");
/// ```
#[derive(Debug, Wraps)]
#[must_use]
pub struct ListboxOption(skene_foundation::Control<Active, skene_foundation::ListboxOption>);

impl ListboxOption {
  /// An option tagged with `value`, with no caption.
  #[inline(always)]
  pub fn new(value: impl skene_foundation::Choice) -> Self {
    Self(skene_foundation::Control::new(
      skene_foundation::ListboxOption::new(value),
    ))
  }
}

impl ListboxOption {
  /// The default mark: a check showing while chosen.
  fn mark() -> skene_foundation::Container<Active> {
    skene_foundation::Container::<Active>::new()
      .set_width(Dimension::Length(16.))
      .set_hidden(true.when(State::Checked, false))
      .set_child(
        Text::<Active>::new("\u{2713}")
          .set_color(Colors::INK.when(State::Disabled, Colors::MUTED_INK)),
      )
  }
}

impl ControlBase<Active, skene_foundation::ListboxOption> for ListboxOption {}

impl DecorationBase<Active> for ListboxOption {}

impl MotionBase<Active> for ListboxOption {}

impl AnyKind for ListboxOption {}

impl View<Active> for ListboxOption {
  fn build(self) -> NodeId {
    let value = self.behavior().value().clone();
    let defaults = skene_foundation::Control::new(skene_foundation::ListboxOption::new(value))
      .set_width(Dimension::Fraction(1.))
      .set_min_height(Dimension::Length(32.))
      .set_padding(Inset::symmetric(8, 4))
      .set_border_width(Inset::all(2))
      .set_border_color(
        Color::TRANSPARENT.when(Condition::from(State::FocusVisible), Colors::ACCENT),
      )
      .set_corner_radius(4)
      .set_background(
        Value::<Color, Condition<f32>>::from(Color::TRANSPARENT)
          .when(State::Hovered.into(), Colors::CONTROL_HOVER)
          .when(State::Focused.into(), Colors::CONTROL),
      )
      .set_cursor(MouseCursor::Click.when(State::Disabled, MouseCursor::Default))
      .set_part(Part::Mark, Self::mark());

    self.0.over(defaults).build()
  }
}
