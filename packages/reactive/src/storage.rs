use std::{marker::PhantomData, rc::Rc};

/// Marker for a thread-local signal, stored in the creating thread's runtime.
pub struct LocalStorage(PhantomData<Rc<()>>);

/// Marker for a thread-safe signal, stored in the process-wide sync registry.
pub struct SyncStorage;
