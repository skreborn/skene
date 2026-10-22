//! The editing core every drawn text entry shares, written once over the geometry and the
//! boundaries its engine reports.
//!
//! It is Parley's cursor and selection model run over any [`TextEngine`](super::TextEngine):
//! a position is a byte of the text and an affinity, a selection an anchor and a focus, and a
//! vertical move keeps a horizontal goal. Over it sit the commands and the keys bound to them,
//! undo and redo with typing merged into steps, the composition an input method marks, the
//! input-client surface each platform's adapter answers its protocol from, and the controlled
//! contract that lets the app's truth decide.

mod command;
mod controlled;
mod editor;
mod history;
mod keys;

#[cfg(test)]
mod tests;

pub use self::{
  command::{Command, Movement, Outcome},
  controlled::{Arbiter, ControlledText},
  editor::{Clause, ClauseStyle, Composition, Editor, Notice, Origin, SelectionEnd},
  history::{EditKind, History, Step},
  keys::Binding,
};
