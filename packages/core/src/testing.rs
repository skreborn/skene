//! A platform for core's own tests: sinks are numbers, and every mirrored call is logged.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  thread::LocalKey,
};

use skene_reactive::{Memo, Signal};

use super::{
  AnyKind,
  BoxProps,
  BoxValue,
  Children,
  ContainerSizes,
  ControlProps,
  ControlState,
  Definition,
  Definitions,
  EventKind,
  HasChildren,
  Media,
  MediaSignals,
  NodeId,
  NodeKind,
  OverlayProps,
  PlainChild,
  Platform,
  RunProps,
  RunValue,
  Scene,
  Seal,
  Slot,
  StaticLeaf,
  Timer,
  View,
};
use crate::{
  AccessProp,
  Color,
  ColorScheme,
  Dimension,
  FocusOrder,
  Inset,
  Length,
  PointerEvent,
  PointerPrecision,
  Size,
  TextSelection,
};

/// One call the platform received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Call {
  Attach(u32, u32, Option<u32>),
  Detach(u32, u32),
  Destroy(u32),
  Listen(u32, EventKind),
  DefineTokens(u32, usize),
  DefineRootTokens(usize),
}

/// The erased definition the test medium accepts.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Erased {
  Color(Definition<Color, f32>),
  Number(Definition<f32, f32>),
  Length(Definition<Length, f32>),
  Dimension(Definition<Dimension<f32>, f32>),
}

impl From<Definition<Color, f32>> for Erased {
  fn from(definition: Definition<Color, f32>) -> Self {
    Self::Color(definition)
  }
}

impl From<Definition<f32, f32>> for Erased {
  fn from(definition: Definition<f32, f32>) -> Self {
    Self::Number(definition)
  }
}

impl From<Definition<Length, f32>> for Erased {
  fn from(definition: Definition<Length, f32>) -> Self {
    Self::Length(definition)
  }
}

impl From<Definition<Dimension<f32>, f32>> for Erased {
  fn from(definition: Definition<Dimension<f32>, f32>) -> Self {
    Self::Dimension(definition)
  }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct TestPlatform;

thread_local! {
  static SCENE: RefCell<Scene<TestPlatform>> = RefCell::new(Scene::new());
  static LOG: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
  static NEXT_SINK: Cell<u32> = const { Cell::new(1) };
  static MEDIA: RefCell<Option<MediaSignals<f32>>> = const { RefCell::new(None) };
  static ORIGINS: RefCell<Vec<(u32, f32, f32)>> = const { RefCell::new(Vec::new()) };
}

impl TestPlatform {
  /// Empties the scene and the log.
  pub(super) fn reset() {
    SCENE.with(|scene| *scene.borrow_mut() = Scene::new());
    LOG.with(|log| log.borrow_mut().clear());
    NEXT_SINK.with(|next| next.set(1));
    ORIGINS.with(|origins| origins.borrow_mut().clear());
  }

  /// Places `sink`'s box at (`x`, `y`), for pointer events to localize against.
  pub(super) fn set_origin(sink: u32, x: f32, y: f32) {
    ORIGINS.with(|origins| origins.borrow_mut().push((sink, x, y)));
  }

  /// The calls received since the last reset or take.
  pub(super) fn take_log() -> Vec<Call> {
    LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
  }

  fn log(call: Call) {
    LOG.with(|log| log.borrow_mut().push(call));
  }

  fn next_sink() -> u32 {
    NEXT_SINK.with(|next| {
      let sink = next.get();

      next.set(sink + 1);

      sink
    })
  }
}

impl Platform for TestPlatform {
  type Scalar = f32;
  type Sink = u32;
  type StyleExt = ();
  type TokenValue = Erased;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(&self, _: &BoxProps<Self>) -> u32 {
    Self::next_sink()
  }

  fn update_box(&self, _: &u32, _: Slot, _: &StaticLeaf<BoxValue<f32>>) {}

  fn update_focus_order(&self, _: &u32, _: Option<FocusOrder>) {}

  fn update_access(&self, _: &u32, _: &[AccessProp]) {}

  fn create_control(&self, _: &ControlProps<Self>) -> u32 {
    Self::next_sink()
  }

  fn set_control_state(&self, _: &u32, _: &ControlState) {}

  fn create_overlay(&self, _: &OverlayProps<Self>) -> u32 {
    Self::next_sink()
  }

  fn set_overlay_open(&self, _: &u32, _: bool, _: Option<&u32>) {}

  fn create_run(&self, _: &RunProps<Self>) -> u32 {
    Self::next_sink()
  }

  fn update_run(&self, _: &u32, _: Slot, _: &StaticLeaf<RunValue>) {}

  fn schedule(&self, _: std::time::Duration, _: Box<dyn FnOnce()>) -> Timer {
    Timer(0)
  }

