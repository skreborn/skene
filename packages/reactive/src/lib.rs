//! # Reactive engine
//!
//! Fine-grained reactivity: [`Signal`]s hold state, [`Memo`]s derive from it,
//! [`Effect`]s react to it, and [`Owner`]s tie every entity's lifetime to the subtree
//! that created it. Reads inside an effect subscribe it; writes re-run exactly the
//! subscribed effects, batched by [`batch`] and silenced by [`untrack`].
//!
//! Local entities live in generational arenas, one set per thread, joined by pooled
//! dependency edges; thread-safe signals ([`SyncSignal`]) live in a process-wide
//! registry, and an off-thread write routes its effects to the designated UI thread
//! ([`Runtime::init_on_ui_thread`]).

mod access;
mod arena;
mod computation;
mod context;
mod dispose;
mod effect;
mod graph;
mod guards;
mod id;
mod link;
mod memo;
mod ops;
mod owned;
mod owner;
mod runtime;
mod selector;
mod signal;
mod slice;
#[cfg(feature = "snapshot")]
mod snapshot;
mod storage;
mod store;
mod sync;
mod trigger;
mod value;

pub use access::{Get, Read, Set, Track, Update, With, Write};
pub use context::{provide_context, use_context};
pub use effect::{Effect, EffectHandle, Reaction, batch, untrack};
pub use guards::{ReadGuard, WriteGuard};
pub use id::Id as ReactiveId;
pub use memo::Memo;
pub use owned::{OwnedSignal, SyncOwnedSignal};
pub use owner::{Owner, on_cleanup};
pub use runtime::Runtime;
pub use selector::Selector;
pub use signal::{
  ReadSignal,
  Signal,
  SyncReadSignal,
  SyncSignal,
  SyncWriteSignal,
  WriteSignal,
  signal,
  sync_signal,
};
pub use skene_macros::Store;
pub use slice::{Slice, SyncSlice};
#[cfg(feature = "snapshot")]
pub use snapshot::{provide, restore, snapshot, staged};
pub use storage::{LocalStorage, SyncStorage};
pub use store::{
  AtIndex,
  AtKey,
  IndexedField,
  KeyedCollection,
  KeyedField,
  PathSegment,
  Store,
  StoreCore,
  StoreField,
  StorePath,
  Subfield,
};
pub use trigger::Trigger;
