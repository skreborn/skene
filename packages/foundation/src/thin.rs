//! A view over the app's own types, thin over a core that is not.

use crate::{Realization, View};

/// A view generic over the app's own types, thin over a core that is not: a
/// [`BasicPicker`](crate::BasicPicker) of the app's values over a
/// [`BasicPickerCore`](crate::BasicPickerCore).
///
/// Its members erase what they are given into the core, and it builds as its core, so
/// whatever the app's types, the core's build is compiled once per realization.
pub trait Thin<P: Realization> {
  /// The core: this view without the app's types.
  type Core: View<P>;

  /// The core, holding everything this view was given.
  fn into_core(self) -> Self::Core;
}
