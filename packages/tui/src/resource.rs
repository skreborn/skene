//! Asynchronously loaded data, readable as a signal.

use std::{future::Future, pin::Pin, rc::Rc};

use skene_reactive::{Get, Set, Signal, With};

use crate::task::spawn;

/// A resource's loader, boxed so it can be stored and re-run on [`reload`](Resource::reload).
type Loader<T> = Rc<dyn Fn() -> Pin<Box<dyn Future<Output = T>>>>;

/// Data loaded by a task, `None` until it lands.
///
/// The load starts on creation and is bound to the current scope, so it ends with the view
/// that asked for it. Reading tracks, so a view that reads a resource re-runs when the value
/// arrives—pair it with a [`DerivedView`](crate::DerivedView) to swap a placeholder for the
/// loaded tree, or wrap the subtree in a [`Suspense`](crate::Suspense).
///
/// The loader is retained, so [`reload`](Self::reload) can re-run it. Failure is the loaded
/// type's business, not the resource's: load a `Result` and match on it.
#[must_use]
pub struct Resource<T: 'static> {
  value: Signal<Option<T>>,
  loader: Signal<Loader<T>>,
}

impl<T: 'static> Clone for Resource<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}

impl<T: 'static> Copy for Resource<T> {}

impl<T: 'static> Resource<T> {
  /// A resource with its value unloaded and `loader` retained for [`load`](Self::load).
  fn with_loader(loader: Loader<T>) -> Self {
    Self {
      value: Signal::new(None),
      loader: Signal::new(loader),
    }
  }

  /// Spawns the loader into the value signal, scope-bound like any task.
  fn load(&self) {
    let value = self.value;
    let future = (self.loader.get())();

    spawn(async move { value.set(Some(future.await)) });
  }

  /// Starts loading, returning a resource that is `None` until `load` completes.
  pub fn new<F>(load: impl Fn() -> F + 'static) -> Self
  where
    F: Future<Output = T> + 'static,
  {
    let resource = Self::with_loader(Rc::new(move || Box::pin(load())));

    resource.load();

    resource
  }

  /// Re-runs the loader, clearing the value to `None` while it reloads.
  pub fn reload(&self) {
    self.value.set(None);

    self.load();
  }

  /// The loaded value, or `None` while it is still loading. Tracks.
  #[must_use]
  pub fn get(&self) -> Option<T>
  where
    T: Clone,
  {
    self.value.get()
  }

  /// Runs `apply` on the loaded value, or on `None` while it is still loading. Tracks, and
  /// needs no [`Clone`].
  pub fn with<R>(&self, apply: impl FnOnce(Option<&T>) -> R) -> R {
    self.value.with(|value| apply(value.as_ref()))
  }

  /// Whether the value has not landed yet. Tracks.
  #[must_use]
  pub fn is_loading(&self) -> bool {
    self.value.with(Option::is_none)
  }
}
