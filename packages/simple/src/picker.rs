//! A ready-made dropdown choice in the Simple design language.

use std::{marker::PhantomData, rc::Rc};

use skene_foundation::{
  AnyChoice,
  AnyChosen,
  AnyKind,
  AnyView,
  Arrangement,
  BasicListboxOption,
  BasicPickerCore,
  Choice,
  Color,
  Column,
  Container,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  FlexBase,
  HasChild,
  HasChildren,
  Inset,
  ItemAlignment,
  Label,
  ListboxOption,
  MouseCursor,
  NodeId,
  Part,
  Realization,
  Row,
  RunBase,
  Spacing,
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
  shape::{SimpleMarks, SimpleUnit, as_part, click_cursor, define_defaults, hairline, shape},
};

/// The glyph after the chosen label.
const INDICATOR: &str = "▾";
/// The glyph before the chosen row.
const CHECK: &str = "✓";

/// A ready-made dropdown choice.
///
/// Field chrome whose hairline becomes the ring while focused, an indicator after the
/// chosen label, and a list on the card surface whose rows highlight while focused and
/// fill in the primary look while chosen.
///
/// The indicator is the face's [`Part::Face`], the list surface the [`Part::List`] and
/// each row's check the [`Part::Item`], so all three follow their control's state on
/// every facade. The chosen value, a value of the app's own type `T`, flows both ways,
/// exactly as on a `BasicPicker`; for custom chrome, compose the raw control directly.
#[doc(hidden)]
#[derive(Debug)]
#[must_use]
pub struct SimplePicker<P: Realization, T: Choice> {
  core: Core<P>,
  value: PhantomData<fn(T) -> T>,
}

/// A [`SimplePicker`] with its values' type erased: what every one builds.
#[derive(Debug)]
struct Core<P: Realization> {
  options: Vec<(AnyChoice, String)>,
  picker: BasicPickerCore<P>,
}

impl<P: Realization, T: Choice> SimplePicker<P, T> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      core: Core {
        options: Vec::new(),
        picker: BasicPickerCore::new(),
      },
      value: PhantomData,
    }
  }

  /// This picker, its picker replaced by what `map` makes of it.
  #[inline(always)]
  fn map_picker(mut self, map: impl FnOnce(BasicPickerCore<P>) -> BasicPickerCore<P>) -> Self {
    self.core.picker = map(self.core.picker);

    self
  }

  /// Sets the options, in order, each a `(value, label)` pair.
  pub fn set_options<L: Into<String>>(mut self, options: impl IntoIterator<Item = (T, L)>) -> Self {
    self.core.options = options
      .into_iter()
      .map(|(value, label)| (AnyChoice::new(value), label.into()))
      .collect();

    self
  }

  /// Sets the chosen value.
  pub fn set_chosen(self, value: T) -> Self {
    self.map_picker(|picker| picker.set_chosen(Some(value)))
  }

  /// Derives the chosen value from the signals `value` reads.
  pub fn derive_chosen(self, value: impl Fn() -> T + 'static) -> Self {
    self.map_picker(|picker| picker.derive_chosen(move || Some(value())))
  }

  /// Runs `action` with the value a choice asks for.
  pub fn handle_choose(self, action: impl Fn(T) + 'static) -> Self {
    self.map_picker(|picker| {
      picker.handle_choose(move |chosen: Option<T>| {
        if let Some(chosen) = chosen {
          action(chosen);
        }
      })
    })
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_chosen(self, signal: Signal<T>) -> Self {
    self
      .derive_chosen(move || signal.get())
      .handle_choose(move |value| signal.set(value))
  }

  /// Sets the accessible name.
  pub fn set_label(self, label: impl Into<String>) -> Self {
    self.map_picker(|picker| picker.set_label(label.into()))
  }

  /// Sets whether the picker is present but not operable.
  #[inline(always)]
  pub fn set_disabled(self, disabled: bool) -> Self {
    self.map_picker(|picker| picker.set_disabled(disabled))
  }

  /// Derives whether the picker is disabled from the signals `disabled` reads.
  pub fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.map_picker(|picker| picker.derive_disabled(disabled))
  }
}

impl<P: Realization, T: Choice> Default for SimplePicker<P, T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization, T: Choice> AnyKind for SimplePicker<P, T> {}

impl<P: Realization, T: Choice> View<P> for SimplePicker<P, T>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  #[inline(always)]
  fn build(self) -> NodeId {
    self.core.build()
  }
}

