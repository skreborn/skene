//! # acme-sparkline
//!
//! A third-party low-level view written against the terminal platform's escape trait: a
//! [`Sparkline`] paints bar glyphs through [`CellPainter`] into an ordinary
//! [`Container`] that sizes it, so it composes with wrappers, themes and events as a
//! built-in does.
//!
//! Host-only: on a wasm target the crate is empty.

#![cfg(not(target_arch = "wasm32"))]

use std::marker::PhantomData;

use skene_common::{Container, realize::with_sink};
use skene_core::{AnyKind, Color, NodeId, Platform, View};
use skene_tui::CellPainter;

/// The bar glyphs, lowest to highest.
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A row of bars, one cell per value, scaled to the values' range.
#[must_use]
pub struct Sparkline<P: Platform> {
  values: Vec<f32>,
  color: Option<Color>,
  platform: PhantomData<fn() -> P>,
}

impl<P: Platform> Sparkline<P> {
  /// A sparkline of `values`, one cell wide per value.
  #[inline(always)]
  pub fn new(values: impl IntoIterator<Item = f32>) -> Self {
    Self {
      values: values.into_iter().collect(),
      color: None,
      platform: PhantomData,
    }
  }

  /// Sets the bars' color; without one they take the ambient foreground.
  #[inline(always)]
  pub const fn set_color(mut self, color: Color) -> Self {
    self.color = Some(color);

    self
  }
}

impl<P: Platform> AnyKind for Sparkline<P> {}

impl<P: Platform<Scalar = u32> + CellPainter> View<P> for Sparkline<P> {
  fn build(self) -> NodeId {
    let width = u32::try_from(self.values.len()).unwrap_or(u32::MAX);
    let node = Container::<P>::new().set_width(width).set_height(1).build();
    let levels = levels(&self.values);
    let color = self.color;

    with_sink::<P, _>(node, |sink| {
      P::default().paint(
        sink,
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
    });

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

      #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
      let level = ((value - least) / range * 7.).round() as usize;

      level.min(7)
    })
    .collect()
}
