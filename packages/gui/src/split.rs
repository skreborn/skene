//! The runtime half of [`macro@lazy`](crate::lazy): awaiting split-off wasm modules.
//!
//! A `#[lazy]` function's expansion declares a thread-local [`SplitLoader`] over the
//! module's wasm loader import and awaits [`SplitLoader::ensure`] before calling into
//! the split-off body. The loader import, the module files, and the fetch glue exist
//! when `skene build web` splits the app (`[web] split = true` in `skene.toml`); an
//! unsplit build resolves the loader import to a no-op and the body stays in place.

use std::{
  cell::RefCell,
  ffi::c_void,
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll, Waker},
};

/// A split module's wasm loader import: called once with the completion callback and
/// the callback's payload.
pub type LoadSplit = unsafe extern "C" fn(unsafe extern "C" fn(*const c_void, bool), *const c_void);

/// A split module's load state, declared thread-local by a `#[lazy]` expansion.
pub struct SplitLoader {
  inner: Rc<Inner>,
}

struct Inner {
  state: RefCell<State>,
}

enum State {
  Deferred(LoadSplit),
  Pending(Vec<Waker>),
  Loaded(bool),
}

impl SplitLoader {
  /// Creates the loader over a split module's wasm loader import.
  ///
  /// # Safety
  ///
  /// `load` must be the module's `__wasm_split_load_*` import, as a `#[lazy]`
  /// expansion binds it.
  #[inline(always)]
  pub unsafe fn new(load: LoadSplit) -> Self {
    Self {
      inner: Rc::new(Inner {
        state: RefCell::new(State::Deferred(load)),
      }),
    }
  }

  /// Resolves `true` once the module is loaded, starting the fetch on the first call.
  ///
  /// A failed fetch never resolves: the module may still arrive when the network
  /// does, and the enclosing boundary keeps showing its fallback until then.
  pub fn ensure(loader: &'static std::thread::LocalKey<Self>) -> impl Future<Output = bool> {
    Ensure {
      inner: loader.with(|loader| Rc::clone(&loader.inner)),
    }
  }
}

/// The future behind [`SplitLoader::ensure`].
struct Ensure {
  inner: Rc<Inner>,
}

impl Future for Ensure {
  type Output = bool;

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<bool> {
    let mut state = self.inner.state.borrow_mut();

    match &mut *state {
      State::Loaded(success) => Poll::Ready(*success),
      State::Pending(wakers) => {
        wakers.push(context.waker().clone());

        Poll::Pending
      }
      State::Deferred(load) => {
        let load = *load;

        *state = State::Pending(vec![context.waker().clone()]);
        drop(state);

        let payload = Rc::into_raw(Rc::clone(&self.inner)).cast::<c_void>();

        unsafe { load(loaded, payload) };

        Poll::Pending
      }
    }
  }
}

/// The completion callback the loader import calls back with its payload.
unsafe extern "C" fn loaded(payload: *const c_void, success: bool) {
  let inner = unsafe { Rc::from_raw(payload.cast::<Inner>()) };
  let previous = std::mem::replace(&mut *inner.state.borrow_mut(), State::Loaded(success));

  if let State::Pending(wakers) = previous {
    for waker in wakers {
      waker.wake();
    }
  }
}
