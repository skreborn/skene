//! The Simple shape language: the lengths, radii and chrome every Simple view is drawn
//! with, spelled once per medium in the medium's own unit.

use skene_foundation::{
  Behavior,
  Color,
  Condition,
  Control,
  CornerRadius,
  DecorationBase,
  DecorationWithCornerRadius,
  Dimension,
  Inset,
  Length,
  MouseCursor,
  NodeId,
  Part,
  Realization,
  Scalar,
  Scene,
  State,
  ThemedIn,
  TokenRef,
  Value,
  View,
  When,
};
use skene_reactive::Memo;

use super::SimplePalette;

/// The lengths the Simple views are drawn with, in one medium's unit.
///
/// [`SimpleUnit`] gives each unit its shape: logical pixels keep the language's pixel
/// metrics, cells take whole-cell insets and glyph-sized marks, and one view draws
/// both.
#[derive(Debug, Clone, Copy, PartialEq)]
#[must_use]
pub struct SimpleShape<N> {
  /// The inset between a control's edge and its content.
  pub control_inset: Inset<N>,
  /// The thickness of a button's border; zero draws none.
  pub control_ring: N,
  /// The thickness of a text entry's or a picker face's border; zero draws none.
  pub field_ring: N,
  /// The corner radius of a control.
  pub control_radius: Length,
  /// The inset between a card's edge and its content.
  pub card_inset: Inset<N>,
  /// The corner radius of a card.
  pub card_radius: Length,
  /// The inset between a picker's list surface and its rows.
  pub list_inset: Inset<N>,
  /// The inset between a picker row's edge and its caption.
  pub row_inset: Inset<N>,
  /// The inset between a tooltip bubble's edge and its text.
  pub tooltip_inset: Inset<N>,
  /// The spacing between a picker's label and its indicator.
  pub indicator_spacing: N,
  /// The spacing between a menu item's caption and its shortcut hint.
  pub shortcut_spacing: N,
  /// The spacing between a card's children.
  pub card_spacing: N,
  /// The edge of a checkbox mark or a radio ring.
  pub mark_size: N,
  /// The thickness of a mark's or a ring's border; zero draws none.
  pub mark_ring: N,
  /// The corner radius of a checkbox mark, a list row and a tooltip bubble.
  pub mark_radius: Length,
  /// The surface of an unchecked mark or ring, which stands in for the ring where it
  /// has no thickness.
  pub mark_surface: TokenRef<Color>,
  /// The diameter of a radio's dot.
  pub dot_size: N,
  /// The thickness of a slider's track.
  pub track_height: N,
  /// The diameter of a slider's thumb.
  pub thumb_size: N,
  /// The thickness of a slider thumb's border; zero draws none.
  pub thumb_ring: N,
  /// The surface of a slider's thumb.
  pub thumb_surface: TokenRef<Color>,
  /// The surface of a slider's thumb while focus is visible, which stands in for the
  /// ring where it has no thickness.
  pub thumb_focus: TokenRef<Color>,
}

/// A length unit the Simple views know their shape in.
#[diagnostic::on_unimplemented(
  message = "the Simple views have no shape in `{Self}`",
  label = "a Simple view is drawn in logical pixels (`f32`) or cells (`i32`)"
)]
pub trait SimpleUnit: Scalar {
  /// The shape in this unit.
  fn shape() -> SimpleShape<Self>;
  /// How the medium shows a control's marks.
  const MARKS: SimpleMarks;
}

/// How a medium shows a checkbox or radio mark and a picker's indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimpleMarks {
  /// Boxes the medium draws: a ring, a dot, a check.
  Drawn,
  /// Glyphs from the [`SimpleGlyphs`](crate::SimpleGlyphs) token set.
  Glyph,
}

