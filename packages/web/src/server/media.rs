//! The environment a server render answers with: the hints it was given, as signals.

use std::cell::Cell;

use skene_foundation::{ColorScheme, Environment, Inset, MediaSignals, Size};
use skene_reactive::{ReadSignal, Set, Signal, WriteSignal};

use super::{HINTS, MEDIA};
use crate::hints::RenderHints;

/// The environment's signals with their write halves.
pub(super) struct MediaState {
  /// The environment's signals.
  signals: MediaSignals<f32>,
  /// Writes the viewport.
  set_viewport: WriteSignal<Size<f32>>,
  /// Writes the color scheme.
  set_scheme: WriteSignal<ColorScheme>,
  /// Writes whether the reader prefers reduced motion.
  set_reduced_motion: WriteSignal<bool>,
  /// The server's render is never hydrated and shows no pixels.
  pub(super) absent: ReadSignal<bool>,
}

/// Sets the environment the following renders answer with.
pub fn set_render_hints(hints: RenderHints) {
  HINTS.with(|current| current.set(hints));
  MEDIA.with(|media| {
    if let Some(media) = &*media.borrow() {
      media.set_viewport.set(hints.viewport);
      media.set_scheme.set(hints.scheme);
      media.set_reduced_motion.set(hints.reduced_motion);
    }
  });
}

/// The environment's signals, created under the root owner on first use.
pub(super) fn media_signals() -> MediaSignals<f32> {
  with_media(|media| media.signals)
}

/// Runs `operation` on the environment state, created under the root owner on first
/// use.
pub(super) fn with_media<R>(operation: impl FnOnce(&MediaState) -> R) -> R {
  MEDIA.with(|media| {
    let mut media = media.borrow_mut();
    let media = media.get_or_insert_with(|| {
      let hints = HINTS.with(Cell::get);

      skene_foundation::root_owner().enter(|| {
        let (signals, writers) = MediaSignals::new(Environment {
          viewport: hints.viewport,
          scheme: hints.scheme,
          reduced_motion: hints.reduced_motion,
          reduced_transparency: hints.reduced_transparency,
          high_contrast: false,
          pointer: hints.pointer,
          hover: hints.hover,
          safe_area: Inset::ZERO,
        });
        let (absent, _) = Signal::transient(false).split();

        MediaState {
          signals,
          set_viewport: writers.viewport,
          set_scheme: writers.scheme,
          set_reduced_motion: writers.reduced_motion,
          absent,
        }
      })
    });

    operation(media)
  })
}
