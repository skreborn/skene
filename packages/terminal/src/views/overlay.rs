//! Content above the app, opened and closed by the app.

use skene_foundation::{
  AnyView,
  DecorationBase,
  HasBoxStyle,
  MotionBase,
  NoChild,
  OverlayBase,
  View,
  Wraps,
};

use crate::ActivePlatform;

/// Content above the app, opened and closed by the app.
///
/// A [`modal`](Self::modal) traps focus, blocks interaction beneath it and paints a
/// [backdrop](Self::set_backdrop); a [`popup`](Self::popup) floats without blocking and
/// is placed against an [anchor](Self::set_anchor).
///
/// The overlay never closes itself: `Escape`, a backdrop click or a press outside a
/// popup only run [`handle_dismiss`](Self::handle_dismiss), and the open state stays
/// what [`derive_open`](Self::derive_open) reads; [`bind_open`](Self::bind_open) wires
/// both to one signal. Opening and closing attach nothing and destroy nothing, so state
/// inside the content survives. Its content box is decorated like a container. Holds
/// [`NoChild`] until [`set_child`](Self::set_child) supplies the content.
///
/// ```
/// use skene_terminal::prelude::*;
///
/// let open = Signal::new(false);
/// let dialog = Overlay::modal()
///   .derive_open(move || open.get())
///   .set_label("Settings")
///   .set_child(Text::new("Settings"));
/// ```
#[derive(Debug, View, Wraps)]
#[must_use]
pub struct Overlay<C = NoChild>(skene_foundation::Overlay<ActivePlatform, C>);

impl OverlayBase<ActivePlatform> for Overlay {}

impl DecorationBase<ActivePlatform> for Overlay {}

impl MotionBase<ActivePlatform> for Overlay {}

impl Overlay {
  /// Sets the content.
  #[inline(always)]
  pub fn set_child(self, child: impl View<ActivePlatform>) -> Overlay<AnyView<ActivePlatform>> {
    Overlay(self.0.set_child(child))
  }
  crate::views::terminal_decoration!(box_style_mut);
}
