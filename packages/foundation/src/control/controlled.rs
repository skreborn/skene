//! A behavior's controlled value: read through what its user derives, and changed only
//! by asking the handler its user installs.

use std::rc::Rc;

use skene_reactive::{Get, Set, Signal};

/// A behavior's controlled value: read through a closure its user derives, changed only
/// by the handler its user installs.
///
/// A bound signal is the truth, and a change its handler rejects never shows. With
/// nothing derived the value reads as `None`; with no handler a change asked for
/// changes nothing. Cloning shares the closures.
#[must_use]
pub struct Controlled<T: 'static> {
  read: Option<Rc<dyn Fn() -> T>>,
  handle: Option<Rc<dyn Fn(T)>>,
}

impl<T: 'static> Clone for Controlled<T> {
  #[inline(always)]
  fn clone(&self) -> Self {
    Self {
      read: self.read.clone(),
      handle: self.handle.clone(),
    }
  }
}

impl<T: 'static> Default for Controlled<T> {
  #[inline(always)]
  fn default() -> Self {
    Self::new()
  }
}

impl<T: 'static> std::fmt::Debug for Controlled<T> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.debug_struct("Controlled").finish_non_exhaustive()
  }
}

impl<T: 'static> Controlled<T> {
  /// A value with nothing derived and no handler.
  #[inline(always)]
  pub const fn new() -> Self {
    Self {
      read: None,
      handle: None,
    }
  }

  /// Fixes the value at `value`.
  pub fn set(&mut self, value: T)
  where
    T: Clone,
  {
    self.read = Some(Rc::new(move || value.clone()));
  }

  /// Reads the value through `read`, tracking the signals it reads.
  pub fn derive(&mut self, read: impl Fn() -> T + 'static) {
    self.read = Some(Rc::new(read));
  }

  /// Asks `handle` for every change.
  pub fn handle(&mut self, handle: impl Fn(T) + 'static) {
    self.handle = Some(Rc::new(handle));
  }

  /// Reads the value from `signal` and asks it for every change.
  #[inline(always)]
  pub fn bind(&mut self, signal: Signal<T>)
  where
    T: Clone,
  {
    self.derive(move || signal.get());
    self.handle(move |value| signal.set(value));
  }

  /// The value as it reads now, tracking what it reads; `None` with nothing derived.
  #[inline(always)]
  #[must_use]
  pub fn read(&self) -> Option<T> {
    self.read.as_ref().map(|read| read())
  }

  /// Asks for `value`.
  #[inline(always)]
  pub fn ask(&self, value: T) {
    if let Some(handle) = &self.handle {
      handle(value);
    }
  }

  /// Whether a handler is installed.
  #[inline(always)]
  #[must_use]
  pub const fn is_handled(&self) -> bool {
    self.handle.is_some()
  }

  /// The closure the value is read through, for a platform realizing it; `None` with
  /// nothing derived.
  #[inline(always)]
  #[must_use]
  pub fn reader(&self) -> Option<Rc<dyn Fn() -> T>> {
    self.read.clone()
  }
}
