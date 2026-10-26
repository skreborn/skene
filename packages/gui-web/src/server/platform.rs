//! The server leaf's platform: the portable vocabulary and the markup escape over the
//! string tree; handlers are inert, focus and sizes the client's, overlays closed, and
//! the native door adopts nothing.

use std::{cell::RefCell, rc::Rc, thread::LocalKey, time::Duration};

use skene_foundation::{
  AccessProp,
  ColorScheme,
  CompiledStyle,
  Content,
  ContentValue,
  ControlState,
  Definitions,
  EventKind,
  FocusOrder,
  Keep,
  Media,
  MediaSignals,
  NodeId,
  NodeSpec,
  Observation,
  PointerEvent,
  Realization,
  Scene,
  Size,
  Slot,
  SlotLeaf,
  Timer,
  TokenScope,
  web::{HydrationSource, Painter, Painting, WebHydration, WebStyle},
};
use skene_reactive::{Memo, ReadSignal};

use super::{
  SCENE,
  head::{persisted_scheme, set_scheme, set_window_title},
  media::{media_signals, with_media},
  with_realizer,
};
use crate::{LinkPolicy, NativeMarkup, Undrawn, Web, token::WebTokenValue};

impl Painting for Web {
  fn paint(_node: NodeId, _painter: Box<Painter>) {}

  fn available() -> ReadSignal<bool> {
    with_media(|media| media.absent)
  }
}

impl Realization for Web {
  const FONT_SIZE: f32 = 16.;
  const KEEP: Keep = Keep::Everything;

  type Scalar = f32;
  type StyleExt = WebStyle;
  type TokenValue = WebTokenValue;
  type Native = NativeMarkup;
  type Entry = Undrawn;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create(node: NodeId, spec: &NodeSpec) -> bool {
    with_realizer(|realizer| realizer.create(node, spec))
  }

  fn apply_style(node: NodeId, style: CompiledStyle<f32, WebStyle>) {
    with_realizer(|realizer| realizer.apply_style(node, &style));
  }

  fn apply_slot(node: NodeId, slot: Slot, leaf: &SlotLeaf<f32, WebStyle>) {
    with_realizer(|realizer| realizer.apply_slot(node, slot, leaf));
  }

  fn update_focus_order(node: NodeId, focus_order: Option<FocusOrder>) {
    with_realizer(|realizer| realizer.update_focus_order(node, focus_order));
  }

  fn update_access(node: NodeId, access: &[AccessProp]) {
    with_realizer(|realizer| realizer.update_access(node, access));
  }

  fn set_control_state(node: NodeId, state: &ControlState) {
    with_realizer(|realizer| realizer.set_control_state(node, *state));
  }

  fn set_overlay_open(_: NodeId, _: bool, _: Option<NodeId>) {}

  fn adopt(node: NodeId, native: NativeMarkup, spec: &NodeSpec) -> bool {
    with_realizer(|realizer| realizer.adopt_markup(node, &native, spec))
  }

  fn write(node: NodeId, value: ContentValue<'_>) {
    with_realizer(|realizer| realizer.write(node, value));
  }

  fn read<R>(
    node: NodeId,
    content: Content,
    read: impl FnOnce(ContentValue<'_>) -> R,
  ) -> Option<R> {
    with_realizer(|realizer| realizer.read(node, content, read))
  }

  fn composing(node: NodeId) -> bool {
    with_realizer(|realizer| realizer.composing(node))
  }

  fn attach(parent: NodeId, child: NodeId, _before: Option<NodeId>) {
    with_realizer(|realizer| {
      let _label = realizer.enter_caption(parent, child);
    });
  }

  fn relocate(parent: NodeId, child: NodeId, before: Option<NodeId>) {
    with_realizer(|realizer| {
      let _left = realizer.leave_caption(child);
    });
    Self::attach(parent, child, before);
  }

  fn hide(_parent: NodeId, child: NodeId) {
    with_realizer(|realizer| {
      let _left = realizer.leave_caption(child);
    });
  }

  fn remove(parent: NodeId, child: NodeId) {
    Self::hide(parent, child);
  }

  fn destroy(node: NodeId) {
    with_realizer(|realizer| {
      let _record = realizer.destroy(node);
    });
  }

  fn define_tokens(scope: TokenScope, definitions: &Definitions<WebTokenValue>) {
    with_realizer(|realizer| realizer.define_tokens(scope, definitions));
  }

  fn listen(_: NodeId, _: EventKind) {}

  /// The event unchanged: the server lays nothing out.
  fn localize_pointer(_: NodeId, event: &PointerEvent) -> PointerEvent {
    *event
  }

  fn focus(_: NodeId) {}

  fn blur(_: NodeId) {}

  fn observe_size(_: NodeId, _: Rc<dyn Fn(Size<f32>)>) -> Observation {
    Observation::unmeasured()
  }

  fn media() -> MediaSignals<f32> {
    media_signals()
  }

  fn watch_media(media: &Media<f32>) -> Memo<bool> {
    Self::media().watch(media)
  }

  fn hydration() -> Option<ReadSignal<bool>> {
    Some(with_media(|media| media.absent))
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
    set_scheme(scheme);
  }

  /// Nothing scheduled ever runs.
  fn schedule(_: Duration, _: Box<dyn FnOnce()>) -> Timer {
    Timer(0)
  }

  fn cancel(_: Timer) {}

  fn chosen_scheme() -> Option<ColorScheme> {
    persisted_scheme()
  }

  fn set_window_title(title: Option<&str>) {
    set_window_title(title);
  }

  fn current_path() -> Option<String> {
    Some(super::current_path())
  }

  /// A render has no view transitions: the update runs at once.
  fn commit_navigation(update: Box<dyn FnOnce()>) {
    update();
  }

  fn record_static_routes(routes: Vec<String>) {
    super::record_static_routes(routes);
  }
}

/// A server render carries the state and markup a client adopts, streaming around each
/// boundary, and adopts nothing itself.
impl WebHydration for Web {
  type LazyRegion = crate::resource_state::LazyRegion;

  fn install_executor() {
    super::install_executor();
  }

  fn is_server() -> bool {
    true
  }

  fn adopt_next() -> Option<String> {
    crate::resource_state::adopt_next()
  }

  fn adopt_tagged(tag: &str) -> Option<String> {
    crate::resource_state::adopt_tagged(tag)
  }

  fn register_source(source: HydrationSource) {
    crate::resource_state::register_source(source);
  }

  fn register_tagged_source(tag: &str, source: HydrationSource) {
    crate::resource_state::register_tagged_source(tag, source);
  }

  fn enter_lazy() -> Self::LazyRegion {
    crate::resource_state::enter_lazy()
  }

  fn within_lazy<R>(region: &Self::LazyRegion, work: impl FnOnce() -> R) -> R {
    crate::resource_state::within_lazy(region, work)
  }

  fn claim_lazy(_start: NodeId) -> bool {
    false
  }

  fn discard_lazy(_start: NodeId) {}

  fn register_boundary(start: NodeId, settled: Box<dyn Fn() -> bool>) {
    super::register_boundary(start, settled);
  }
}

impl Web {
  /// Opens the link run `node` as `policy` says, its destination's attributes rewritten
  /// now and at every later write.
  pub fn set_link_policy(node: NodeId, policy: LinkPolicy) {
    with_realizer(|realizer| realizer.set_link_policy(node, policy));
  }
}
