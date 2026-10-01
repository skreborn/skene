//! A ready-made slider in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleSliderBase;
use skene_web::ActivePlatform;

/// A ready-made slider: a track on the secondary surface, the run filled to the value
/// in the primary surface, and a thumb that rings while focused.
///
/// The track, the fill and the thumb are the control's
/// [`Part::Track`](skene_web::Part::Track), [`Part::Fill`](skene_web::Part::Fill) and
/// [`Part::Thumb`](skene_web::Part::Thumb); the fill runs to the thumb's leading edge and
/// the thumb travels the track less its own size, so both stay inside the track at either end. The
/// value flows both ways. For a slider drawn some other way, compose
/// [`BasicSlider`](skene_web::BasicSlider) directly.
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleSlider(skene_simple::SimpleSlider<ActivePlatform>);

impl SimpleSliderBase<ActivePlatform> for SimpleSlider {}
