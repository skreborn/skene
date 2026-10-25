//! A ready-made radio group and its members, in the Simple design language.

use std::marker::PhantomData;

use skene_foundation::{
  Alignment,
  AnyKind,
  AnyView,
  BasicRadio,
  Choice,
  Color,
  Container,
  ContainerBase,
  Control,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  HasChild,
  Inset,
  NodeId,
  Part,
  Radio,
  RadioGroup,
  RadioGroupContext,
  Realization,
  RunBase,
  Scalar,
  State,
  Text,
  View,
  When,
  Wraps,
};
use skene_reactive::{Get, Memo, Set, Signal, use_context};

use super::{
  SimpleGlyphs,
  SimplePalette,
  shape::{SimpleMarks, SimpleUnit, click_cursor, define_defaults, shape},
};

/// A ready-made group of radio buttons: the members inside it are one tab stop, the
/// arrow keys move between them, and exactly one is chosen.
///
/// Members are [`SimpleRadio`]s at any depth inside the child, tagged with values of the
/// app's own type `T`, so one may be wrapped or interleaved with anything. The chosen
/// value flows both ways; a group that starts with nothing chosen holds an `Option`, its
/// members tagged `Some(..)`. Without the view [`set_child`](Self::set_child) supplies,
/// it builds an empty group.
#[doc(hidden)]
#[derive(Debug)]
#[must_use]
pub struct SimpleRadioGroup<P: Realization, T: Choice> {
  behavior: RadioGroup,
  label: Option<String>,
  child: Option<AnyView<P>>,
  value: PhantomData<fn(T) -> T>,
}

impl<P: Realization, T: Choice> SimpleRadioGroup<P, T> {
  /// A group with nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      behavior: RadioGroup::typed::<Option<T>>(),
      label: None,
      child: None,
      value: PhantomData,
    }
  }
}

impl<P: Realization, T: Choice> Default for SimpleRadioGroup<P, T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization, T: Choice> SimpleRadioGroup<P, T> {
  /// Sets the chosen member's value.
  pub fn set_chosen(mut self, chosen: T) -> Self {
    self.behavior = self.behavior.set_chosen(Some(chosen));

    self
  }

  /// Derives the chosen member's value from the signals `chosen` reads.
  pub fn derive_chosen(mut self, chosen: impl Fn() -> T + 'static) -> Self {
    self.behavior = self.behavior.derive_chosen(move || Some(chosen()));

    self
  }

  /// Runs `action` with the value a choice asks for.
  pub fn handle_choose(mut self, action: impl Fn(T) + 'static) -> Self {
    self.behavior = self.behavior.handle_choose(move |chosen: Option<T>| {
      if let Some(chosen) = chosen {
        action(chosen);
      }
    });

    self
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(self, signal: Signal<T>) -> Self {
    self
      .derive_chosen(move || signal.get())
      .handle_choose(move |chosen| signal.set(chosen))
  }

  /// Sets the group's accessible name.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets the view holding the members.
  pub fn set_child(mut self, child: impl View<P>) -> Self {
    self.child = Some(child.into_any());

    self
  }
}

impl<P: Realization, T: Choice> AnyKind for SimpleRadioGroup<P, T> {}

impl<P: Realization, T: Choice> View<P> for SimpleRadioGroup<P, T> {
  fn build(self) -> NodeId {
    let mut group = Control::<P, _>::new(self.behavior);

    if let Some(label) = self.label {
      group = group.set_label(label);
    }

    if let Some(child) = self.child {
      group = group.set_child(child);
    }

    group.build()
  }
}

/// A ready-made radio button: a ring on the card surface whose dot appears while
/// chosen and which rings while focused, beside its caption.
///
/// The ring is the control's [`Part::Mark`], so it follows the control's state on
/// every facade. Outside a [`SimpleRadioGroup`] it draws, is never chosen and takes
/// focus like any control. For a radio drawn some other way, compose [`BasicRadio`]
/// directly.
#[doc(hidden)]
#[derive(Debug)]
#[must_use]
pub struct SimpleRadio<P: Realization> {
  member: Radio,
  label: Option<String>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> SimpleRadio<P> {
  /// The member tagged with `value`, what its group is asked to choose when it is
  /// chosen.
  pub fn new(value: impl Choice) -> Self {
    Self {
      member: Radio::new(value),
      label: None,
      platform: PhantomData,
    }
  }

  /// Sets the caption beside the ring, which is also the accessible name.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }
}

impl<P: Realization> AnyKind for SimpleRadio<P> {}

impl<P: Realization> View<P> for SimpleRadio<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<P>();