  fn cancel(&self, _: Timer) {}

  fn set_text(&self, _: &u32, _: &str) {}

  fn create_anchor(&self) -> u32 {
    Self::next_sink()
  }

  fn create_scope(&self) -> u32 {
    Self::next_sink()
  }

  fn attach(&self, parent: &u32, child: &u32, before: Option<&u32>) {
    Self::log(Call::Attach(*parent, *child, before.copied()));
  }

  fn detach(&self, parent: &u32, child: &u32) {
    Self::log(Call::Detach(*parent, *child));
  }

  fn destroy(&self, sink: u32) {
    Self::log(Call::Destroy(sink));
  }

  fn define_tokens(&self, scope: &u32, definitions: &Definitions<Erased>) {
    Self::log(Call::DefineTokens(*scope, definitions.len()));
  }

  fn define_root_tokens(&self, definitions: &Definitions<Erased>) {
    Self::log(Call::DefineRootTokens(definitions.len()));
  }

  fn listen(&self, sink: &u32, kind: EventKind) {
    Self::log(Call::Listen(*sink, kind));
  }

  fn localize_pointer(&self, sink: &u32, event: &PointerEvent) -> PointerEvent {
    ORIGINS.with(|origins| {
      origins
        .borrow()
        .iter()
        .find(|(placed, _, _)| placed == sink)
        .map_or(*event, |(_, x, y)| PointerEvent {
          local_x: event.x - x,
          local_y: event.y - y,
          ..*event
        })
    })
  }

  fn focus(&self, _: &u32) {}

  fn blur(&self, _: &u32) {}

  fn set_value(&self, _: &u32, _: &str) {}

  fn set_selection(&self, _: &u32, _: TextSelection) {}

  fn set_checked(&self, _: &u32, _: bool) {}

  fn observe_size(&self, _: &u32, _: Rc<dyn Fn(Size<f32>)>) {}

  fn media(&self) -> MediaSignals<f32> {
    MEDIA.with(|media| {
      *media.borrow_mut().get_or_insert_with(|| MediaSignals {
        viewport: Signal::opaque(Size {
          width: 1024.,
          height: 768.,
        })
        .read_only(),
        scheme: Signal::opaque(ColorScheme::Light).read_only(),
        reduced_motion: Signal::opaque(false).read_only(),
        reduced_transparency: Signal::opaque(false).read_only(),
        high_contrast: Signal::opaque(false).read_only(),
        pointer: Signal::opaque(PointerPrecision::Fine).read_only(),
        hover: Signal::opaque(true).read_only(),
        safe_area: Signal::opaque(Inset::ZERO).read_only(),
        container_sizes: ContainerSizes::new(),
      })
    })
  }

  fn watch_media(&self, media: &Media<f32>) -> Memo<bool> {
    self.media().watch(media)
  }

  fn set_scheme(&self, _scheme: Option<ColorScheme>) {}

  fn stored_scheme(&self) -> Option<ColorScheme> {
    None
  }
}

/// A box with children and nothing else.
pub(super) struct TestBox {
  children: Children<TestPlatform, PlainChild>,
}

impl TestBox {
  pub(super) fn new() -> Self {
    Self {
      children: Children::new(),
    }
  }

  /// Mints a box node directly, outside any view, recording the pending annotation as a
  /// box builder does.
  pub(super) fn node() -> NodeId {
    Scene::<TestPlatform>::with(|scene| {
      let annotation = scene.take_pending_annotation();
      let node = scene.create(NodeKind::Box, TestPlatform::next_sink());

      if let Some(annotation) = annotation {
        scene.set_annotation(node, annotation);
      }

      node
    })
  }
}

impl HasChildren<TestPlatform> for TestBox {
  type Kind = PlainChild;

  fn children_mut(&mut self, _: Seal) -> &mut Children<TestPlatform, PlainChild> {
    &mut self.children
  }
}

impl AnyKind for TestBox {}

impl View<TestPlatform> for TestBox {
  fn build(self) -> NodeId {
    let node = Self::node();

    self.children.mount(node);

    node
  }
}

/// The sink of `node`.
pub(super) fn sink(node: NodeId) -> u32 {
  Scene::<TestPlatform>::with(|scene| *scene.record(node).expect("live node").sink())
}

/// The children of `node`, in order.
pub(super) fn children(node: NodeId) -> Vec<NodeId> {
  Scene::<TestPlatform>::with(|scene| scene.children(node).collect())
}

/// The kinds of the children of `node`, in order.
pub(super) fn child_kinds(node: NodeId) -> Vec<NodeKind> {
  Scene::<TestPlatform>::with(|scene| {
    scene
      .children(node)
      .map(|child| scene.record(child).expect("live child").kind())
      .collect()
  })
}
