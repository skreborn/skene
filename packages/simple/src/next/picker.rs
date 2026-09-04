//! A ready-made dropdown choice in the Simple design language.

use std::rc::Rc;

use skene_core::{
  Arrangement,
  Color,
  Dimension,
  ItemAlignment,
  MouseCursor,
  next::{AnyKind, AnyView, Driver, HasChildren, IntoView, NodeId, Part, State, View, When},
};
use skene_reactive::{Get, Memo, Signal};
use skene_views::{
  Column,
  Container,
  CornerRadius,
  Label,
  ListItem,
  RawOption,
  RawPicker,
  Row,
  Text,
};

use super::{
  SimplePalette,
  shape::{
    CONTROL_INSET,
    INDICATOR_GAP,
    LIST_INSET,
    MARK_RADIUS,
    RADIUS,
    ROW_INSET,
    as_part,
    define_defaults,
    hairline,
  },
};

/// The glyph after the chosen label.
const INDICATOR: &str = "▾";
/// The glyph before the chosen row.
const CHECK: &str = "✓";

/// A ready-made dropdown choice: field chrome whose hairline becomes the ring while
/// focused, an indicator after the chosen label, and a list on the card surface whose
/// rows highlight while focused and fill in the primary look while chosen.
///
/// The indicator is the face's [`Part::Face`], the list surface the [`Part::List`] and
/// each row's check the [`Part::Item`], so all three follow their control's state on
/// every facade. The chosen value flows both ways, exactly as on [`RawPicker`]; for
/// custom chrome, compose the raw control directly.
#[must_use]
pub struct SimplePicker<D: Driver> {
  options: Vec<(String, String)>,
  picker: RawPicker<D>,
}

impl<D: Driver> SimplePicker<D> {
  /// A picker with no options and nothing chosen.
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      options: Vec::new(),
      picker: RawPicker::new(),
    }
  }

  /// Sets the options, in order, each a `(value, label)` pair.
  #[inline(always)]
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
  #[inline(always)]
  pub fn set_value(mut self, value: impl Into<String>) -> Self {
    self.picker = self.picker.set_value(value);

    self
  }

  /// Derives the chosen value from the signals `value` reads.
  #[inline(always)]
  pub fn derive_value(mut self, value: impl Fn() -> String + 'static) -> Self {
    self.picker = self.picker.derive_value(value);

    self
  }

  /// Runs `action` with the value a choice asks for.
  #[inline(always)]
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
  #[inline(always)]
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
  #[inline(always)]
  pub fn derive_disabled(mut self, disabled: impl Fn() -> bool + 'static) -> Self {
    self.picker = self.picker.derive_disabled(disabled);

    self
  }
}

impl<D: Driver> Default for SimplePicker<D> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<D: Driver> AnyKind for SimplePicker<D> {}

impl<D: Driver> View<D> for SimplePicker<D>
where
  D::StyleExt: CornerRadius,
{
  fn build(self) -> NodeId {
    define_defaults::<D>();

    let Self { options, picker } = self;
    let labels = Rc::new(options);
    let rows: Vec<AnyView<D>> = labels
      .iter()
      .map(|(value, label)| row::<D>(value, label))
      .collect();
    let surface = Container::<D>::new()
      .set_padding(LIST_INSET)
      .set_color(SimplePalette::<D::Scalar>::CARD)
      .set_border(hairline(SimplePalette::<D::Scalar>::BORDER))
      .set_corner_radius(RADIUS)
      .set_child(Column::<D>::new().set_children(rows));

    picker
      .set_width(Dimension::Fraction(1.))
      .set_padding(CONTROL_INSET)
      .set_color(SimplePalette::<D::Scalar>::CARD)
      .set_border(hairline(SimplePalette::<D::Scalar>::BORDER).when(
        State::FocusVisible,
        hairline::<D::Scalar>(SimplePalette::<D::Scalar>::RING),
      ))
      .set_corner_radius(RADIUS)
      .set_cursor(MouseCursor::Click)
      .set_options([AnyView::<D>::new(move || as_part::<D>(Part::List, surface))])
      .set_face(move |value: Memo<String>| face::<D>(&labels, value))
      .build()
  }
}

/// The face: the chosen option's label, and the indicator as the [`Part::Face`].
fn face<D: Driver>(labels: &Rc<Vec<(String, String)>>, value: Memo<String>) -> Row<D> {
  let labels = Rc::clone(labels);
  let shown = move || {
    let chosen = value.get();

    labels
      .iter()
      .find(|(value, _)| *value == chosen)
      .map_or(chosen, |(_, label)| label.clone())
  };
  let indicator = Text::<D>::new(INDICATOR).set_color(
    SimplePalette::<D::Scalar>::MUTED_FOREGROUND
      .when(State::Open, SimplePalette::<D::Scalar>::FOREGROUND),
  );

  Row::<D>::new()
    .set_spacing(INDICATOR_GAP)
    .set_arrangement(Arrangement::SpaceBetween)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::<D>::derive(shown),
      as_part::<D>(Part::Face, indicator),
    ))
}

/// One row of the list: highlighted while focused, filled in the primary look while
/// chosen, its check the [`Part::Item`] and its caption the label.
fn row<D: Driver>(value: &str, label: &str) -> AnyView<D>
where
  D::StyleExt: CornerRadius,
{
  let check = Text::<D>::new(CHECK)
    .set_color(Color::TRANSPARENT.when(State::Checked, SimplePalette::<D::Scalar>::PRIMARY_INK));
  let caption = Text::<D>::new(label).set_color(
    SimplePalette::<D::Scalar>::FOREGROUND
      .when(State::Checked, SimplePalette::<D::Scalar>::PRIMARY_INK),
  );

  RawOption::<D>::new(ListItem::new(value))
    .set_padding(ROW_INSET)
    .set_color(
      Color::TRANSPARENT
        .when(
          State::Focused,
          SimplePalette::<D::Scalar>::SECONDARY_SURFACE,
        )
        .when(State::Checked, SimplePalette::<D::Scalar>::PRIMARY_SURFACE),
    )
    .set_corner_radius(MARK_RADIUS)
    .set_cursor(MouseCursor::Click)
    .set_part(Part::Item, check)
    .set_label(Label::new(caption))
    .into_view()
}
