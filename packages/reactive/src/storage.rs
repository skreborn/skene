use std::{any::Any, marker::PhantomData, rc::Rc};

use crate::id::Id;

/// Marker for a thread-local signal, stored in the current thread's runtime.
pub struct UnsyncStorage(PhantomData<Rc<()>>);
/// Marker for a thread-safe signal, stored in the global sync runtime.
pub struct SyncStorage;

/// Internal abstraction over how signals are stored.
pub(crate) trait Storage<T: Any + 'static> {
  fn create(value: T) -> Id;
}
