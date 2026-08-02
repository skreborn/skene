//! The hydration state channel shared by the web leaves.
//!
//! The server drains each registered source into per-resource blobs—an array in
//! creation order plus tag-keyed entries—and the client adopts each value by the same
//! order or tag.

/// A critical resource's serializer, registered with the driver so a server render can
/// drain it into the hydration state once it settles.
pub type HydrationSource = Box<dyn Fn() -> Option<String>>;

/// The id of the `<script>` element carrying the hydration state, shared by the server's
/// page assembly and the client's `hydrate_page`.
pub const STATE_SCRIPT_ID: &str = "skene-state";

/// Encodes the settled resource state: one entry per order-keyed resource in creation
/// order (`null` for one that did not settle), then the tag-keyed entries sorted by
/// tag.
#[cfg(target_os = "wasi")]
pub(crate) fn encode_state(ordered: &[Option<String>], tagged: &[(String, String)]) -> String {
  serde_json::to_string(&(ordered, tagged)).unwrap_or_else(|_| "[[],[]]".to_owned())
}

/// Decodes the state produced by [`encode_state`] back into the order-keyed entries and
/// the tag-keyed ones. Malformed state adopts nothing rather than panicking.
#[cfg(target_os = "unknown")]
pub(crate) type DecodedState = (Vec<Option<String>>, Vec<(String, String)>);

#[cfg(target_os = "unknown")]
pub(crate) fn decode_state(state: &str) -> DecodedState {
  serde_json::from_str(state).unwrap_or_default()
}
