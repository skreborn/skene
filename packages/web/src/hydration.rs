//! The hydration state channel shared by the web leaves.
//!
//! The server drains each registered source into a JSON array of per-resource blobs, in
//! creation order; the client decodes it and adopts each value in the same order.

/// A critical resource's serializer, registered with the driver so a server render can
/// drain it into the hydration state once it settles.
pub type HydrationSource = Box<dyn Fn() -> Option<String>>;

/// The id of the `<script>` element carrying the hydration state, shared by the server's
/// page assembly and the client's `hydrate_page`.
pub const STATE_SCRIPT_ID: &str = "skene-state";

/// Encodes each resource's settled state, in creation order—`null` for one that did not
/// settle.
#[cfg(target_os = "wasi")]
pub(crate) fn encode_state(values: &[Option<String>]) -> String {
  serde_json::to_string(values).unwrap_or_else(|_| "[]".to_owned())
}

/// Decodes the state produced by [`encode_state`] back into one entry per resource, in
/// creation order. Malformed state adopts nothing rather than panicking.
#[cfg(target_os = "unknown")]
pub(crate) fn decode_state(state: &str) -> Vec<Option<String>> {
  serde_json::from_str(state).unwrap_or_default()
}
