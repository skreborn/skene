//! A box the app draws: kurbo's shapes and peniko's brushes, recorded in device pixels and
//! replayed by the platform in the scene's order.

pub use skene_foundation::view::painted::{Canvas, Paint, paints, shapes};
use skene_foundation::{
  view::{self as base, PaintedBase},
  wraps,
};

use crate::Active;

/// A box the app draws on a [`Canvas`]: kurbo's shapes and peniko's brushes, recorded in
/// device pixels and replayed by the platform in the scene's order, beneath what is drawn
/// after it.
///
/// The closure runs again whenever a signal it reads changes, whenever the states it read
/// change, and whenever the box is drawn at another device size or scale. The box is as large
/// as its sizing makes it: give it a width and a height, or a weight in its layout. Without a
/// description it is decoration a reader skips.
///
/// ```
/// use skene_gui::{painted::shapes::Circle, prelude::*};
///
/// let level = Signal::new(0.5_f64);
///
/// Painted::new(move |canvas| {
///   canvas.set_transform(canvas.to_device());
///
///   let radius = canvas.layout_size().height / 2.;
///
///   canvas.fill(&Circle::new((radius, radius), radius), Color::rgb(0x34c759));
///   canvas.fill(&Circle::new((radius, radius), radius * level.get()), Paint::Ink);
/// })
/// .set_width(48)
/// .set_height(48)
/// .set_description("Level");
/// ```
#[must_use]
pub struct Painted(base::Painted<Active>);

wraps!([] Painted => base::Painted<Active>, Active);

impl PaintedBase<Active> for Painted {}
