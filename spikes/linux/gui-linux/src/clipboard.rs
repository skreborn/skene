//! The clipboard and the primary selection, whichever display server the window is on:
//! Wayland's `wl_data_device` and primary selection, X11's `CLIPBOARD` and `PRIMARY`.
//!
//! A write offers the text until another client takes the selection; a read asks the
//! selection's owner for UTF-8 text and hands it over once it arrives (at once where the
//! window owns the selection itself). Both are queued here and carried out by the backend on
//! its loop.

use std::cell::RefCell;

/// Which selection.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Selection {
  /// The clipboard: what copy writes and paste reads.
  Clipboard,
  /// The primary selection: what is selected, pasted with the middle button.
  Primary,
}

/// What a read hands its text to: `None` where the selection is empty or holds no text.
pub type Then = Box<dyn FnOnce(Option<String>)>;

/// A queued operation.
pub enum Op {
  /// Offer the text.
  Write(Selection, String),
  /// Read the text.
  Read(Selection, Then),
}

thread_local! {
  static OPS: RefCell<Vec<Op>> = const { RefCell::new(Vec::new()) };
}

/// Offers `text` as `selection`.
pub fn write(selection: Selection, text: impl Into<String>) {
  OPS.with(|ops| ops.borrow_mut().push(Op::Write(selection, text.into())));
  crate::runtime::inbox().send(crate::runtime::Message::Wake);
}

/// Reads `selection`'s text, handing it to `then` once it arrives.
pub fn read(selection: Selection, then: impl FnOnce(Option<String>) + 'static) {
  OPS.with(|ops| ops.borrow_mut().push(Op::Read(selection, Box::new(then))));
  crate::runtime::inbox().send(crate::runtime::Message::Wake);
}

/// The operations queued since the last call: what the backend carries out.
pub fn take() -> Vec<Op> {
  OPS.with(|ops| std::mem::take(&mut *ops.borrow_mut()))
}
