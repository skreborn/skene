// The derived variant is only constructible and mountable on a target with a platform; the
// type itself is always present, since it is a field of the shared `Container` builder.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::rc::Rc;

use skene_core::{AnyView, IntoChildren, IntoView, NodeId};

/// A container's children.
#[must_use]
pub struct Children(Inner);

enum Inner {
  Static(Vec<AnyView>),
  Derived(Box<dyn FnOnce(NodeId, Prepare)>),
}

type Prepare = Rc<dyn Fn(NodeId) -> NodeId>;

impl Default for Children {
  #[inline(always)]
  fn default() -> Self {
    Children(Inner::Static(Vec::new()))
  }
}

impl Children {
  /// Creates a static set of children.
  #[inline(always)]
  pub fn new(children: impl IntoChildren) -> Self {
    Children(Inner::Static(children.into_children()))
  }

  /// Creates a derived set of children.
  ///
  /// `items` yields the items, `key` gives each a stable identity, `view` renders one.
  #[cfg(target_arch = "wasm32")]
  #[inline(always)]
  pub fn derive<T, I, K, V>(
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + std::hash::Hash + 'static,
    V: skene_core::View,
  {
    Children(Inner::Derived(Box::new(move |parent, prepare| {
      crate::reconcile(parent, items, key, move |item| prepare(view(item).build()));
    })))
  }

  /// Mounts these children under `parent`.
  #[cfg(target_arch = "wasm32")]
  pub fn mount(self, parent: NodeId) {
    self.mount_with(parent, std::convert::identity);
  }

  /// Mounts these children under `parent`.
  ///
  /// Each built child node is passed through `prepare` before insertion.
  ///
  /// Reconciliation tracks the node `prepare` returns.
  #[cfg(target_arch = "wasm32")]
  pub fn mount_with(self, parent: NodeId, prepare: impl Fn(NodeId) -> NodeId + 'static) {
    use skene_core::View as _;

    let prepare: Prepare = Rc::new(prepare);

    match self.0 {
      Inner::Static(views) => {
        for view in views {
          let node = prepare(view.build());

          crate::driver::attach(parent, node, None);
        }
      }
      Inner::Derived(install) => install(parent, prepare),
    }
  }
}

/// A container that has children.
pub trait HasChildren: Sized {
  /// Sets the children.
  ///
  /// This is the only method a trait implementor needs to provide.
  #[must_use]
  fn set_children_to(self, children: Children) -> Self;

  /// Sets a single static child.
  #[inline(always)]
  #[must_use]
  fn set_child(self, child: impl IntoView) -> Self {
    self.set_children_to(Children::new(vec![child.into_view()]))
  }

  /// Sets a static set of children.
  #[inline(always)]
  #[must_use]
  fn set_children(self, children: impl IntoChildren) -> Self {
    self.set_children_to(Children::new(children))
  }

  /// Sets a single derived child.
  #[cfg(target_arch = "wasm32")]
  #[inline(always)]
  #[must_use]
  fn derive_child<V: skene_core::View>(self, child: impl Fn() -> V + 'static) -> Self {
    self.set_children_to(Children::derive(|| vec![()], |()| (), move |()| child()))
  }

  /// Sets a derived set of children.
  ///
  /// `items` yields the items, `key` gives each a stable identity, `view` renders one.
  #[cfg(target_arch = "wasm32")]
  #[inline(always)]
  #[must_use]
  fn derive_children<T, I, K, V>(
    self,
    items: impl Fn() -> I + 'static,
    key: impl Fn(&T) -> K + 'static,
    view: impl Fn(T) -> V + 'static,
  ) -> Self
  where
    I: IntoIterator<Item = T>,
    K: Eq + std::hash::Hash + 'static,
    V: skene_core::View,
  {
    self.set_children_to(Children::derive(items, key, view))
  }
}
