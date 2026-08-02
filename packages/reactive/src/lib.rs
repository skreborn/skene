//! # Reactive engine

mod base;
mod context;
mod derived;
mod effect;
mod id;
mod impls;
mod memo;
mod read;
mod runtime;
mod scope;
mod signal;
#[cfg(feature = "snapshot")]
mod snapshot;
mod storage;
mod store;
mod sync_runtime;
mod trigger;
mod write;

pub use base::{BaseSignal, SyncBaseSignal};
pub use context::Context;
pub use derived::{DerivedRwSignal, SyncDerivedRwSignal};
pub use effect::{Effect, EffectPriority, EffectTrait, SignalTracker, UpdaterEffect};
pub use id::Id as ReactiveId;
pub use memo::Memo;
pub use read::{ReadRef, SignalGet, SignalRead, SignalTrack, SignalWith};
pub use runtime::Runtime;
pub use scope::Scope;
pub use signal::{
  ReadSignal,
  RwSignal,
  SyncReadSignal,
  SyncRwSignal,
  SyncWriteSignal,
  WriteSignal,
};
pub use skene_macros::Store;
#[cfg(feature = "snapshot")]
pub use snapshot::{restore, snapshot};
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
pub use write::{SignalUpdate, SignalWrite, WriteRef};
