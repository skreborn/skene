use std::{any::Any, cell::RefCell, fmt, marker::PhantomData, rc::Rc};

use crate::{
  context::Context,
  effect::{Effect, UpdaterEffect},
  id::Id,
  memo::Memo,
  runtime::{RUNTIME, Runtime},
  signal::{ReadSignal, RwSignal, SignalState, SignalValue, Subscribers, WriteSignal},
  storage::{SyncStorage, UnsyncStorage},
  trigger::Trigger,
};

/// Controls a signal's lifetime explicitly, apart from the implicit scope tree.
///
/// Every signal belongs to a `Scope`, created explicitly or implicitly. Disposing
/// a `Scope` cleans up every signal under it and all of its child scopes.
#[derive(Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct Scope(pub(crate) Id, pub(crate) PhantomData<*const ()>);

impl Default for Scope {
  fn default() -> Self {
    Self::new()
  }
}

impl fmt::Debug for Scope {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut r#struct = formatter.debug_struct("Scope");

    r#struct.field("id", &self.0);

    r#struct.finish()
  }
}

impl Scope {
  /// Creates a new `Scope` that isn't a child or parent of any scope.
  #[inline(always)]
  pub fn new() -> Self {
    Self(Id::next(), PhantomData)
  }

  /// Returns the current `Scope` in the `Runtime`. Any signal, `Effect`, or `Memo`
  /// created with an implicit scope is placed under this `Scope`.
  #[inline(always)]
  pub fn current() -> Scope {
    RUNTIME.with(|runtime| Scope(*runtime.current_scope.borrow(), PhantomData))
  }

  /// Creates a child `Scope` of this `Scope`.
  pub fn create_child(&self) -> Scope {
    let child = Id::next();

    RUNTIME.with(|runtime| {
      runtime
        .children
        .borrow_mut()
        .entry(self.0)
        .or_default()
        .insert(child);

      runtime.parents.borrow_mut().insert(child, self.0);
    });

    Scope(child, PhantomData)
  }

  /// Reparents this scope under another scope.
  ///
  /// If this scope already has a parent, it is removed from that parent first.
  ///
  /// # Example
  /// ```rust
  /// # use skene_reactive::Scope;
  /// let parent = Scope::new();
  /// let child = Scope::new(); // Initially has no parent
  ///
  /// child.set_parent(parent);
  /// // Now child is a child of parent, and will be disposed when parent is disposed
  /// ```
  pub fn set_parent(&self, new_parent: Scope) {
    RUNTIME.with(|runtime| {
      if let Some(old_parent) = runtime.parents.borrow_mut().remove(&self.0)
        && let Some(children) = runtime.children.borrow_mut().get_mut(&old_parent)
      {
        children.remove(&self.0);
      }

      runtime
        .children
        .borrow_mut()
        .entry(new_parent.0)
        .or_default()
        .insert(self.0);

      runtime.parents.borrow_mut().insert(self.0, new_parent.0);
    });
  }

  /// Returns the parent scope of this scope, if any.
  #[inline(always)]
  pub fn parent(&self) -> Option<Scope> {
    RUNTIME.with(|runtime| {
      runtime
        .parents
        .borrow()
        .get(&self.0)
        .map(|id| Scope(*id, PhantomData))
    })
  }

  /// Creates a new signal under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_signal<T>(self, value: T) -> (ReadSignal<T>, WriteSignal<T>)
  where
    T: Any + 'static,
  {
    self.enter(|| RwSignal::new_split(value))
  }

  /// Creates a `RwSignal` under this `Scope` (local/unsync by default).
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_rw_signal<T>(self, value: T) -> RwSignal<T>
  where
    T: Any + 'static,
  {
    self.enter(|| RwSignal::new(value))
  }

