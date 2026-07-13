//! The platform layer: everything specific to what a compile target can actually do.
//!
//! Exactly one platform is compiled, selected by target-gating. There is no trait and no
//! runtime dispatch: shared code names a function like `platform::web::create_element`, and
//! the target decides which implementation that is. A target with no platform has no
//! realization, so building a built-in view there is a compile error rather than a runtime
//! failure.
//!
//! ## Families and leaves
//!
//! A **family** is the realization shared by platforms that emit the same semantics; a
//! **leaf** is one concrete platform, selected by `target_os`. The web family ([`web`])
//! contains its leaves: the browser (`web::csr`, `target_os = "unknown"`) and an eventual
//! server-side runner (`web::ssr`, `target_os = "wasi"`) — both emit web semantics, to a
//! live DOM versus an HTML string. The shared realization lives in the family and is written
//! against a leaf vocabulary, so adding a leaf is new files plus one `cfg` and the family
//! does not move. A native family (`native::{macos, linux}`, or `android`/`ios`) would slot
//! in the same way.
//!
//! Two seams sit at two levels. The **structural** seam every GUI platform provides —
//! `tree` (node attach/detach/destroy) and `drive` (run a future on the platform executor) —
//! the active family re-exports up to `platform::` here, so the facade's `reconcile`/`Children`
//! and the scope-bound `spawn` built on `drive` reach the active leaf whatever its family. The
//! **web** vocabulary (`create_element`, `set_style`, …) is web-specific and stays inside
//! [`web`], reached only by the web-family realization.

// The web family and its leaves.
#[cfg(target_arch = "wasm32")]
pub mod web;

/// A critical resource's serializer, registered with the active leaf so the server can drain it
/// into the hydration state once the render settles.
#[cfg(target_arch = "wasm32")]
pub(crate) type HydrationSource = Box<dyn Fn() -> Option<String>>;

// The structural seam, resolved through the active family to its active leaf. `adopt_next` and
// `register_source` are the hydration channel: on the server a resource registers its settled
// value for serialization, on the client it adopts the server's value instead of loading;
// `is_server` lets a client-only load skip the server round.
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{adopt_next, drive, is_server, register_source, tree};
