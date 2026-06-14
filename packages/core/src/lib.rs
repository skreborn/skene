//! # skene core
//!
//! The view tree and reactive glue, rendered through the
//! [`skene_renderer`] backend abstraction. The core knows nothing about CSS,
//! the DOM, GPUs or terminals: views build a retained node tree and wire
//! reactive effects that patch nodes directly. Whatever [`Backend`] is installed
//! realizes those nodes.
//!
//! Backends install themselves with [`set_backend`] and then call [`mount`] to
//! build the root view.

mod element;
mod runtime;
mod view;
mod views;

pub use element::El;
pub use runtime::{register_handler, root_scope, set_backend, with_backend};
/// Re-export the reactive primitives so apps need only depend on the facade.
pub use skene_reactive as reactive;
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
/// Re-export the renderer abstraction's public types (styles, colors, events).
pub use skene_renderer as renderer;
pub use skene_renderer::{
  Axis,
  Backend,
  Color,
  CrossAxisAlignment,
  Cursor,
  Display,
  ElementKind,
  Event,
  EventKind,
  Length,
  MainAxisAlignment,
  NodeId,
  Style,
};
pub use view::{AnyView, IntoView, View};
pub use views::{
  Button,
  ButtonBuilder,
  Container,
  ContainerBuilder,
  DynView,
  Flex,
  FlexBuilder,
  Label,
  dyn_view,
};

/// Build the application root and insert it under the backend's root node.
///
/// `app` is invoked **inside** the app [`root_scope`] so that any signals,
/// memos or effects it creates live for the lifetime of the program (rather
/// than being disposed with a transient scope).
pub fn mount<V: IntoView>(app: impl FnOnce() -> V) -> NodeId {
  let scope = root_scope();
  scope.enter(|| {
    let node = app().into_view().build();
    let root = with_backend(|b| b.root());
    with_backend(|b| b.insert_child(root, node, None));
    node
  })
}