  /// Creates a sync signal under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_sync_signal<T>(
    self,
    value: T,
  ) -> (ReadSignal<T, SyncStorage>, WriteSignal<T, SyncStorage>)
  where
    T: Any + Send + Sync + 'static,
  {
    self.enter(|| RwSignal::<T, SyncStorage>::new_sync_split(value))
  }

  /// Creates a sync `RwSignal` under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_sync_rw_signal<T>(self, value: T) -> RwSignal<T, SyncStorage>
  where
    T: Any + Send + Sync + 'static,
  {
    self.enter(|| RwSignal::<T, SyncStorage>::new_sync(value))
  }

  /// Creates a local (unsync) signal under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_local_signal<T>(
    self,
    value: T,
  ) -> (ReadSignal<T, UnsyncStorage>, WriteSignal<T, UnsyncStorage>)
  where
    T: Any + 'static,
  {
    self.enter(|| RwSignal::new_split(value))
  }

  /// Creates a local (unsync) `RwSignal` under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_local_rw_signal<T>(self, value: T) -> RwSignal<T, UnsyncStorage>
  where
    T: Any + 'static,
  {
    self.enter(|| RwSignal::new(value))
  }

  /// Creates a `Memo` under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_memo<T>(self, f: impl Fn(Option<&T>) -> T + 'static) -> Memo<T>
  where
    T: PartialEq + 'static,
  {
    self.enter(|| Memo::new(f))
  }

  /// Creates a `Trigger` under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_trigger(self) -> Trigger {
    self.enter(Trigger::new)
  }

  /// Creates an effect under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_effect<T>(self, f: impl Fn(Option<T>) -> T + 'static)
  where
    T: Any + 'static,
  {
    self.enter(|| Effect::new(f))
  }

  /// Creates an updater under this `Scope`.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn create_updater<R>(
    self,
    compute: impl Fn() -> R + 'static,
    on_change: impl Fn(R) + 'static,
  ) -> R
  where
    R: 'static,
  {
    self.enter(|| UpdaterEffect::new(compute, on_change))
  }

  /// Stores a context value in this scope.
  ///
  /// The stored context value can be retrieved by this scope and any of its
  /// descendants using [`Scope::get_context`] or [`Context::get`](crate::Context::get).
  /// Child scopes can provide their own values of the same type, which will
  /// shadow the parent's value for that subtree.
  ///
  /// `Context` values are automatically cleaned up when the scope is disposed.
  ///
  /// # Example
  /// ```rust
  /// # use skene_reactive::Scope;
  /// let scope = Scope::new();
  /// scope.provide_context(42i32);
  /// scope.enter(|| {
  ///   assert_eq!(scope.get_context::<i32>(), Some(42));
  /// });
  /// ```
  pub fn provide_context<T>(&self, value: T)
  where
    T: Clone + 'static,
  {
    self.enter(|| Context::provide(value))
  }

  /// Tries to retrieve a stored context value from this scope or its ancestors.
  ///
  /// `Context` lookup walks up the scope tree from this scope to find the
  /// nearest ancestor that provides a value of the requested type.
  ///
  /// Note: This method must be called while the scope is entered (i.e., inside
  /// [`Scope::enter`]) for the lookup to work correctly.
  ///
  /// # Example
  /// ```rust
  /// # use skene_reactive::Scope;
  /// let parent = Scope::new();
  /// parent.provide_context(42i32);
  ///
  /// let child = parent.create_child();
  /// child.enter(|| {
  ///   // Child sees parent's context
  ///   assert_eq!(child.get_context::<i32>(), Some(42));
  /// });
  /// ```
  pub fn get_context<T>(&self) -> Option<T>
  where
    T: Clone + 'static,
  {
    self.enter(Context::get::<T>)
  }

  /// Runs the given closure within this scope.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn enter<T>(&self, f: impl FnOnce() -> T) -> T
  where
    T: 'static,
  {
    Runtime::assert_ui_thread();

    let prev_scope = RUNTIME.with(|runtime| {
      let mut current_scope = runtime.current_scope.borrow_mut();
      let prev_scope = *current_scope;
      *current_scope = self.0;
      prev_scope
    });

    let result = f();

    RUNTIME.with(|runtime| {
      *runtime.current_scope.borrow_mut() = prev_scope;
    });

    result
  }

  /// Wraps a closure so it runs under a new child scope of this scope.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn enter_child<T, U>(self, f: impl Fn(T) -> U + 'static) -> impl Fn(T) -> (U, Scope) + 'static
  where
    T: 'static,
  {
    Runtime::assert_ui_thread();
    let parent = self;

    move |t| {
      let scope = parent.create_child();

      let prev_scope = RUNTIME.with(|runtime| {
        let mut current_scope = runtime.current_scope.borrow_mut();
        let prev_scope = *current_scope;
        *current_scope = scope.0;
        prev_scope
      });

      let result = f(t);

      RUNTIME.with(|runtime| {
        *runtime.current_scope.borrow_mut() = prev_scope;
      });

      (result, scope)
    }
  }

  /// Subscribes the running effect to this scope, binding the effect's lifetime to it.
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn track(&self) {
    Runtime::assert_ui_thread();

    let signal = if let Some(signal) = self.0.signal() {
      signal
    }
    else {
      let signal = SignalState::from_parts(
        self.0,
        SignalValue::Local(Rc::new(RefCell::new(()))),
        Subscribers::local(),
      );

      self.0.add_signal(signal.clone());

      signal
    };

    signal.subscribe();
  }

  /// Runs `f` when this scope is disposed.
  ///
  /// The hook for anything the runtime cannot clean up itself — cancelling a spawned task,
  /// clearing a timer, closing a subscription — so it ends with the scope that started it
  /// rather than outliving the view. Callbacks run after the scope's children are disposed,
  /// in reverse registration order.
  ///
  /// # Panics
  ///
  /// Panics if called off the UI thread.
  pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
    Runtime::assert_ui_thread();

    RUNTIME.with(|runtime| {
      runtime
        .cleanups
        .borrow_mut()
        .entry(self.0)
        .or_default()
        .push(Box::new(f));
    });
  }

  /// Disposes this scope, cleaning up all of its signals and child scopes.
  pub fn dispose(&self) {
    self.0.dispose();
  }
}
