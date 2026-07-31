//! Common imports for a terminal application: the view vocabulary, cell types, reactive
//! primitives, and the entry point.

pub use skene_core::{
  Axis,
  Color,
  CrossAxisAlignment,
  IntoChildren,
  IntoView,
  KeyEvent,
  Layout,
  MainAxisAlignment,
  View,
};
pub use skene_reactive::{
  Effect,
  Memo,
  ReadSignal,
  RwSignal,
  Scope,
  SignalGet,
  SignalUpdate,
  SignalWith,
  WriteSignal,
};

pub use crate::{
  BoxStyle,
  Children,
  Condition,
  Container,
  DynamicView,
  HasChildren,
  Input,
  Inset,
  RunStyle,
  Text,
  empty,
  install_router,
  launch,
};
