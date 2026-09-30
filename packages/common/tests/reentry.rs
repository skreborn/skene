//! A platform that reaches the scene from inside its own mutations: every platform call a
//! view makes runs with the scene released, so a platform whose response is
//! synchronous (a `focus` that fires a focus event, a removal that fires a blur)
//! cannot find the scene borrowed.

use std::{cell::RefCell, rc::Rc, thread::LocalKey};

use skene_common::{Column, Container, NodeHandle, RawCheckbox, Text, Theme, Toggle};
use skene_core::{
  AccessProp,
  BoxProps,
  BoxValue,
  Color,
  ColorScheme,
  ContainerSizes,
  ControlProps,
  ControlState,
  Definition,
  Definitions,
  Dimension,
  EventKind,
  FocusOrder,
  HasChild,
  HasChildren,
  Inset,
  Length,
  Media,
  MediaSignals,
  NodeId,
  Observation,
  OverlayProps,
  Placement,
  Platform,
  PointerPrecision,
  RunProps,
  RunValue,
  Scene,
  Size,
  Slot,
  StaticLeaf,
  TextSelection,
  Timer,
  Token,
  TokenName,
  TokenSet,
  View,
};
use skene_reactive::{Get, Memo, Owner, Set, Signal};

/// The platform under test: every mirrored call re-enters the scene before recording
/// what it was asked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Reentrant;

#[derive(Debug, Clone, PartialEq)]
enum Erased {
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

thread_local! {
  static SCENE: RefCell<Scene<Reentrant>> = RefCell::new(Scene::new());
  static CALLS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
  static MEDIA: RefCell<Option<(Owner, MediaSignals<f32>)>> = const { RefCell::new(None) };
}

/// Re-enters the scene, which panics if the caller still holds it, then logs `call`.
fn reenter(call: &'static str) {
  Scene::<Reentrant>::with(|_| assert!(Scene::<Reentrant>::is_borrowed()));
  CALLS.with(|calls| calls.borrow_mut().push(call));
}

fn calls() -> Vec<&'static str> {
  CALLS.with(|calls| calls.borrow().clone())
}

impl Platform for Reentrant {
  type Scalar = f32;
  type StyleExt = ();
  type TokenValue = Erased;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(_: NodeId, _: &BoxProps<Self>) {
    reenter("create_box");
  }

  fn update_box(_: NodeId, _: Slot, _: &StaticLeaf<BoxValue<f32>>) {
    reenter("update_box");
  }

  fn update_focus_order(_: NodeId, _: Option<FocusOrder>) {
    reenter("update_focus_order");
  }

  fn update_access(_: NodeId, _: &[AccessProp]) {
    reenter("update_access");
  }

  fn create_control(_: NodeId, _: &ControlProps<Self>) {
    reenter("create_control");
  }

  fn set_control_state(_: NodeId, _: &ControlState) {
    reenter("set_control_state");
  }

  fn create_overlay(_: NodeId, _: &OverlayProps<Self>) {
    reenter("create_overlay");
  }

  fn set_overlay_open(_: NodeId, _: bool, _: Option<NodeId>) {
    reenter("set_overlay_open");
  }

  fn set_overlay_placement(_: NodeId, _: Placement) {
    reenter("set_overlay_placement");
  }

  fn create_run(_: NodeId, _: &RunProps<Self>) {
    reenter("create_run");
  }

  fn update_run(_: NodeId, _: Slot, _: &StaticLeaf<RunValue>) {
    reenter("update_run");
  }

  fn set_text(_: NodeId, _: &str) {
    reenter("set_text");
  }

  fn schedule(_: std::time::Duration, _: Box<dyn FnOnce()>) -> Timer {
    Timer(0)
  }

  fn cancel(_: Timer) {}

  fn create_anchor(_: NodeId) {
    reenter("create_anchor");
  }

  fn create_scope(_: NodeId) {
    reenter("create_scope");
  }

  fn attach(_: NodeId, _: NodeId, _: Option<NodeId>) {
    reenter("attach");
  }

  fn detach(_: NodeId, _: NodeId) {
    reenter("detach");
  }

  fn destroy(_: NodeId) {
    reenter("destroy");
  }

  fn define_tokens(_: NodeId, _: &Definitions<Erased>) {
    reenter("define_tokens");
  }

