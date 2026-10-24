//! # acme-sparkline
//!
//! A third-party low-level view written against the terminal platform's escape trait: a
//! [`Sparkline`] paints bar glyphs through [`CellPainter`] into an ordinary
//! [`Container`] that sizes it, so it composes with wrappers, themes and events as a
//! built-in does, and hands the same box a pixel painter through [`Painting`], so a
//! terminal that proves a graphics protocol draws the bars at pixel height over the
//! glyphs while every other terminal keeps the glyphs: the floor always stands and the
//! upgrade arrives when the platform proves it, with nothing for the view's user to
//! choose.
//!
//! Built in the terminal's build alone: every other build compiles the crate empty.

#![cfg(skene_platform = "terminal")]

use std::marker::PhantomData;

use skene_foundation::{
  AnyKind,
  Color,
  Container,
  DecorationBase,
  NodeId,
  Painting,
  Realization,
  View,
  kurbo::Rect,
};
use skene_terminal::CellPainter;

/// The bar glyphs, lowest to highest.
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A row of bars, one cell per value, scaled to the values' range.
#[derive(Debug)]
#[must_use]
pub struct Sparkline<P: Realization> {
  values: Vec<f32>,
  color: Option<Color>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Realization> Sparkline<P> {
  /// A sparkline of `values`, one cell wide per value.
  #[inline(always)]
  pub fn new(values: impl IntoIterator<Item = f32>) -> Self {
    Self {
      values: values.into_iter().collect(),
      color: None,
      platform: PhantomData,
    }
  }

  /// Sets the bars' color; without one the glyphs take the ambient foreground and the
  /// pixels a mid gray.
  #[inline(always)]
  pub const fn set_color(mut self, color: Color) -> Self {
    self.color = Some(color);

    self
  }
}

impl<P: Realization> AnyKind for Sparkline<P> {}

impl<P: Realization<Scalar = i32> + CellPainter + Painting> View<P> for Sparkline<P> {
  fn build(self) -> NodeId {
    let width = i32::try_from(self.values.len()).unwrap_or(i32::MAX);
    let node = Container::<P>::new().set_width(width).set_height(1).build();
    let levels = levels(&self.values);
    let pixel_levels = levels.clone();
    let color = self.color;

    <P as CellPainter>::paint(
      node,
      Box::new(move |cells, _| {
        for (x, level) in levels.iter().enumerate() {
          let x = u32::try_from(x).unwrap_or(u32::MAX);

          cells.put(x, 0, BARS[*level]);

          if let Some(color) = color {
            cells.set_foreground(x, 0, color);
          }
        }
      }),
    );
    <P as Painting>::paint(
      node,
      Box::new(move |canvas, size| {
        let count = u32::try_from(pixel_levels.len().max(1)).unwrap_or(u32::MAX);
        let bar = f64::from(size.width) / f64::from(count);
        let brush = color.unwrap_or(Color::srgb(0.5, 0.5, 0.5));

        for (x, level) in pixel_levels.iter().enumerate() {
          let level = f64::from(u8::try_from(*level).unwrap_or(7));
          let height = f64::from(size.height) * (level + 1.) / 8.;
          let left = f64::from(u32::try_from(x).unwrap_or(u32::MAX)) * bar;

          canvas.fill(
            &Rect::new(
              left,
              f64::from(size.height) - height,
              left + bar,
              f64::from(size.height),
            ),
            brush,
          );
        }
      }),
    );

    node
  }
}

/// Each value's bar, scaled so the least value is the lowest bar and the greatest the
/// highest; equal values are all the lowest.
fn levels(values: &[f32]) -> Vec<usize> {
  let least = values.iter().copied().fold(f32::INFINITY, f32::min);
  let greatest = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
  let range = greatest - least;

  values
    .iter()
    .map(|value| {
      if range <= 0. {
        return 0;
      }

      #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a value within the range makes a level in 0..=7"
      )]
      let level = ((value - least) / range * 7.).round() as usize;

      level.min(7)
    })
    .collect()
}