impl<P: Realization> Core<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  /// Builds the picker, its options' rows and its face over the picker's core.
  fn build(self) -> NodeId {
    define_defaults::<P>();

    let shape = shape::<P>();
    let Self { options, picker } = self;
    let labels = Rc::new(options);
    let rows: Vec<AnyView<P>> = labels
      .iter()
      .map(|(value, label)| row::<P>(value, label))
      .collect();
    let surface = Container::<P>::new()
      .set_padding(shape.list_inset)
      .set_background(SimplePalette::<P::Scalar>::CARD)
      .set_border_width(hairline())
      .set_border_color(SimplePalette::<P::Scalar>::BORDER)
      .set_corner_radius(shape.control_radius)
      .set_child(Column::<P>::new().set_children(rows));

    picker
      .set_width(Dimension::Fraction(1.))
      .set_padding(shape.control_inset)
      .set_background(SimplePalette::<P::Scalar>::CARD.when(
        State::Disabled,
        SimplePalette::<P::Scalar>::DISABLED_SURFACE,
      ))
      .set_border_width(Inset::all(shape.field_ring))
      .set_border_color(
        SimplePalette::<P::Scalar>::BORDER
          .when(State::FocusVisible, SimplePalette::<P::Scalar>::RING)
          .when(State::Open, SimplePalette::<P::Scalar>::RING),
      )
      .set_corner_radius(shape.control_radius)
      .set_cursor(click_cursor())
      .set_options([AnyView::<P>::new(move || as_part::<P>(Part::List, surface))])
      .set_face(move |chosen: Memo<AnyChosen>| face::<P>(&labels, chosen))
      .build()
  }
}

/// The face: the chosen option's label, and the indicator as the [`Part::Face`].
fn face<P: Realization>(labels: &Rc<Vec<(AnyChoice, String)>>, chosen: Memo<AnyChosen>) -> Row<P>
where
  P::Scalar: SimpleUnit,
{
  let labels = Rc::clone(labels);
  let shown = move || {
    let chosen = chosen.get();

    labels
      .iter()
      .find(|(value, _)| chosen.contains(value))
      .map(|(_, label)| label.clone())
      .unwrap_or_default()
  };
  let indicator = match P::Scalar::MARKS {
    SimpleMarks::Drawn => Text::<P>::new(INDICATOR),
    SimpleMarks::Glyph => {
      let glyphs = use_context::<Memo<SimpleGlyphs>>();

      Text::<P>::derive(move || {
        glyphs
          .map_or_else(SimpleGlyphs::default, |glyphs| glyphs.get())
          .indicator()
      })
    }
  }
  .set_color(
    SimplePalette::<P::Scalar>::MUTED_FOREGROUND
      .when(State::Open, SimplePalette::<P::Scalar>::FOREGROUND),
  );

  Row::<P>::new()
    .set_spacing(Spacing::all(shape::<P>().indicator_spacing))
    .set_arrangement(Arrangement::SpaceBetween)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::<P>::derive(shown).set_color(
        SimplePalette::<P::Scalar>::FOREGROUND
          .when(State::Disabled, SimplePalette::<P::Scalar>::DISABLED_INK),
      ),
      as_part::<P>(Part::Face, indicator),
    ))
}

/// One row of the list: highlighted while focused, filled in the primary look while
/// chosen, its check the [`Part::Item`] and its caption the label.
fn row<P: Realization>(value: &AnyChoice, label: &str) -> AnyView<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  let check = Container::<P>::new()
    .set_hidden(true.when(State::Checked, false))
    .set_child(Text::<P>::new(CHECK).set_color(SimplePalette::<P::Scalar>::PRIMARY_INK));
  let caption = Text::<P>::new(label).set_color(
    SimplePalette::<P::Scalar>::FOREGROUND
      .when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_INK),
  );

  BasicListboxOption::<P>::new(ListboxOption::new(value.clone()))
    .set_padding(shape::<P>().row_inset)
    .set_background(
      Color::TRANSPARENT
        .when(
          State::Focused,
          SimplePalette::<P::Scalar>::SECONDARY_SURFACE,
        )
        .when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_SURFACE),
    )
    .set_corner_radius(shape::<P>().mark_radius)
    .set_cursor(MouseCursor::Click)
    .set_part(Part::Item, check)
    .set_label(Label::new(caption).set_name(label))
    .into_any()
}

/// What a platform's `SimplePicker` of `T`s carries.
pub trait SimplePickerBase<P: Realization, T: Choice>: Wraps<Inner = SimplePicker<P, T>> {
  /// A picker with no options and nothing chosen.
  #[must_use]
  fn new() -> Self {
    Self::wrap(SimplePicker::new())
  }

  /// Sets the options, in order, each a `(value, label)` pair.
  #[must_use]
  fn set_options<L: Into<String>>(self, options: impl IntoIterator<Item = (T, L)>) -> Self {
    Self::wrap(self.into_inner().set_options(options))
  }

  /// Sets the chosen value.
  #[must_use]
  fn set_chosen(self, value: T) -> Self {
    Self::wrap(self.into_inner().set_chosen(value))
  }

  /// Derives the chosen value from the signals `value` reads.
  #[must_use]
  fn derive_chosen(self, value: impl Fn() -> T + 'static) -> Self {
    Self::wrap(self.into_inner().derive_chosen(value))
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

  /// Sets the accessible name.
  #[must_use]
  fn set_label(self, label: impl Into<String>) -> Self {
    Self::wrap(self.into_inner().set_label(label))
  }

  /// Sets whether the picker is present but not operable.
  #[must_use]
  fn set_disabled(self, disabled: bool) -> Self {
    Self::wrap(self.into_inner().set_disabled(disabled))
  }

  /// Derives whether the picker is disabled from the signals `disabled` reads.
  #[must_use]
  fn derive_disabled(self, disabled: impl Fn() -> bool + 'static) -> Self {
    Self::wrap(self.into_inner().derive_disabled(disabled))
  }
}
