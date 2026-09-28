//! Membership tracking over one key-valued source.

use std::hash::Hash;

use rustc_hash::FxHashMap;

use crate::{
  access::With,
  effect::{Effect, batch},
  owner::Owner,
  signal::{Signal, update_silently},
  trigger::Trigger,
};

/// Tracks which key a source currently yields, so a reader asking about one key
/// re-runs only when that key's membership flips.
///
/// One tracked run of the source per change fans out to at most two readers' worth
/// of triggers: the key left and the key entered.
#[must_use]
pub struct Selector<K: Eq + Hash + Clone + 'static> {
  state: Signal<SelectorState<K>>,
  owner: Owner,
}

impl<K: Eq + Hash + Clone + 'static> Copy for Selector<K> {}

#[expect(
  clippy::expl_impl_clone_on_copy,
  reason = "a derive would bound the key on Copy"
)]
impl<K: Eq + Hash + Clone + 'static> Clone for Selector<K> {
  fn clone(&self) -> Self {
    *self
  }
}

struct SelectorState<K> {
  current: Option<K>,
  keys: FxHashMap<K, Trigger>,
}

impl<K: Eq + Hash + Clone + 'static> Selector<K> {
  /// Creates a selector over `source`, owned by the current [`Owner`].
  #[cfg_attr(debug_assertions, track_caller)]
  pub fn new(source: impl Fn() -> K + 'static) -> Self {
    let owner = Owner::current();
    let state = Signal::opaque(SelectorState {
      current: None,
      keys: FxHashMap::default(),
    });

    Effect::new(move |_| {
      let next = source();
      let flipped = update_silently(state.id(), |state: &mut SelectorState<K>| {
        if state.current.as_ref() == Some(&next) {
          return None;
        }

        let previous = state.current.replace(next.clone());
        let left = previous.and_then(|previous| state.keys.get(&previous).copied());
        let entered = state.keys.get(&next).copied();

        Some((left, entered))
      })
      .flatten();

      if let Some((left, entered)) = flipped {
        batch(|| {
          if let Some(left) = left {
            left.notify();
          }

          if let Some(entered) = entered {
            entered.notify();
          }
        });
      }
    });

    Self { state, owner }
  }

  /// Whether the source currently yields `key`, subscribing the running computation
  /// to changes of exactly that answer.
  #[must_use]
  pub fn is(&self, key: &K) -> bool {
    let owner = self.owner;
    let trigger = update_silently(self.state.id(), |state: &mut SelectorState<K>| {
      if let Some(trigger) = state.keys.get(key) {
        return *trigger;
      }

      let trigger = owner.enter(Trigger::new);

      state.keys.insert(key.clone(), trigger);

      trigger
    });

    let Some(trigger) = trigger
    else {
      return false;
    };

    trigger.track();

    self
      .state
      .try_with_untracked(|state| state.current.as_ref() == Some(key))
      .unwrap_or(false)
  }
}