    let mark: AnyView<P> = match P::Scalar::MARKS {
      SimpleMarks::Drawn => drawn_mark::<P>().into_any(),
      SimpleMarks::Glyph => {
        let glyphs = use_context::<Memo<SimpleGlyphs>>();
        let group = use_context::<RadioGroupContext>();
        let value = self.member.value().clone();

        Text::<P>::derive(move || {
          let checked = group.as_ref().is_some_and(|group| group.is_chosen(&value));

          glyphs
            .map_or_else(SimpleGlyphs::default, |glyphs| glyphs.get())
            .radio(checked)
        })
        .set_color(
          SimplePalette::<P::Scalar>::FOREGROUND
            .when(State::FocusVisible, SimplePalette::<P::Scalar>::RING),
        )
        .into_any()
      }
    };
    let mut control = BasicRadio::new(self.member)
      .set_cursor(click_cursor())
      .set_part(Part::Mark, mark);

    if let Some(label) = self.label {
      control = control.set_label(label);
    }

    control.build()
  }
}

/// The drawn mark: a ring on the card surface holding a dot that fills with the primary
/// surface while checked, ringing while focused.
fn drawn_mark<P: Realization>() -> Container<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  let shape = shape::<P>();
  let dot = Container::<P>::new()
    .set_width(Dimension::Length(shape.dot_size))
    .set_height(Dimension::Length(shape.dot_size))
    .set_corner_radius(shape.dot_size.to_f32() / 2.)
    .set_background(
      Color::TRANSPARENT.when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_SURFACE),
    );

  Container::<P>::new()
    .set_width(Dimension::Length(shape.mark_size))
    .set_height(Dimension::Length(shape.mark_size))
    .set_background(shape.mark_surface)
    .set_border_width(Inset::all(shape.mark_ring))
    .set_border_color(
      SimplePalette::<P::Scalar>::BORDER
        .when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_SURFACE)
        .when(State::FocusVisible, SimplePalette::<P::Scalar>::RING),
    )
    .set_corner_radius(shape.mark_size.to_f32() / 2.)
    .set_alignment(Alignment::CENTER)
    .set_child(dot)
}

/// What a platform's `SimpleRadioGroup` of `T`s carries.
pub trait SimpleRadioGroupBase<P: Realization, T: Choice>:
  Wraps<Inner = SimpleRadioGroup<P, T>>
{
  /// A group with nothing chosen.
  #[must_use]
  fn new() -> Self {
    Self::wrap(SimpleRadioGroup::new())
  }

  /// Sets the chosen member's value.
  #[must_use]
  fn set_chosen(self, chosen: T) -> Self {
    Self::wrap(self.into_inner().set_chosen(chosen))
  }

  /// Derives the chosen member's value from the signals `chosen` reads.
  #[must_use]
  fn derive_chosen(self, chosen: impl Fn() -> T + 'static) -> Self {
    Self::wrap(self.into_inner().derive_chosen(chosen))
  }

  /// Runs `action` with the value a choice asks for.
  #[must_use]
  fn handle_choose(self, action: impl Fn(T) + 'static) -> Self {
    Self::wrap(self.into_inner().handle_choose(action))
  }

  /// Binds the chosen value to `signal` in both directions.
  #[must_use]
  fn bind_chosen(self, signal: Signal<T>) -> Self {
    Self::wrap(self.into_inner().bind_chosen(signal))
  }

  /// Sets the group's accessible name.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
  }

  /// Sets the view holding the members.
  #[must_use]
  fn set_child(self, child: impl View<P>) -> Self {
    Self::wrap(self.into_inner().set_child(child))
  }
}

/// What a platform's `SimpleRadio` carries.
pub trait SimpleRadioBase<P: Realization>: Wraps<Inner = SimpleRadio<P>> {
  /// The member tagged with `value`, what its group is asked to choose when it is
  /// chosen.
  #[must_use]
  fn new(value: impl Choice) -> Self {
    Self::wrap(SimpleRadio::new(value))
  }

  /// Sets the caption beside the ring, which is also the accessible name.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
  }
}
