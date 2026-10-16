//! Linux: what the Linux build alone has.
//!
//! Skene draws every control on Linux, so the shared controls are Linux's own; what remains
//! here is the display server's: the clipboard and the primary selection.

/// The clipboard and the primary selection: Wayland's data device and primary selection,
/// X11's `CLIPBOARD` and `PRIMARY`.
pub mod clipboard {
  pub use skene_gui_linux::clipboard::{Selection, read, write};
}
