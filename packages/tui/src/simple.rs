//! The Simple component library's facade-side machinery: the shape vocabulary in
//! cells and the shared builder surface. The palette and the emphasis variants come
//! from `skene-simple`, shared with every other facade.
//!
//! Terminal interaction feedback rides the border, never the backdrop: hovering
//! brightens a Simple control's border to the ring, and focusing it inks the border
//! in the control's own ink.
//!
//! Every `Simple*` view composes the raw controls through their public surface alone,
//! so a third-party component library sits on exactly the same footing.

mod button;
mod card;
mod checkbox;
mod picker;
mod radio;
mod slider;
mod text_field;

pub use skene_simple::{SimplePalette, SimpleVariant};

pub use self::{
  button::SimpleButton,
  card::SimpleCard,
  checkbox::SimpleCheckbox,
  picker::SimplePicker,
  radio::SimpleRadio,
  slider::SimpleSlider,
  text_field::SimpleTextField,
};
use crate::props::{BorderGlyphs, BoxStyle, Condition};

/// The border every Simple surface draws with—arc corners, the terminal's radius.
pub(crate) const GLYPHS: BorderGlyphs = BorderGlyphs::ROUNDED;

/// The shared field chrome: padded, hairline-bordered, rounded.
pub(crate) fn field_style() -> BoxStyle {
  BoxStyle::new()
    .set_padding(crate::Inset::horizontal(1))
    .set_border(SimplePalette::border())
    .set_border_glyphs(GLYPHS)
}

/// The shared field hover variant: the border brightens to the ring.
pub(crate) fn field_hover() -> (Condition, BoxStyle) {
  (
    Condition::Hovered,
    BoxStyle::new().set_border(SimplePalette::ring()),
  )
}

/// The shared field focus variant: the border inks fully.
pub(crate) fn field_focus() -> (Condition, BoxStyle) {
  (
    Condition::Focused,
    BoxStyle::new().set_border(SimplePalette::foreground()),
  )
}

/// Generates the shared non-styling control surface on a Simple control with a `raw`
/// field, each method forwarded to the raw control: autofocus, focus and anchor
/// binding, and the focus/blur/key handlers.
macro_rules! simple_surface {
  ($control:ident) => {
    impl $control {
      /// Makes the control take focus when it mounts.
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
