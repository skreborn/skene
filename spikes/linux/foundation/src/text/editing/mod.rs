//! The editing core every drawn text entry shares, written once over the geometry and the
//! boundaries its engine reports: the text and its grapheme selection, the composition an
//! input method marks in it, the commands and the keys bound to them, undo and redo with
//! typing merged into steps, the input-client surface each platform's adapter answers its
//! protocol from, and the controlled contract that lets the app's truth decide.
//!
//! The terminal is its first client; Linux's IBus, Fcitx and text-input-v3 adapters, then
//! Android's `InputConnection`, iOS's `UITextInput` and macOS's `NSTextInputClient` speak the
//! same surface.

mod command;
mod controlled;
mod editor;
mod history;
mod keys;

#[cfg(test)]
mod tests;

pub use self::{
  command::{Command, Movement, Outcome},
  controlled::{Arbiter, Controlled},
  editor::{Clause, ClauseStyle, Composition, Editor, Notice, Origin},
  history::{EditKind, History, Snapshot},
  keys::Binding,
};
