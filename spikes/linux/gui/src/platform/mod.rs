//! Each platform's own vocabulary, as std's `os` holds each operating system's.
//!
//! What one platform alone has exists in that platform's build alone, so naming it outside
//! fails to compile. On a compositor platform that is its toolkit's own controls, under
//! Skene's names where the meaning matches (`derive_on`, `handle_change`, `set_placeholder`)
//! and the toolkit's own in their documentation and as doc aliases, and the native door
//! through which a library adopts a toolkit view of its own. Linux is a window platform: Skene
//! draws the whole window, there is no toolkit view to adopt, and a library's control is drawn
//! as a built-in's is (`Control` over a behavior of its own), so what remains here is the
//! display server's.

#[cfg(skene_platform = "linux")]
pub mod linux;
