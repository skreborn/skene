//! A ready-made radio group and its members, in the Simple design language.

use std::marker::PhantomData;

use skene_foundation::{
  Alignment,
  AnyKind,
  AnyView,
  BasicRadio,
  BasicRadioGroup,
  Color,
  Container,
  ContainerBase,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  HasChild,
  Inset,
  NoChild,
  NodeId,
  Part,
  Platform,
  Radio,
  RadioGroup,
  RadioGroupContext,
  RunBase,
  Scalar,
  State,
  Text,
  View,
  When,
  Wraps,
};
use skene_reactive::{Get, Memo, Signal, use_context};

use super::{
  SimpleGlyphs,
  SimplePalette,
  shape::{SimpleMarks, SimpleUnit, click_cursor, define_defaults, shape},
};

/// A ready-made group of radio buttons: the members inside it are one tab stop, the
/// arrow keys move between them, and exactly one is chosen.
///
/// Members are [`SimpleRadio`]s at any depth inside the child, so one may be wrapped
/// or interleaved with anything. The chosen value flows both ways. Holds [`NoChild`]
/// until [`set_child`](Self::set_child) supplies the view holding the members.
#[doc(hidden)]
#[derive(Debug)]
#[must_use]
pub struct SimpleRadioGroup<P: Platform, C = NoChild> {
  behavior: RadioGroup,
  label: Option<String>,
  child: C,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> SimpleRadioGroup<P> {
  /// A group with nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      behavior: RadioGroup::new(),
      label: None,
      child: NoChild,
      platform: PhantomData,
    }
  }
}

impl<P: Platform> Default for SimpleRadioGroup<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> SimpleRadioGroup<P> {
  /// Sets the chosen member's value.
  pub fn set_chosen(mut self, chosen: impl Into<String>) -> Self {
    self.behavior = self.behavior.set_chosen(chosen);

    self
  }

  /// Derives the chosen member's value from the signals `chosen` reads.
  pub fn derive_chosen(mut self, chosen: impl Fn() -> Option<String> + 'static) -> Self {
    self.behavior = self.behavior.derive_chosen(chosen);

    self
  }

  /// Runs `action` with the value a choice asks for.
  pub fn handle_choose(mut self, action: impl Fn(String) + 'static) -> Self {
    self.behavior = self.behavior.handle_choose(action);

    self
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(mut self, signal: Signal<Option<String>>) -> Self {
    self.behavior = self.behavior.bind_chosen(signal);

    self
  }

  /// Sets the group's accessible name.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.label = Some(label.into());

    self
  }

  /// Sets the view holding the members.
  pub fn set_child(self, child: impl View<P>) -> SimpleRadioGroup<P, AnyView<P>> {
    SimpleRadioGroup {
      behavior: self.behavior,
      label: self.label,
      child: child.into_any(),
      platform: PhantomData,
    }
  }
}

impl<P: Platform, C> AnyKind for SimpleRadioGroup<P, C> {}

impl<P: Platform> View<P> for SimpleRadioGroup<P, AnyView<P>> {
  fn build(self) -> NodeId {
    let mut group = BasicRadioGroup::new(self.behavior);

    if let Some(label) = self.label {
      group = group.set_label(label);
    }

    group.set_child(self.child).build()
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
pub struct SimpleRadio<P: Platform> {
  member: Radio,
  label: Option<String>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> SimpleRadio<P> {
  /// The member standing for `value`, the string the group reports when it is chosen.
  pub fn new(value: impl Into<String>) -> Self {
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

impl<P: Platform> AnyKind for SimpleRadio<P> {}

impl<P: Platform> View<P> for SimpleRadio<P>
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
        let value = self.member.value().to_owned();

        Text::<P>::derive(move || {
          let checked = group
            .as_ref()
            .is_some_and(|group| group.chosen().as_deref() == Some(value.as_str()));

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
fn drawn_mark<P: Platform>() -> Container<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  let shape = shape::<P>();
  let dot = Container::<P>::new()
    .set_width(Dimension::Length(shape.dot_size))
    .set_height(Dimension::Length(shape.dot_size))
    .set_corner_radius(shape.dot_size.to_f32() / 2.)
    .set_color(
      Color::TRANSPARENT.when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_SURFACE),
    );

  Container::<P>::new()
    .set_width(Dimension::Length(shape.mark_size))
    .set_height(Dimension::Length(shape.mark_size))
    .set_color(shape.mark_surface)
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

/// What a platform's `SimpleRadioGroup` carries.
pub trait SimpleRadioGroupBase<P: Platform>: Wraps<Inner = SimpleRadioGroup<P>> {
  /// A group with nothing chosen.
  #[must_use]
  fn new() -> Self {
    Self::wrap(SimpleRadioGroup::new())
  }

  /// Sets the chosen member's value.
  #[must_use]
  fn set_chosen(self, chosen: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_chosen(chosen))
  }

  /// Derives the chosen member's value from the signals `chosen` reads.
  #[must_use]
  fn derive_chosen(self, chosen: impl Fn() -> Option<String> + 'static) -> Self {
    Self::wrap(self.into_inner().derive_chosen(chosen))
  }

  /// Runs `action` with the value a choice asks for.
  #[must_use]
  fn handle_choose(self, action: impl Fn(String) + 'static) -> Self {
    Self::wrap(self.into_inner().handle_choose(action))
  }

  /// Binds the chosen value to `signal` in both directions.
  #[must_use]
  fn bind_chosen(self, signal: Signal<Option<String>>) -> Self {
    Self::wrap(self.into_inner().bind_chosen(signal))
  }

  /// Sets the group's accessible name.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
  }
}

/// What a platform's `SimpleRadio` carries.
pub trait SimpleRadioBase<P: Platform>: Wraps<Inner = SimpleRadio<P>> {
  /// The member standing for `value`, the string the group reports when it is chosen.
  #[must_use]
  fn new(value: impl Into<String>) -> Self {
    Self::wrap(SimpleRadio::new(value))
  }

  /// Sets the caption beside the ring, which is also the accessible name.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
  }
}
