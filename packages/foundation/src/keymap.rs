//! A keymap: keyboard shortcuts over the one view they serve.

use std::{marker::PhantomData, rc::Rc};

use crate::{
  AnyKind,
  AnyView,
  Handler,
  KeyEvent,
  NoChild,
  NodeId,
  Prevent,
  Realization,
  Scene,
  Shortcut,
  View,
  Wraps,
};

/// Keyboard shortcuts over the one view they serve.
///
/// A shortcut pressed while focus is inside the view runs the action bound to it, and
/// the platform's own response to the key and every shortcut an ancestor binds are
/// passed over. Holds [`NoChild`] until [`set_child`](Self::set_child) supplies the view
/// served; [`serve`](Self::serve) binds the shortcuts on a node that already exists
/// instead.
#[must_use]
pub struct Keymap<P: Realization, C = NoChild> {
  bindings: Vec<(Shortcut, Rc<dyn Fn()>)>,
  child: C,
  platform: PhantomData<P>,
}

impl<P: Realization, C> std::fmt::Debug for Keymap<P, C> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Keymap").finish_non_exhaustive()
  }
}

impl<P: Realization> Keymap<P> {
  /// A keymap binding no shortcut yet.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      bindings: Vec::new(),
      child: NoChild,
      platform: PhantomData,
    }
  }

  /// Sets the view the keymap serves.
  pub fn set_child(self, child: impl View<P>) -> Keymap<P, AnyView<P>> {
    Keymap {
      bindings: self.bindings,
      child: child.into_any(),
      platform: PhantomData,
    }
  }

  /// Binds the shortcuts on `node`, a view that already exists; returns `node`.
  pub fn serve(self, node: NodeId) -> NodeId {
    let bindings = self.bindings;

    if !bindings.is_empty() {
      Scene::<P>::with(|scene| {
        scene.handle(
          node,
          Handler::KeyDown(Rc::new(move |event: &KeyEvent| {
            bindings
              .iter()
              .find(|(shortcut, _)| event.matches(shortcut))
              .map_or(Prevent::None, |(_, action)| {
                action();

                Prevent::All
              })
          })),
        );
      });
    }

    node
  }
}

impl<P: Realization> Default for Keymap<P> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<P: Realization> Keymap<P> {
  /// Runs `action` when `shortcut` is pressed while focus is inside the view served.
  pub fn handle_shortcut(mut self, shortcut: Shortcut, action: impl Fn() + 'static) -> Self {
    self.bindings.push((shortcut, Rc::new(action)));

    self
  }
}

impl<P: Realization, C> AnyKind for Keymap<P, C> {}

impl<P: Realization> View<P> for Keymap<P, AnyView<P>> {
  fn build(self) -> NodeId {
    let node = self.child.build();

    Keymap::<P> {
      bindings: self.bindings,
      child: NoChild,
      platform: PhantomData,
    }
    .serve(node)
  }
}

/// What a facade's [`Keymap`] carries: its constructor, the node it serves and its shortcuts.
pub trait KeymapBase<P: Realization>: Wraps<Inner = Keymap<P>> {
  /// A keymap binding no shortcut yet.
  #[must_use]
  fn new() -> Self {
    Self::wrap(Keymap::new())
  }

  /// Binds the shortcuts on `node`, a view that already exists; returns `node`.
  fn serve(self, node: NodeId) -> NodeId {
    self.into_inner().serve(node)
  }

  /// Runs `action` when `shortcut` is pressed while focus is inside the view served.
  #[must_use]
  fn handle_shortcut(self, shortcut: Shortcut, action: impl Fn() + 'static) -> Self {
    Self::wrap(self.into_inner().handle_shortcut(shortcut, action))
  }
}