impl SimpleUnit for f32 {
  fn shape() -> SimpleShape<Self> {
    SimpleShape {
      control_inset: Inset::symmetric(12., 8.),
      control_ring: 1.,
      field_ring: 1.,
      control_radius: Length(8.),
      card_inset: Inset::all(24.),
      card_radius: Length(12.),
      list_inset: Inset::all(4.),
      row_inset: Inset::all(6.),
      tooltip_inset: Inset::symmetric(8., 4.),
      indicator_spacing: 8.,
      shortcut_spacing: 24.,
      card_spacing: 16.,
      mark_size: 18.,
      mark_ring: 1.,
      mark_radius: Length(4.),
      mark_surface: SimplePalette::<Self>::CARD,
      dot_size: 8.,
      track_height: 6.,
      thumb_size: 16.,
      thumb_ring: 2.,
      thumb_surface: SimplePalette::<Self>::PRIMARY_SURFACE,
      thumb_focus: SimplePalette::<Self>::PRIMARY_SURFACE,
    }
  }

  const MARKS: SimpleMarks = SimpleMarks::Drawn;
}

impl SimpleUnit for i32 {
  fn shape() -> SimpleShape<Self> {
    SimpleShape {
      control_inset: Inset::symmetric(1, 0),
      control_ring: 0,
      field_ring: 1,
      control_radius: Length(0.25),
      card_inset: Inset::symmetric(2, 1),
      card_radius: Length(0.5),
      list_inset: Inset::all(0),
      row_inset: Inset::symmetric(1, 0),
      tooltip_inset: Inset::symmetric(1, 0),
      indicator_spacing: 1,
      shortcut_spacing: 2,
      card_spacing: 1,
      mark_size: 1,
      mark_ring: 0,
      mark_radius: Length(0.25),
      mark_surface: SimplePalette::<Self>::BORDER,
      dot_size: 1,
      track_height: 1,
      thumb_size: 1,
      thumb_ring: 0,
      thumb_surface: SimplePalette::<Self>::MUTED_FOREGROUND,
      thumb_focus: SimplePalette::<Self>::RING,
    }
  }

  const MARKS: SimpleMarks = SimpleMarks::Glyph;
}

/// The shape of the platform's unit.
pub(crate) fn shape<P: Realization>() -> SimpleShape<P::Scalar>
where
  P::Scalar: SimpleUnit,
{
  <P::Scalar as SimpleUnit>::shape()
}

/// The cursor over a control a click operates: the click cursor, and the default one
/// while the control is disabled.
pub(crate) fn click_cursor<N: Scalar>() -> Value<MouseCursor, Condition<N>> {
  MouseCursor::Click.when(Condition::from(State::Disabled), MouseCursor::Default)
}

/// A border's width of one unit on every edge.
pub(crate) fn hairline<N: Scalar>() -> Inset<N> {
  Inset::all(1)
}

/// The palette in force as a memo, its default defined at the root where no theme
/// provides one.
pub(crate) fn palette<P: Realization>() -> Memo<SimplePalette<P::Scalar>> {
  <SimplePalette<P::Scalar> as ThemedIn<P>>::watch()
}

/// Defines the default palette at the root where no theme provides one, so a view
/// referencing the tokens resolves them.
pub(crate) fn define_defaults<P: Realization>() {
  let _ = palette::<P>();
}

/// Builds `view` and records it as `part` of the control it is mounted under.
pub(crate) fn as_part<P: Realization>(part: Part, view: impl View<P>) -> NodeId {
  let node = view.build();

  Scene::<P>::with(|scene| scene.set_part(node, part));

  node
}

/// The chrome of a text entry: full width, padded, on the card surface, a hairline in
/// the border color that becomes the ring while focused, rounded; on the disabled
/// surface under the default cursor while disabled.
pub(crate) fn field_chrome<P: Realization, B: Behavior<P>>(control: Control<P, B>) -> Control<P, B>
where
  P::Scalar: SimpleUnit,
  P::StyleExt: CornerRadius,
{
  let shape = shape::<P>();

  control
    .set_width(Dimension::Fraction(1.))
    .set_padding(shape.control_inset)
    .set_background(SimplePalette::<P::Scalar>::CARD.when(
      State::Disabled,
      SimplePalette::<P::Scalar>::DISABLED_SURFACE,
    ))
    .set_border_width(Inset::all(shape.field_ring))
    .set_border_color(
      SimplePalette::<P::Scalar>::BORDER
        .when(State::FocusVisible, SimplePalette::<P::Scalar>::RING),
    )
    .set_corner_radius(shape.control_radius)
    .set_cursor(MouseCursor::Text.when(State::Disabled, MouseCursor::Default))
}
