//! The hydration state channel shared by the web leaves.
//!
//! The server drains each registered source into per-resource blobs—an array in
//! creation order for the resources built outside any lazy segment, one such array per
//! lazy segment, and tag-keyed entries—and the client adopts each value by the same
//! segment and order, or by tag. A lazy segment's content builds when its module
//! arrives, in no fixed order and after the resources built beside it, so it keys its
//! own resources: by the segments enclosing it, each by its build order within the
//! segment holding it, which both leaves count alike.

#[cfg(not(skene_web_platform = "headless"))]
use std::cell::RefCell;
#[cfg(skene_web_platform = "browser")]
use std::collections::VecDeque;

#[cfg(not(skene_web_platform = "headless"))]
use skene_foundation::web::HydrationSource;

/// The id of the `<script>` element carrying the hydration state, shared by the server's
/// page assembly and the client's `launch`.
pub const STATE_SCRIPT_ID: &str = "skene-state";

/// A lazy segment's place among the hydrating resources.
///
/// The segments enclosing it, outermost first, each by its build order within the
/// segment holding it; the resources its content builds are matched across the boundary
/// under this key, in their creation order. Taken with [`enter_lazy`] when the segment
/// is built and applied with [`within_lazy`] when its content is.
#[cfg(not(skene_web_platform = "headless"))]
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub struct LazyRegion(Vec<u32>);

/// A lazy segment's path with its order-keyed entries, in creation order.
#[cfg(not(skene_web_platform = "headless"))]
type LazyEntries<V> = (Vec<u32>, V);

/// The state as it crosses the channel: the order-keyed entries outside any lazy
/// segment, the tag-keyed ones, then each lazy segment's order-keyed entries.
#[cfg(not(skene_web_platform = "headless"))]
type State = (
  Vec<Option<String>>,
  Vec<(String, String)>,
  Vec<LazyEntries<Vec<Option<String>>>>,
);

/// Encodes the settled resource state: one entry per order-keyed resource in creation
/// order (`null` for one that did not settle), then the tag-keyed entries sorted by
/// tag, then each lazy segment's entries in the segment's own creation order, the
/// segments in the order their content built.
#[cfg(skene_web_platform = "server")]
fn encode_state(state: &State) -> String {
  serde_json::to_string(state).unwrap_or_else(|_| "[[],[],[]]".to_owned())
}

/// Decodes the state produced by [`encode_state`]. Malformed state adopts nothing rather
/// than panicking.
#[cfg(skene_web_platform = "browser")]
fn decode_state(state: &str) -> State {
  serde_json::from_str(state).unwrap_or_default()
}

#[cfg(not(skene_web_platform = "headless"))]
thread_local! {
  /// The lazy segment whose content is building, as its path; empty outside any.
  static REGION: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
  /// How many lazy segments each building segment on the path has built so far, the
  /// outermost first: the next one's index within it. Empty until the first is built.
  static LAZY_COUNTS: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
}

#[cfg(skene_web_platform = "browser")]
thread_local! {
  /// The server-rendered values outside any lazy segment, to adopt in creation order.
  static ADOPTED: RefCell<VecDeque<Option<String>>> = const { RefCell::new(VecDeque::new()) };
  /// Each lazy segment's server-rendered values, to adopt in the segment's own
  /// creation order.
  static ADOPTED_LAZY: RefCell<Vec<LazyEntries<VecDeque<Option<String>>>>> =
    const { RefCell::new(Vec::new()) };
  /// The server-rendered tag-keyed values to adopt during hydration.
  static TAGGED: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

#[cfg(skene_web_platform = "server")]
thread_local! {
  /// Each critical resource's serializer with the lazy segment it was created in, in
  /// creation order; drained once the render settles.
  static SOURCES: RefCell<Vec<(Vec<u32>, HydrationSource)>> = const { RefCell::new(Vec::new()) };
  /// Each tag-keyed resource's serializer; drained with [`SOURCES`], sorted by tag.
  static TAGGED_SOURCES: RefCell<Vec<(String, HydrationSource)>> =
    const { RefCell::new(Vec::new()) };
}

/// Takes the place of the lazy segment being built: the next index within the segment
/// whose content is building, under that segment's own place.
///
/// Called once per lazy segment, when it is built, on both leaves; the content applies
/// the result through [`within_lazy`].
#[cfg(not(skene_web_platform = "headless"))]
pub(crate) fn enter_lazy() -> LazyRegion {
  let index = LAZY_COUNTS.with(|counts| {
    let mut counts = counts.borrow_mut();

    let Some(count) = counts.last_mut()
    else {
      counts.push(1);

      return 0;
    };
    let index = *count;

    *count += 1;

    index
  });
  let mut path = REGION.with(|region| region.borrow().clone());

  path.push(index);

  LazyRegion(path)
}

/// Runs `work` as the content of the lazy segment at `region`.
///
/// The resources it creates register or adopt under that segment. A poll of the
/// content's future is run this way, so a resource created while it loads belongs to
/// the segment too.
#[cfg(not(skene_web_platform = "headless"))]
pub(crate) fn within_lazy<R>(region: &LazyRegion, work: impl FnOnce() -> R) -> R {
  let previous = REGION.with(|current| current.replace(region.0.clone()));

  LAZY_COUNTS.with(|counts| counts.borrow_mut().push(0));

  let result = work();

  LAZY_COUNTS.with(|counts| counts.borrow_mut().pop());
  REGION.with(|current| *current.borrow_mut() = previous);

  result
}

/// Forgets the segment being built and every count, so a render or a hydration starts
/// outside any lazy segment.
#[cfg(not(skene_web_platform = "headless"))]
fn reset_regions() {
  REGION.with(|region| region.borrow_mut().clear());
  LAZY_COUNTS.with(|counts| counts.borrow_mut().clear());
}

/// Takes the next resource's adopted server value in the segment being built, if
/// hydrating left one.
#[cfg(skene_web_platform = "browser")]
#[must_use]
pub(crate) fn adopt_next() -> Option<String> {
  REGION.with(|region| {
    let region = region.borrow();

    if region.is_empty() {
      return ADOPTED.with(|adopted| adopted.borrow_mut().pop_front().flatten());
    }

    ADOPTED_LAZY.with(|adopted| {
      let mut adopted = adopted.borrow_mut();
      let (_, values) = adopted.iter_mut().find(|(path, _)| *path == *region)?;

      values.pop_front().flatten()
    })
  })
}

/// The server never adopts: it produces the state the client adopts.
#[cfg(skene_web_platform = "server")]
#[inline(always)]
#[must_use]
#[expect(
  clippy::missing_const_for_fn,
  reason = "the browser's twin is not const, and one function has one constness"
)]
pub(crate) fn adopt_next() -> Option<String> {
  None
}

