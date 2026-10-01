//! A ready-made slider in the Simple design language.

use skene_foundation::{View, Wraps};
use skene_simple::SimpleSliderBase;
use skene_terminal::ActivePlatform;

/// A ready-made slider: a track on the secondary surface, the run filled to the value
/// in the primary surface, and a thumb that rings while focused, or in cells takes the
/// ring's color.
///
/// The track, the fill and the thumb are the control's
/// [`Part::Track`](skene_terminal::Part::Track), [`Part::Fill`](skene_terminal::Part::Fill) and
/// [`Part::Thumb`](skene_terminal::Part::Thumb); the fill runs to the thumb's leading edge and
/// the thumb travels the track less its own size, so both stay inside the track at either end. The
/// value flows both ways. For a slider drawn some other way, compose
/// [`BasicSlider`](skene_terminal::BasicSlider) directly.
///
/// ```
/// use skene_simple_terminal::prelude::*;
/// use skene_terminal::{SliderRange, prelude::*};
///
/// let volume = Signal::new(50.);
/// let slider = SimpleSlider::new()
///   .set_label("Volume")
///   .set_range(SliderRange::new(0., 100., 5.))
///   .bind_value(volume);
/// ```
#[derive(Debug, Default, View, Wraps)]
#[must_use]
pub struct SimpleSlider(skene_simple::SimpleSlider<ActivePlatform>);

impl SimpleSliderBase<ActivePlatform> for SimpleSlider {}
