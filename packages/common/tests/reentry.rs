//! A platform that reaches the scene from inside its own mutations: every platform call a
//! view makes runs with the scene released, so a platform whose response is
//! synchronous (a `focus` that fires a focus event, a removal that fires a blur)
//! cannot find the scene borrowed.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  thread::LocalKey,
};

use skene_common::{Column, Container, FocusHandle, RawCheckbox, Text, Theme, Toggle};
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
  static NEXT: Cell<u32> = const { Cell::new(1) };
  static CALLS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
  static MEDIA: RefCell<Option<(Owner, MediaSignals<f32>)>> = const { RefCell::new(None) };
}

/// Re-enters the scene, which panics if the caller still holds it, then logs `call`.
fn reenter(call: &'static str, sink: u32) {
  let entered = Scene::<Reentrant>::with(|_| {
    assert!(Scene::<Reentrant>::is_borrowed());

    sink > 0
  });

  assert!(entered);
  CALLS.with(|calls| calls.borrow_mut().push(call));
}

fn mint() -> u32 {
  let sink = NEXT.get();

  NEXT.set(sink + 1);

  sink
}

fn calls() -> Vec<&'static str> {
  CALLS.with(|calls| calls.borrow().clone())
}

impl Platform for Reentrant {
  type Scalar = f32;
  type Sink = u32;
  type StyleExt = ();
  type TokenValue = Erased;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(_: &BoxProps<Self>) -> u32 {
    mint()
  }

  fn update_box(sink: u32, _: Slot, _: &StaticLeaf<BoxValue<f32>>) {
    reenter("update_box", sink);
  }

  fn update_focus_order(sink: u32, _: Option<FocusOrder>) {
    reenter("update_focus_order", sink);
  }

  fn update_access(sink: u32, _: &[AccessProp]) {
    reenter("update_access", sink);
  }

  fn create_control(_: &ControlProps<Self>) -> u32 {
    mint()
  }

  fn set_control_state(sink: u32, _: &ControlState) {
    reenter("set_control_state", sink);
  }

  fn create_overlay(_: &OverlayProps<Self>) -> u32 {
    mint()
  }

  fn set_overlay_open(sink: u32, _: bool, _: Option<u32>) {
    reenter("set_overlay_open", sink);
  }

  fn set_overlay_placement(sink: u32, _: Placement) {
    reenter("set_overlay_placement", sink);
  }

  fn create_run(_: &RunProps<Self>) -> u32 {
    mint()
  }

  fn update_run(sink: u32, _: Slot, _: &StaticLeaf<RunValue>) {
    reenter("update_run", sink);
  }

  fn set_text(sink: u32, _: &str) {
    reenter("set_text", sink);
  }

  fn schedule(_: std::time::Duration, _: Box<dyn FnOnce()>) -> Timer {
    Timer(0)
  }

  fn cancel(_: Timer) {}

  fn bind_node(sink: u32, _: NodeId) {
    reenter("bind_node", sink);
  }

  fn create_anchor() -> u32 {
    let sink = mint();

    reenter("create_anchor", sink);

    sink
  }

  fn create_scope() -> u32 {
    mint()
  }

  fn attach(parent: u32, _: u32, _: Option<u32>) {
    reenter("attach", parent);
  }

  fn detach(parent: u32, _: u32) {
    reenter("detach", parent);
  }

  fn destroy(sink: u32) {
    reenter("destroy", sink);
  }

  fn define_tokens(scope: u32, _: &Definitions<Erased>) {
    reenter("define_tokens", scope);
  }

  fn define_root_tokens(_: &Definitions<Erased>) {
    reenter("define_root_tokens", 1);
  }

  fn listen(sink: u32, _: EventKind) {
    reenter("listen", sink);
  }

  fn focus(sink: u32) {
    reenter("focus", sink);
  }

  fn blur(sink: u32) {
    reenter("blur", sink);
  }

  fn set_value(sink: u32, _: &str) {
    reenter("set_value", sink);
  }

  fn set_selection(sink: u32, _: TextSelection) {
    reenter("set_selection", sink);
  }

  fn set_checked(sink: u32, _: bool) {
    reenter("set_checked", sink);
  }

  fn observe_size(sink: u32, _: Rc<dyn Fn(Size<f32>)>) -> Observation {
    reenter("observe_size", sink);

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
    reenter("definitions", 1);

    vec![self.card.define()]
  }
}

/// Focus, blur, a rewritten text, a redefined palette, a mirrored control state, the
/// tree mutations, a node's binding, an anchor, a listener and a token set's definitions
/// all reach the platform with the scene released.
#[test]
fn every_mirrored_call_runs_with_the_scene_released() {
  let owner = Owner::new();
  let palette = Signal::opaque(Palette::default());
  let label = Signal::new("one".to_owned());
  let checked = Signal::new(false);
  let focus = FocusHandle::<Reentrant>::new();
  let root = owner.enter(|| {
    let focus = focus.clone();

    Theme::<Reentrant, _>::derive(move || palette.get())
      .set_child_builder(move || {
        Container::<Reentrant>::new().bind_focus(&focus).set_child(
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
    "bind_node",
    "create_anchor",
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
