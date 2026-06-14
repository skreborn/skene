//! The process-wide (thread-local) glue between the core and a [`Backend`].
//!
//! The core is single-threaded (the reactive runtime is too), so the active
//! backend and the event-handler registry live in thread-locals. Views reach
//! the backend through [`with_backend`]; the backend reaches handlers through
//! the [`EventSink`] installed in [`set_backend`].

use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;
use skene_reactive::{Runtime, Scope};
use skene_renderer::{Backend, Event, EventKind, NodeId};

pub(crate) type Handler = Rc<dyn Fn(Event)>;

thread_local! {
    static BACKEND: RefCell<Option<Rc<RefCell<dyn Backend>>>> = const { RefCell::new(None) };
    static HANDLERS: RefCell<FxHashMap<(NodeId, EventKind), Vec<Handler>>> =
        RefCell::new(FxHashMap::default());
    static ROOT_SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
}

/// Install the active backend and wire its event sink to the handler registry.
///
/// This also initializes the reactive runtime on the current (UI) thread.
pub fn set_backend(backend: Rc<RefCell<dyn Backend>>) {
  Runtime::init_on_ui_thread();

  let sink: skene_renderer::EventSink = Rc::new(|node: NodeId, event: Event| {
    let kind = event.kind();
    let handlers = HANDLERS.with(|h| h.borrow().get(&(node, kind)).cloned());
    if let Some(handlers) = handlers {
      for handler in handlers {
        handler(event.clone());
      }
    }
  });
  backend.borrow_mut().set_event_sink(sink);

  BACKEND.with(|b| *b.borrow_mut() = Some(backend));
}

/// Run a closure with mutable access to the active backend.
///
/// Panics if no backend has been installed.
pub fn with_backend<R>(f: impl FnOnce(&mut dyn Backend) -> R) -> R {
  BACKEND.with(|b| {
    let backend = b.borrow();
    let backend = backend
      .as_ref()
      .expect("no backend installed; call skene_dom::launch (or set_backend) first");
    let mut backend = backend.borrow_mut();
    f(&mut *backend)
  })
}

/// Register an event handler for `(node, kind)` and subscribe the node to that
/// event category on the backend.
pub fn register_handler(node: NodeId, kind: EventKind, handler: Handler) {
  HANDLERS.with(|h| {
    h.borrow_mut()
      .entry((node, kind))
      .or_default()
      .push(handler);
  });
  with_backend(|b| b.listen(node, kind));
}

/// The application root scope. All top-level signals/effects live under it, so
/// it (and everything it owns) survives for the lifetime of the app.
pub fn root_scope() -> Scope {
  ROOT_SCOPE.with(|s| {
    let mut s = s.borrow_mut();
    if let Some(scope) = *s {
      scope
    }
    else {
      let scope = Scope::new();
      *s = Some(scope);
      scope
    }
  })
}
