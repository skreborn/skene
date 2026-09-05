//! The hydration state channel shared by the web leaves.
//!
//! The server drains each registered source into per-resource blobs—an array in
//! creation order plus tag-keyed entries—and the client adopts each value by the same
//! order or tag.

#[cfg(target_os = "unknown")]
use std::cell::Cell;
#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

/// A critical resource's serializer, registered with the driver so a server render can
/// drain it into the hydration state once it settles.
pub type HydrationSource = Box<dyn Fn() -> Option<String>>;

/// The id of the `<script>` element carrying the hydration state, shared by the server's
/// page assembly and the client's `hydrate_page`.
pub const STATE_SCRIPT_ID: &str = "skene-state";

/// The state as it crosses the channel: the order-keyed entries, then the tag-keyed
/// ones.
#[cfg(target_os = "unknown")]
type State = (Vec<Option<String>>, Vec<(String, String)>);

/// Encodes the settled resource state: one entry per order-keyed resource in creation
/// order (`null` for one that did not settle), then the tag-keyed entries sorted by
/// tag.
#[cfg(target_os = "wasi")]
fn encode_state(ordered: &[Option<String>], tagged: &[(String, String)]) -> String {
  serde_json::to_string(&(ordered, tagged)).unwrap_or_else(|_| "[[],[]]".to_owned())
}

/// Decodes the state produced by [`encode_state`] back into the order-keyed entries and
/// the tag-keyed ones. Malformed state adopts nothing rather than panicking.
#[cfg(target_os = "unknown")]
fn decode_state(state: &str) -> State {
  serde_json::from_str(state).unwrap_or_default()
}

#[cfg(target_os = "unknown")]
thread_local! {
  /// The server-rendered resource values to adopt during hydration, in creation order.
  static ADOPTED: RefCell<Vec<Option<String>>> = const { RefCell::new(Vec::new()) };
  /// The next resource's creation-order slot, matched against [`ADOPTED`].
  static NEXT_SLOT: Cell<usize> = const { Cell::new(0) };
  /// The server-rendered tag-keyed values to adopt during hydration.
  static TAGGED: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

#[cfg(target_os = "wasi")]
thread_local! {
  /// Each critical resource's serializer, in creation order; drained once the render
  /// settles.
  static SOURCES: RefCell<Vec<HydrationSource>> = const { RefCell::new(Vec::new()) };
  /// Each tag-keyed resource's serializer; drained with [`SOURCES`], sorted by tag.
  static TAGGED_SOURCES: RefCell<Vec<(String, HydrationSource)>> =
    const { RefCell::new(Vec::new()) };
}

/// Advances to the next resource slot, returning its adopted server value if hydrating.
#[cfg(target_os = "unknown")]
#[must_use]
pub fn adopt_next() -> Option<String> {
  let slot = NEXT_SLOT.get();

  NEXT_SLOT.set(slot + 1);

  ADOPTED.with(|adopted| adopted.borrow().get(slot).cloned().flatten())
}

/// The server never adopts: it produces the state the client adopts.
#[cfg(target_os = "wasi")]
#[inline(always)]
#[must_use]
pub fn adopt_next() -> Option<String> {
  None
}

/// Takes the adopted server value registered under `tag`, if hydrating left one.
#[cfg(target_os = "unknown")]
#[must_use]
pub fn adopt_tagged(tag: &str) -> Option<String> {
  TAGGED.with(|tagged| {
    let mut tagged = tagged.borrow_mut();
    let index = tagged.iter().position(|(existing, _)| existing == tag)?;

    Some(tagged.remove(index).1)
  })
}

/// The server never adopts: it produces the state the client adopts.
#[cfg(target_os = "wasi")]
#[inline(always)]
#[must_use]
pub fn adopt_tagged(_tag: &str) -> Option<String> {
  None
}

/// A no-op on the client: it adopts server state rather than producing it.
#[cfg(target_os = "unknown")]
#[inline(always)]
pub fn register_source(_source: HydrationSource) {}

/// Registers a critical resource's serializer, to be drained once the render settles.
#[cfg(target_os = "wasi")]
pub fn register_source(source: HydrationSource) {
  SOURCES.with(|sources| sources.borrow_mut().push(source));
}

/// A no-op on the client: it adopts server state rather than producing it.
#[cfg(target_os = "unknown")]
#[inline(always)]
pub fn register_tagged_source(_tag: &str, _source: HydrationSource) {}

/// Registers a tag-keyed resource's serializer, to be drained once the render settles.
///
/// # Panics
///
/// Panics when `tag` is already registered in this render: tags pair one server value
/// with one client resource, so a duplicate has no well-defined owner.
#[cfg(target_os = "wasi")]
pub fn register_tagged_source(tag: &str, source: HydrationSource) {
  TAGGED_SOURCES.with(|sources| {
    let mut sources = sources.borrow_mut();

    assert!(
      sources.iter().all(|(existing, _)| existing != tag),
      "duplicate resource tag: {tag}",
    );

    sources.push((tag.to_owned(), source));
  });
}

/// The browser is a client, not the server.
#[cfg(target_os = "unknown")]
#[inline(always)]
#[must_use]
pub fn is_server() -> bool {
  false
}

/// This leaf renders on the server, so a client-only load skips its round here.
#[cfg(target_os = "wasi")]
#[inline(always)]
#[must_use]
pub fn is_server() -> bool {
  true
}

/// Stages the server's serialized resource `state` for the resources built next to
/// adopt, by creation order and by tag.
#[cfg(target_os = "unknown")]
pub(crate) fn stage(state: &str) {
  let (ordered, tagged) = decode_state(state);

  NEXT_SLOT.set(0);
  ADOPTED.with(|adopted| *adopted.borrow_mut() = ordered);
  TAGGED.with(|adopted| *adopted.borrow_mut() = tagged);
}

/// Forgets every registered resource serializer, so a render starts clean.
#[cfg(target_os = "wasi")]
pub(crate) fn reset_sources() {
  SOURCES.with(|sources| sources.borrow_mut().clear());
  TAGGED_SOURCES.with(|sources| sources.borrow_mut().clear());
}

/// Drains the registered sources into the encoded hydration state.
#[cfg(target_os = "wasi")]
pub(crate) fn collect_state() -> String {
  let values: Vec<Option<String>> =
    SOURCES.with(|sources| sources.borrow().iter().map(|source| source()).collect());
  let mut tagged: Vec<(String, String)> = TAGGED_SOURCES.with(|sources| {
    sources
      .borrow()
      .iter()
      .filter_map(|(tag, source)| source().map(|value| (tag.clone(), value)))
      .collect()
  });

  tagged.sort();

  encode_state(&values, &tagged)
}
