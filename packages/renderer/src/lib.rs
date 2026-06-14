//! # skene renderer abstraction
//!
//! This crate defines the boundary between skene's core (the View tree and
//! reactive glue) and whatever actually renders it. The core builds and mutates
//! a retained tree of [`NodeId`]s through the [`Backend`] trait, attaches
//! semantic [`Style`]s, and receives abstract [`Event`]s back through an
//! [`EventSink`].
//!
//! Crucially, nothing here knows about CSS, the DOM, GPUs, or terminals. That
//! keeps the same abstraction usable by:
//!
//! * a **DOM** backend (the one shipped today) that offloads layout/paint/text to the browser,
//! * a future **native** backend that does its own layout + paint + accessibility,
//! * a future **TUI** backend.
//!
//! See [`Backend`] for the full contract.

mod backend;
mod event;
mod node;
mod style;

pub use backend::{Backend, EventSink};
pub use event::{Event, EventKind, KeyEvent, Modifiers, PointerButton, PointerEvent};
pub use node::{ElementKind, NodeId};
pub use style::{AlignItems, Color, Cursor, Display, FlexDirection, JustifyContent, Length, Style};
