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
  Modifiers,
  MouseCursor,
  PointerButton,
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
  Button,
  Children,
  Condition,
  Container,
  DynamicView,
  FocusHandle,
  HasChildren,
  Input,
  Inset,
  PointerEvent,
  RunStyle,
  Text,
  derive_title,
  empty,
  install_router,
  launch,
  set_title,
};
