//! The Simple component library's facade-side machinery: the shape vocabulary in
//! logical pixels and the shared builder surface. The palette and the emphasis
//! variants come from `skene-simple`, shared with every other facade.
//!
//! Every `Simple*` view composes the raw controls through their public surface alone,
//! so a third-party component library sits on exactly the same footing.

use skene_core::{Border, BoxStyle, Condition, Inset};
pub use skene_simple::{SimplePalette, SimpleVariant};

/// The corner radius of a Simple control.
pub(crate) const RADIUS: f32 = 8.;
/// The corner radius of a [`SimpleCard`](crate::SimpleCard).
pub(crate) const CARD_RADIUS: f32 = 12.;

/// The shared field chrome: padded, hairline-bordered, rounded.
pub(crate) fn field_style() -> BoxStyle {
  BoxStyle::new()
    .set_padding(Inset::symmetric(12., 8.))
    .set_border(Border::new(1., SimplePalette::border()))
    .set_border_radius(RADIUS)
}

/// The shared field focus variant: the border brightens to the ring.
pub(crate) fn field_focus() -> (Condition, BoxStyle) {
  (
    Condition::Focused,
    BoxStyle::new().set_border(Border::new(1., SimplePalette::ring())),
  )
}

/// Generates the shared non-styling control surface on a Simple control with a `raw`
/// field, each method forwarded to the raw control: focus order, autofocus, focus and
/// anchor binding, and the focus/blur/key handlers.
macro_rules! simple_surface {
  ($control:ident) => {
    impl $control {
      /// Sets the control's place in sequential focus traversal.
      #[inline(always)]
      pub fn set_focus_order(mut self, focus_order: skene_core::FocusOrder) -> Self {
        self.raw = self.raw.set_focus_order(focus_order);

        self
      }

      /// Makes the control take focus when the page loads.
      #[inline(always)]
      pub fn set_autofocus(mut self) -> Self {
        self.raw = self.raw.set_autofocus();

        self
      }

      /// Binds `handle` to the control, so
      /// [`FocusHandle::focus`](crate::FocusHandle::focus) targets it once built.
      #[inline(always)]
      pub fn bind_focus(mut self, handle: &crate::FocusHandle) -> Self {
        self.raw = self.raw.bind_focus(handle);

        self
      }

      /// Binds `anchor` to the control, so an overlay's
      /// [`set_anchor`](crate::Overlay::set_anchor) positions against it once built.
      #[inline(always)]
      pub fn bind_anchor(mut self, anchor: &crate::Anchor) -> Self {
        self.raw = self.raw.bind_anchor(anchor);

        self
      }

      /// Sets the focus handler.
      #[inline(always)]
      pub fn handle_focus(mut self, action: impl Fn() + 'static) -> Self {
        self.raw = self.raw.handle_focus(action);

        self
      }

      /// Sets the blur handler.
      #[inline(always)]
      pub fn handle_blur(mut self, action: impl Fn() + 'static) -> Self {
        self.raw = self.raw.handle_blur(action);

        self
      }

      /// Sets the key-press handler.
      #[inline(always)]
      pub fn handle_key_down<R: Into<skene_core::Prevent>>(
        mut self,
        action: impl Fn(&skene_core::KeyEvent) -> R + 'static,
      ) -> Self {
        self.raw = self.raw.handle_key_down(action);

        self
      }
    }
  };
}

pub(crate) use simple_surface;