  fn define_root_tokens(_: &Definitions<Erased>) {
    reenter("define_root_tokens");
  }

  fn listen(_: NodeId, _: EventKind) {
    reenter("listen");
  }

  fn focus(_: NodeId) {
    reenter("focus");
  }

  fn blur(_: NodeId) {
    reenter("blur");
  }

  fn set_value(_: NodeId, _: &str) {
    reenter("set_value");
  }

  fn set_selection(_: NodeId, _: TextSelection) {
    reenter("set_selection");
  }

  fn set_checked(_: NodeId, _: bool) {
    reenter("set_checked");
  }

  fn observe_size(_: NodeId, _: Rc<dyn Fn(Size<f32>)>) -> Observation {
    reenter("observe_size");

    Observation::unmeasured()
  }

  fn media() -> MediaSignals<f32> {
    MEDIA.with(|media| {
      media
        .borrow_mut()
        .get_or_insert_with(|| {
          let owner = Owner::new();
          let signals = owner.enter(|| MediaSignals {
            viewport: Signal::opaque(Size::new(1024., 768.)).read_only(),
            scheme: Signal::opaque(ColorScheme::Light).read_only(),
            reduced_motion: Signal::opaque(false).read_only(),
            reduced_transparency: Signal::opaque(false).read_only(),
            high_contrast: Signal::opaque(false).read_only(),
            pointer: Signal::opaque(PointerPrecision::Fine).read_only(),
            hover: Signal::opaque(true).read_only(),
            safe_area: Signal::opaque(Inset::ZERO).read_only(),
            container_sizes: ContainerSizes::new(),
          });

          (owner, signals)
        })
        .1
    })
  }

  fn watch_media(media: &Media<f32>) -> Memo<bool> {
    Self::media().watch(media)
  }

  fn set_scheme(_scheme: Option<skene_core::ColorScheme>) {}

  fn stored_scheme() -> Option<skene_core::ColorScheme> {
    None
  }
}

/// A palette of one token, so a theme has something to define.
#[derive(Debug, Clone, PartialEq)]
struct Palette {
  card: Token<Color, f32>,
}

impl Default for Palette {
  fn default() -> Self {
    Self {
      card: Token::new("card", Color::srgb(1., 1., 1.)),
    }
  }
}

impl TokenSet<Erased> for Palette {
  fn definitions(&self) -> Vec<(TokenName, Erased)> {
    reenter("definitions");

    vec![self.card.define()]
  }
}

/// Focus, blur, a rewritten text, a redefined palette, a mirrored control state, the
/// tree mutations, each node's realization, a listener and a token set's definitions
/// all reach the platform with the scene released.
#[test]
fn every_mirrored_call_runs_with_the_scene_released() {
  let owner = Owner::new();
  let palette = Signal::opaque(Palette::default());
  let label = Signal::new("one".to_owned());
  let checked = Signal::new(false);
  let focus = NodeHandle::<Reentrant>::new();
  let root = owner.enter(|| {
    Theme::<Reentrant, _>::derive(move || palette.get())
      .set_child_builder(move || {
        Container::<Reentrant>::new().bind_handle(focus).set_child(
          Column::<Reentrant>::new().set_children((
            Text::<Reentrant>::derive(move || label.get()),
            RawCheckbox::<Reentrant>::new(Toggle::new().bind_checked(checked)),
          )),
        )
      })
      .build()
  });

  for call in [
    "attach",
    "create_anchor",
    "create_box",
    "create_control",
    "create_run",
    "create_scope",
    "define_tokens",
    "definitions",
    "listen",
    "set_text",
    "set_control_state",
  ] {
    assert!(calls().contains(&call), "{call}");
  }

  CALLS.with(|calls| calls.borrow_mut().clear());

  focus.focus();
  focus.blur();
  label.set("two".to_owned());
  checked.set(true);
  palette.set(Palette {
    card: Token::new("card", Color::srgb(0., 0., 0.)),
  });

  assert_eq!(
    calls(),
    [
      "focus",
      "blur",
      "set_text",
      "set_control_state",
      "update_access",
      "definitions",
      "define_tokens"
    ]
  );

  Scene::<Reentrant>::with(|scene| scene.destroy(root));

  assert!(calls().contains(&"destroy"));
  assert!(!Scene::<Reentrant>::is_borrowed());

  owner.dispose();
}
