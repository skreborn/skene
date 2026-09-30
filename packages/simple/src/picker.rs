//! A ready-made dropdown choice in the Simple design language.

use std::rc::Rc;

use skene_common::{
  BasicListboxOption,
  BasicPicker,
  Column,
  Container,
  CornerRadius,
  Label,
  ListboxOption,
  Row,
  Text,
};
use skene_core::{
  AnyKind,
  AnyView,
  Arrangement,
  Color,
  Dimension,
  HasChild,
  HasChildren,
  Inset,
  ItemAlignment,
  MouseCursor,
  NodeId,
  Part,
  Platform,
  State,
  View,
  Visibility,
  When,
};
use skene_reactive::{Get, Memo, Signal, use_context};

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
/// every facade. The chosen value flows both ways, exactly as on [`BasicPicker`]; for
/// custom chrome, compose the raw control directly.
#[derive(Debug)]
#[must_use]
pub struct SimplePicker<P: Platform> {
  options: Vec<(String, String)>,
  picker: BasicPicker<P>,
}

impl<P: Platform> SimplePicker<P> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      options: Vec::new(),
      picker: BasicPicker::new(),
    }
  }

  /// Sets the options, in order, each a `(value, label)` pair.
  pub fn set_options<V: Into<String>, L: Into<String>>(
    mut self,
    options: impl IntoIterator<Item = (V, L)>,
  ) -> Self {
    self.options = options
      .into_iter()
      .map(|(value, label)| (value.into(), label.into()))
      .collect();

    self
  }

  /// Sets the chosen value.
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.picker = self.picker.set_value(value);

    self
  }

  /// Derives the chosen value from the signals `value` reads.
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.picker = self.picker.derive_value(value);

    self
  }

  /// Runs `action` with the value a choice asks for.
  pub fn handle_change(mut self, action: impl Fn(String) + 'static) -> Self {
    self.picker = self.picker.handle_change(action);

    self
  }

  /// Binds the chosen value to `signal` in both directions.
  #[inline(always)]
  pub fn bind_value(mut self, signal: Signal<String>) -> Self {
    self.picker = self.picker.bind_value(signal);

    self
  }

  /// Sets the accessible name.
  pub fn set_label(mut self, label: impl Into<String>) -> Self {
    self.picker = self.picker.set_label(label.into());

    self
  }

  /// Sets whether the picker is present but not operable.
  #[inline(always)]
  pub fn set_disabled(mut self, disabled: bool) -> Self {
    self.picker = self.picker.set_disabled(disabled);

    self
  }

  /// Derives whether the picker is disabled from the signals `disabled` reads.
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.picker = self.picker.derive_disabled(disabled);

    self
  }
}

impl<P: Platform> Default for SimplePicker<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Platform> AnyKind for SimplePicker<P> {}

impl<P: Platform> View<P> for SimplePicker<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
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
      .set_color(SimplePalette::<P::Scalar>::CARD)
      .set_border_width(hairline())
      .set_border_color(SimplePalette::<P::Scalar>::BORDER)
      .set_corner_radius(shape.control_radius)
      .set_child(Column::<P>::new().set_children(rows));

    picker
      .set_width(Dimension::Fraction(1.))
      .set_padding(shape.control_inset)
      .set_color(SimplePalette::<P::Scalar>::CARD.when(
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
      .set_face(move |value: Memo<String>| face::<P>(&labels, value))
      .build()
  }
}

/// The face: the chosen option's label, and the indicator as the [`Part::Face`].
fn face<P: Platform>(labels: &Rc<Vec<(String, String)>>, value: Memo<String>) -> Row<P>
where
  P::Scalar: SimpleUnit,
{
  let labels = Rc::clone(labels);
  let shown = move || {
    let chosen = value.get();

    labels
      .iter()
      .find(|(value, _)| *value == chosen)
      .map_or(chosen, |(_, label)| label.clone())
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
    .set_spacing(shape::<P>().indicator_spacing)
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
fn row<P: Platform>(value: &str, label: &str) -> AnyView<P>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  let check = Container::<P>::new()
    .set_visibility(Visibility::Hidden.when(State::Checked, Visibility::Visible))
    .set_child(Text::<P>::new(CHECK).set_color(SimplePalette::<P::Scalar>::PRIMARY_INK));
  let caption = Text::<P>::new(label).set_color(
    SimplePalette::<P::Scalar>::FOREGROUND
      .when(State::Checked, SimplePalette::<P::Scalar>::PRIMARY_INK),
  );

  BasicListboxOption::<P>::new(ListboxOption::new(value))
    .set_padding(shape::<P>().row_inset)
    .set_color(
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
