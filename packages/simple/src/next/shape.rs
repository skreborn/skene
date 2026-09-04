//! The Simple shape language: the lengths, radii and chrome every Simple view is drawn
//! with, spelled in the medium's font size so one shape serves pixels and cells.

use skene_core::{
  Color,
  Dimension,
  MouseCursor,
  next::{
    Border,
    Driver,
    Expr,
    IntoView,
    Length,
    NodeId,
    Part,
    Scalar,
    Scene,
    State,
    TokenRef,
    View,
    When,
  },
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
/// The inset between a control's edge and its content.
pub const CONTROL_INSET: Expr<Length> = Expr::em(0.5);
/// The inset between a card's edge and its content.
pub const CARD_INSET: Expr<Length> = Expr::em(1.5);
/// The inset between a picker's list surface and its rows.
pub const LIST_INSET: Expr<Length> = Expr::em(0.25);
/// The inset between a picker row's edge and its caption.
pub const ROW_INSET: Expr<Length> = Expr::em(0.375);
/// The spacing between a picker's label and its indicator.
pub const INDICATOR_GAP: Expr<Length> = Expr::em(0.5);
/// The spacing between a card's children.
pub const CARD_SPACING: Expr<Length> = Expr::em(1.);
/// The edge of a checkbox mark or a radio ring.
pub const MARK_SIZE: Expr<Length> = Expr::em(1.);
/// The diameter of a radio's dot.
pub const DOT_SIZE: Expr<Length> = Expr::em(0.5);
/// The thickness of a slider's track.
pub const TRACK_HEIGHT: Expr<Length> = Expr::em(0.375);
/// The diameter of a slider's thumb.
pub const THUMB_SIZE: Expr<Length> = Expr::em(1.);

/// A border `width` units thick in the token `color`.
#[inline(always)]
pub fn border<N: Scalar>(width: f32, color: TokenRef<Color>) -> Border<N> {
  Border::new(N::from_f32(width), color)
}

/// A one-unit border in the token `color`.
#[inline(always)]
pub fn hairline<N: Scalar>(color: TokenRef<Color>) -> Border<N> {
  border(1., color)
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
  D::StyleExt: CornerRadius,
{
  control
    .set_width(Dimension::Fraction(1.))
    .set_padding(CONTROL_INSET)
    .set_color(SimplePalette::<D::Scalar>::CARD)
    .set_border(hairline(SimplePalette::<D::Scalar>::BORDER).when(
      State::FocusVisible,
      hairline::<D::Scalar>(SimplePalette::<D::Scalar>::RING),
    ))
    .set_corner_radius(RADIUS)
    .set_cursor(MouseCursor::Text)
}
