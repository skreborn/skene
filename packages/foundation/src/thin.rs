//! A view over the app's own types, thin over a core that is not.

use crate::{Realization, View};

/// A view generic over the app's own types, thin over a core that is not.
///
/// A [`Control`](crate::Control) over any behavior is thin over a
/// [`ControlCore`](crate::ControlCore), a [`Theme`](crate::Theme) of any palette over a
/// [`ThemeCore`](crate::ThemeCore), a [`OneOf`](crate::OneOf) over any key over a
/// [`OneOfCore`](crate::OneOfCore), a [`Link`](crate::Link) to any route over a
/// [`LinkCore`](crate::LinkCore) and a [`BasicPicker`](crate::BasicPicker) of the app's
/// values over a [`BasicPickerCore`](crate::BasicPickerCore).
///
/// Its members erase what they are given into the core, and it builds as its core, so
/// whatever the app's types, the core's build is compiled once per realization: a
/// facade's view over it names the core in its derive (`#[view(core = …)]`) and builds
/// it through a function generic in nothing, compiled in the facade.
#[diagnostic::on_unimplemented(
  message = "`{Self}` is not a view thin over a core",
  label = "a view over the app's own types is thin over a core",
  note = "a view over the app's own types erases them into a core that is not generic over them and builds as that core"
)]
pub trait Thin<P: Realization> {
  /// The core: this view without the app's types.
  type Core: View<P>;

  /// The core, holding everything this view was given.
  fn into_core(self) -> Self::Core;
}