/// Takes the adopted server value registered under `tag`, if hydrating left one.
#[cfg(skene_web_platform = "browser")]
#[must_use]
pub(crate) fn adopt_tagged(tag: &str) -> Option<String> {
  TAGGED.with(|tagged| {
    let mut tagged = tagged.borrow_mut();
    let index = tagged.iter().position(|(existing, _)| existing == tag)?;

    Some(tagged.remove(index).1)
  })
}

/// The server never adopts: it produces the state the client adopts.
#[cfg(skene_web_platform = "server")]
#[inline(always)]
#[must_use]
#[expect(
  clippy::missing_const_for_fn,
  reason = "the browser's twin is not const, and one function has one constness"
)]
pub(crate) fn adopt_tagged(_tag: &str) -> Option<String> {
  None
}

/// A no-op on the client: it adopts server state rather than producing it.
#[cfg(skene_web_platform = "browser")]
#[inline(always)]
pub(crate) fn register_source(_source: HydrationSource) {}

/// Registers a critical resource's serializer under the segment being built, to be
/// drained once the render settles.
#[cfg(skene_web_platform = "server")]
pub(crate) fn register_source(source: HydrationSource) {
  let region = REGION.with(|region| region.borrow().clone());

  SOURCES.with(|sources| sources.borrow_mut().push((region, source)));
}

/// A no-op on the client: it adopts server state rather than producing it.
#[cfg(skene_web_platform = "browser")]
#[inline(always)]
pub(crate) fn register_tagged_source(_tag: &str, _source: HydrationSource) {}

/// Registers a tag-keyed resource's serializer, to be drained once the render settles.
///
/// # Panics
///
/// Panics when `tag` is already registered in this render: tags pair one server value
/// with one client resource, so a duplicate has no well-defined owner.
#[cfg(skene_web_platform = "server")]
pub(crate) fn register_tagged_source(tag: &str, source: HydrationSource) {
  TAGGED_SOURCES.with(|sources| {
    let mut sources = sources.borrow_mut();

    assert!(
      sources.iter().all(|(existing, _)| existing != tag),
      "duplicate resource tag: {tag}",
    );

    sources.push((tag.to_owned(), source));
  });
}

/// Stages the server's serialized resource `state` for the resources built next to
/// adopt, by segment and creation order and by tag.
#[cfg(skene_web_platform = "browser")]
pub(crate) fn stage(state: &str) {
  let (ordered, tagged, lazy) = decode_state(state);

  reset_regions();
  ADOPTED.with(|adopted| *adopted.borrow_mut() = ordered.into());
  ADOPTED_LAZY.with(|adopted| {
    *adopted.borrow_mut() = lazy
      .into_iter()
      .map(|(path, values)| (path, values.into()))
      .collect();
  });
  TAGGED.with(|adopted| *adopted.borrow_mut() = tagged);
}

/// Ends hydration: the order-keyed values no resource adopted are dropped, so a
/// resource built after the served tree loads rather than taking a value served for
/// another.
#[cfg(skene_web_platform = "browser")]
pub(crate) fn finish() {
  ADOPTED.with(|adopted| adopted.borrow_mut().clear());
}

/// Forgets every registered resource serializer, so a render starts clean.
#[cfg(skene_web_platform = "server")]
pub(crate) fn reset_sources() {
  reset_regions();
  SOURCES.with(|sources| sources.borrow_mut().clear());
  TAGGED_SOURCES.with(|sources| sources.borrow_mut().clear());
}

/// Drains the registered sources into the encoded hydration state.
#[cfg(skene_web_platform = "server")]
pub(crate) fn collect_state() -> String {
  let mut ordered = Vec::new();
  let mut lazy: Vec<LazyEntries<Vec<Option<String>>>> = Vec::new();

  SOURCES.with(|sources| {
    for (region, source) in sources.borrow().iter() {
      let value = source();

      if region.is_empty() {
        ordered.push(value);

        continue;
      }

      match lazy.iter_mut().find(|(path, _)| path == region) {
        Some((_, values)) => values.push(value),
        None => lazy.push((region.clone(), vec![value])),
      }
    }
  });

  let mut tagged: Vec<(String, String)> = TAGGED_SOURCES.with(|sources| {
    sources
      .borrow()
      .iter()
      .filter_map(|(tag, source)| source().map(|value| (tag.clone(), value)))
      .collect()
  });

  tagged.sort();

  encode_state(&(ordered, tagged, lazy))
}
