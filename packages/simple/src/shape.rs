//! The Simple shape language: the lengths, radii and chrome every Simple view is drawn
//! with, spelled once per medium in the medium's own unit.

use skene_core::{
  Border,
  Color,
  Dimension,
  Driver,
  Inset,
  IntoView,
  MouseCursor,
  NodeId,
  Part,
  Scalar,
  Scene,
  State,
  TokenRef,
  View,
  When,
};
use skene_reactive::Memo;
use skene_views::{Behavior, Control, CornerRadius, ThemedIn};

use super::SimplePalette;

/// The corner radius of a control, in the medium's unit.
pub const RADIUS: f32 = 8.;
/// The corner radius of a card.
pub const CARD_RADIUS: f32 = 12.;
/// The corner radius of a checkbox mark.
pub const MARK_RADIUS: f32 = 4.;
/// The corner radius rounding a one-em box into a circle.
pub const ROUND: f32 = 8.;
/// The corner radius rounding a slider track's ends.
pub const TRACK_RADIUS: f32 = 3.;

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
  /// The inset between a card's edge and its content.
  pub card_inset: Inset<N>,
  /// The inset between a picker's list surface and its rows.
  pub list_inset: Inset<N>,
  /// The inset between a picker row's edge and its caption.
  pub row_inset: Inset<N>,
  /// The inset between a tooltip bubble's edge and its text.
  pub tooltip_inset: Inset<N>,
  /// The spacing between a picker's label and its indicator.
  pub indicator_gap: N,
  /// The spacing between a menu item's caption and its shortcut hint.
  pub shortcut_gap: N,
  /// The spacing between a card's children.
  pub card_spacing: N,
  /// The edge of a checkbox mark or a radio ring.
  pub mark_size: N,
  /// The thickness of a mark's or a ring's border; zero draws none.
  pub mark_ring: N,
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
  label = "a Simple view is drawn in logical pixels (`f32`) or cells (`u32`)"
)]
pub trait SimpleUnit: Scalar {
  /// The shape in this unit.
  const SHAPE: SimpleShape<Self>;
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
  const SHAPE: SimpleShape<Self> = SimpleShape {
    control_inset: Inset::symmetric(12., 8.),
    control_ring: 1.,
    field_ring: 1.,
    card_inset: Inset::all(24.),
    list_inset: Inset::all(4.),
    row_inset: Inset::all(6.),
    tooltip_inset: Inset::symmetric(8., 4.),
    indicator_gap: 8.,
    shortcut_gap: 24.,
    card_spacing: 16.,
    mark_size: 18.,
    mark_ring: 1.,
    mark_surface: SimplePalette::<Self>::CARD,
    dot_size: 8.,
    track_height: 6.,
    thumb_size: 16.,
    thumb_ring: 2.,
    thumb_surface: SimplePalette::<Self>::PRIMARY_SURFACE,
    thumb_focus: SimplePalette::<Self>::PRIMARY_SURFACE,
  };
  const MARKS: SimpleMarks = SimpleMarks::Drawn;
}

impl SimpleUnit for u32 {
  const SHAPE: SimpleShape<Self> = SimpleShape {
    control_inset: Inset::symmetric(1, 0),
    control_ring: 0,
    field_ring: 1,
    card_inset: Inset::symmetric(2, 1),
    list_inset: Inset::all(0),
    row_inset: Inset::symmetric(1, 0),
    tooltip_inset: Inset::symmetric(1, 0),
    indicator_gap: 1,
    shortcut_gap: 2,
    card_spacing: 1,
    mark_size: 1,
    mark_ring: 0,
    mark_surface: SimplePalette::<Self>::BORDER,
    dot_size: 1,
    track_height: 1,
    thumb_size: 1,
    thumb_ring: 0,
    thumb_surface: SimplePalette::<Self>::MUTED_FOREGROUND,
    thumb_focus: SimplePalette::<Self>::RING,
  };
  const MARKS: SimpleMarks = SimpleMarks::Glyph;
}

/// The shape of the driver's unit.
#[inline(always)]
pub const fn shape<D: Driver>() -> SimpleShape<D::Scalar>
where
  D::Scalar: SimpleUnit,
{
  <D::Scalar as SimpleUnit>::SHAPE
}

/// A border `width` thick in the token `color`.
#[inline(always)]
pub fn border<N: Scalar>(width: N, color: TokenRef<Color>) -> Border<N> {
  Border::new(width, color)
}

/// A one-unit border in the token `color`.
#[inline(always)]
pub fn hairline<N: Scalar>(color: TokenRef<Color>) -> Border<N> {
  border(N::from_f32(1.), color)
}

/// The palette in force as a memo, its default defined at the root where no theme
/// provides one.
#[inline(always)]
pub fn palette<D: Driver>() -> Memo<SimplePalette<D::Scalar>> {
  <SimplePalette<D::Scalar> as ThemedIn<D>>::watch()
}

/// Defines the default palette at the root where no theme provides one, so a view
/// referencing the tokens resolves them.
#[inline(always)]
pub fn define_defaults<D: Driver>() {
  let _ = palette::<D>();
}

/// Builds `view` and records it as `part` of the control it is mounted under.
pub fn as_part<D: Driver>(part: Part, view: impl IntoView<D>) -> NodeId {
  let node = view.into_view().build();

  Scene::<D>::with(|scene| scene.set_part(node, part));

  node
}

/// The chrome of a text entry: full width, padded, on the card surface, a hairline in
/// the border color that becomes the ring while focused, rounded.
pub fn field_chrome<D: Driver, B: Behavior<D>>(control: Control<D, B>) -> Control<D, B>
where
  D::Scalar: SimpleUnit,
  D::StyleExt: CornerRadius,
{
  let shape = shape::<D>();

  control
    .set_width(Dimension::Fraction(1.))
    .set_padding(shape.control_inset)
    .set_color(SimplePalette::<D::Scalar>::CARD)
    .set_border(
      border(shape.field_ring, SimplePalette::<D::Scalar>::BORDER).when(
        State::FocusVisible,
        border(shape.field_ring, SimplePalette::<D::Scalar>::RING),
      ),
    )
    .set_corner_radius(RADIUS)
    .set_cursor(MouseCursor::Text)
}
