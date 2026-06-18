#[cfg(any(test, feature = "test"))]
use std::ops::Deref;
use std::{any::Any, cell::Cell, rc::Rc};

use skene_reactive::{Runtime, Scope};

use crate::{Backend, NodeId};

thread_local! {
  static BACKEND: Cell<Option<*const dyn Backend>> = const { Cell::new(None) };
  static ROOT_SCOPE: Cell<Option<Scope>> = const { Cell::new(None) };
}

/// Installs a backend for the current thread.
///
/// # Panics
///
/// Panics if a backend is already installed for this thread.
pub fn install_backend(backend: Rc<dyn Backend>) {
  Runtime::init_on_ui_thread();

  BACKEND.with(|slot| {
    slot.update(|slot| {
      assert!(
        slot.is_none(),
        "a backend is already installed for this thread",
      );

      Some(Rc::as_ptr(Box::leak(Box::new(backend))))
    });
  });
}

/// Runs a closure with access to the installed backend.
///
/// # Panics
///
/// Panics if no backend is installed for this thread.
#[inline(always)]
pub fn with_backend<R>(apply: impl FnOnce(&dyn Backend) -> R) -> R {
  let backend = BACKEND
    .with(Cell::get)
    .expect("no backend is installed for this thread");

  // SAFETY: the pointee is either a leaked instance that lives for the entire lifetime of the
  // process or one owned by the single `TestBackend` guard on the stack.
  apply(unsafe { &*backend })
}

/// Runs a closure with access to the installed backend as its concrete type.
///
/// Used to reach a backend's own typed API.
///
/// # Panics
///
/// Panics if no backend is installed for this thread, or if it is not of type `B`.
#[inline(always)]
pub fn with_backend_as<B: Backend, R>(apply: impl FnOnce(&B) -> R) -> R {
  with_backend(|backend| {
    apply(
      (backend as &dyn Any)
        .downcast_ref()
        .expect("the installed backend does not match the expected type"),
    )
  })
}

/// Returns the root scope for the current thread.
///
/// If there is no scope yet, a new one is created.
pub fn root_scope() -> Scope {
  ROOT_SCOPE.with(|current| {
    if let Some(scope) = current.get() {
      scope
    }
    else {
      let scope = Scope::new();

      current.set(Some(scope));

      scope
    }
  })
}

/// Builds `view` under the installed backend's root node.
///
/// Returns the built node's identifier.
pub fn mount(view: impl FnOnce() -> NodeId) -> NodeId {
  let scope = root_scope();

  scope.enter(|| {
    with_backend(|backend| {
      let node = view();

      backend.attach_child(backend.root(), node, None);

      node
    })
  })
}

/// An installed backend's guard.
///
/// The backend is uninstalled when this guard is dropped.
#[cfg(any(test, feature = "test"))]
#[must_use]
pub struct BackendGuard<B: Backend> {
  backend: Rc<B>,
}

#[cfg(any(test, feature = "test"))]
impl<B: Backend> Deref for BackendGuard<B> {
  type Target = B;

  fn deref(&self) -> &B {
    &self.backend
  }
}

#[cfg(any(test, feature = "test"))]
impl<B: Backend> Drop for BackendGuard<B> {
  fn drop(&mut self) {
    Runtime::reset();

    ROOT_SCOPE.with(|active_scope| active_scope.set(None));
    BACKEND.with(|slot| slot.set(None));
  }
}

/// Installs a test backend for the current thread.
///
/// Dropping the returned guard uninstalls the backend and resets the reactive runtime and root
/// scope.
///
/// # Panics
///
/// Panics if a backend is already installed for this thread.
#[cfg(any(test, feature = "test"))]
pub fn install_test_backend<B: Backend>(backend: B) -> BackendGuard<B> {
  let backend = Rc::new(backend);

  BACKEND.with(|slot| {
    slot.update(|slot| {
      assert!(
        slot.is_none(),
        "a backend is already installed for this thread",
      );

      Some(Rc::as_ptr(&backend) as *const dyn Backend)
    });
  });

  BackendGuard { backend }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, rc::Rc};

  use skene_reactive::RwSignal;

  use crate::{Backend, MockBackend, NodeId, install_test_backend};

  struct DropTracker {
    dropped: Rc<Cell<bool>>,
  }

  impl Drop for DropTracker {
    fn drop(&mut self) {
      self.dropped.set(true);
    }
  }

  impl Backend for DropTracker {
    fn root(&self) -> NodeId {
      NodeId::new(0)
    }

    fn create_group(&self) -> NodeId {
      NodeId::new(0)
    }

    fn attach_child(&self, _parent: NodeId, _child: NodeId, _before: Option<NodeId>) {}
    fn detach_child(&self, _parent: NodeId, _child: NodeId) {}

    fn destroy(&self, _node: NodeId) {}
  }

  #[test]
  #[should_panic = "a backend is already installed for this thread"]
  fn single_install() {
    let _first = install_test_backend(MockBackend::new());
    let _second = install_test_backend(MockBackend::new());
  }

  #[test]
  fn drop_guard() {
    let backend_uninstalled = Rc::new(Cell::new(false));
    let runtime_reset = Rc::new(Cell::new(false));

    {
      let _backend = install_test_backend(DropTracker {
        dropped: backend_uninstalled.clone(),
      });

      let _ = RwSignal::new(DropTracker {
        dropped: runtime_reset.clone(),
      });

      assert!(!backend_uninstalled.get());
      assert!(!runtime_reset.get());
    }

    assert!(backend_uninstalled.get());
    assert!(runtime_reset.get());
  }
}
