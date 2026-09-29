//! The runtime half of [`macro@lazy`](crate::lazy): awaiting split-off wasm modules.
//!
//! A `#[lazy]` function's expansion declares a thread-local [`SplitLoader`] over the
//! module's wasm loader import and awaits [`SplitLoader::ensure`] before calling into
//! the split-off body. The loader import, the module files, and the fetch glue exist
//! when `skene build web` splits the app (`[web] split = true` in `skene.toml`); an
//! unsplit build resolves the loader import to a no-op and the body stays in place.

use std::{
  cell::{Cell, RefCell},
  ffi::c_void,
  future::Future,
  pin::Pin,
  rc::Rc,
  task::{Context, Poll, Waker},
  time::Duration,
};

use skene_core::Platform as _;

use crate::ActivePlatform;

/// A split module's wasm loader import: each call runs the completion callback exactly
/// once with the payload it was given and whether the module loaded.
pub type LoadSplit = unsafe extern "C" fn(unsafe extern "C" fn(*const c_void, bool), *const c_void);

/// The wait before the first repeated ask after a failed load, doubling with each
/// failure up to [`LONGEST_RETRY`].
const FIRST_RETRY: Duration = Duration::from_millis(250);

/// The longest wait between two asks for a module that keeps failing to load.
const LONGEST_RETRY: Duration = Duration::from_secs(30);

/// A split module's load state, declared thread-local by a `#[lazy]` expansion.
pub struct SplitLoader {
  inner: Rc<Inner>,
}

struct Inner {
  state: RefCell<State>,
  /// The loads that have failed so far.
  failures: Cell<u32>,
}

enum State {
  Deferred(LoadSplit),
  Pending(LoadSplit, Vec<Waker>),
  Loaded,
}

impl SplitLoader {
  /// Creates the loader over a split module's wasm loader import.
  ///
  /// # Safety
  ///
  /// `load` must be the module's `__wasm_split_load_*` import, as a `#[lazy]`
  /// expansion binds it, whose every call runs its callback exactly once with the
  /// payload it was given.
  #[inline(always)]
  pub unsafe fn new(load: LoadSplit) -> Self {
    Self {
      inner: Rc::new(Inner {
        state: RefCell::new(State::Deferred(load)),
        failures: Cell::new(0),
      }),
    }
  }

  /// Resolves once the module is loaded, starting the fetch on the first call.
  ///
  /// A failed load is asked for again after a wait that doubles with each failure, so
  /// the module arrives when the network does, the enclosing boundary showing its
  /// fallback until then.
  pub fn ensure(loader: &'static std::thread::LocalKey<Self>) -> impl Future<Output = ()> {
    Ensure {
      inner: loader.with(|loader| Rc::clone(&loader.inner)),
    }
  }
}

impl Inner {
  /// Asks the loader import for the module, handing it one strong count of `self` as
  /// the payload [`loaded`] takes back.
  fn ask(self: &Rc<Self>, load: LoadSplit) {
    let payload = Rc::into_raw(Rc::clone(self)).cast::<c_void>();

    // SAFETY: `load` is the module's loader import, per `SplitLoader::new`, which runs
    // `loaded` exactly once with `payload`, where the count it owns is taken back.
    unsafe { load(loaded, payload) };
  }
}

/// The future behind [`SplitLoader::ensure`].
struct Ensure {
  inner: Rc<Inner>,
}

impl Future for Ensure {
  type Output = ();

  fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
    let mut state = self.inner.state.borrow_mut();

    match &mut *state {
      State::Loaded => Poll::Ready(()),
      State::Pending(_, wakers) => {
        wakers.push(context.waker().clone());

        Poll::Pending
      }
      State::Deferred(load) => {
        let load = *load;

        *state = State::Pending(load, vec![context.waker().clone()]);
        drop(state);
        self.inner.ask(load);

        Poll::Pending
      }
    }
  }
}

/// The completion callback the loader import runs with its payload: a loaded module
/// wakes every waiter, and a failed load is asked for again after a wait.
unsafe extern "C" fn loaded(payload: *const c_void, success: bool) {
  // SAFETY: `payload` is the `Rc::into_raw` of an `Inner` that `Inner::ask` handed the
  // loader import, which runs this callback exactly once with it.
  let inner = unsafe { Rc::from_raw(payload.cast::<Inner>()) };

  if success {
    let previous = std::mem::replace(&mut *inner.state.borrow_mut(), State::Loaded);

    if let State::Pending(_, wakers) = previous {
      for waker in wakers {
        waker.wake();
      }
    }

    return;
  }

  let failures = inner.failures.get().saturating_add(1);
  let wait = FIRST_RETRY
    .saturating_mul(1 << failures.min(8).saturating_sub(1))
    .min(LONGEST_RETRY);

  inner.failures.set(failures);
  tracing::warn!("a split module failed to load; asking again in {wait:?}");

  let _ = ActivePlatform::default().schedule(
    wait,
    Box::new(move || {
      let load = match &*inner.state.borrow() {
        State::Pending(load, _) => *load,
        State::Deferred(_) | State::Loaded => return,
      };

      inner.ask(load);
    }),
  );
}
