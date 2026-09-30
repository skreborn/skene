//! The server leaf's platform: the portable vocabulary and the markup escape over the
//! string tree; handlers are inert, focus and sizes the client's, overlays closed.

use std::{cell::RefCell, rc::Rc, thread::LocalKey, time::Duration};

use skene_common::gui::{GuiStyle, Painter, Painting};
use skene_core::{
  AccessProp,
  BoxProps,
  BoxValue,
  ColorScheme,
  ControlProps,
  ControlState,
  Definitions,
  EventKind,
  FocusOrder,
  Media,
  MediaSignals,
  NodeId,
  Observation,
  OverlayProps,
  PickerOption,
  Platform,
  RunProps,
  RunValue,
  Scene,
  Size,
  Slot,
  StaticBoxStyle,
  StaticLeaf,
  TextSelection,
  Timer,
};
use skene_reactive::{Memo, ReadSignal};

use super::{
  SCENE,
  head::{set_scheme, stored_scheme},
  media::{media_signals, with_media},
  with_realizer,
};
use crate::{Declarations, Web, WebMarkup, realize::Realized, token::WebTokenValue};

impl Painting for Web {
  fn paint(_node: NodeId, _painter: Box<Painter>) {}

  fn available() -> ReadSignal<bool> {
    with_media(|media| media.absent)
  }
}

impl Platform for Web {
  type Scalar = f32;
  type StyleExt = GuiStyle;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create_box(node: NodeId, props: &BoxProps<Self>) {
    with_realizer(|realizer| realizer.create_box(node, props));
  }

  fn update_box(node: NodeId, slot: Slot, leaf: &StaticLeaf<BoxValue<f32, GuiStyle>>) {
    with_realizer(|realizer| realizer.update_box(node, slot, leaf));
  }

  fn update_focus_order(node: NodeId, focus_order: Option<FocusOrder>) {
    with_realizer(|realizer| realizer.update_focus_order(node, focus_order));
  }

  fn update_access(node: NodeId, access: &[AccessProp]) {
    with_realizer(|realizer| realizer.update_access(node, access));
  }

  fn create_control(node: NodeId, props: &ControlProps<Self>) {
    with_realizer(|realizer| realizer.create_control(node, props));
  }

  fn set_control_state(node: NodeId, state: &ControlState) {
    with_realizer(|realizer| realizer.set_control_state(node, *state));
  }

  fn create_overlay(node: NodeId, props: &OverlayProps<Self>) {
    with_realizer(|realizer| realizer.create_overlay(node, props));
  }

  fn set_overlay_open(_: NodeId, _: bool, _: Option<NodeId>) {}

  fn apply_layout(node: NodeId, layout: &StaticBoxStyle<Self>) {
    with_realizer(|realizer| realizer.apply_layout(node, layout));
  }

  fn create_run(node: NodeId, props: &RunProps<Self>) {
    with_realizer(|realizer| realizer.create_run(node, props));
  }

  fn update_run(node: NodeId, slot: Slot, leaf: &StaticLeaf<RunValue<GuiStyle>>) {
    with_realizer(|realizer| realizer.update_run(node, slot, leaf));
  }

  fn set_text(node: NodeId, text: &str) {
    with_realizer(|realizer| realizer.set_text(node, text));
  }

  fn create_anchor(node: NodeId) {
    with_realizer(|realizer| realizer.create_anchor(node));
  }

  fn create_scope(node: NodeId) {
    with_realizer(|realizer| realizer.create_scope(node));
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

  fn define_tokens(node: NodeId, definitions: &Definitions<WebTokenValue>) {
    with_realizer(|realizer| realizer.define_tokens(node, definitions));
  }

  fn define_root_tokens(definitions: &Definitions<WebTokenValue>) {
    with_realizer(|realizer| realizer.define_root_tokens(definitions));
  }

  fn listen(_: NodeId, _: EventKind) {}

  fn focus(_: NodeId) {}

  fn blur(_: NodeId) {}

  fn set_value(node: NodeId, value: &str) {
    with_realizer(|realizer| realizer.set_value(node, value));
  }

  fn set_options(node: NodeId, options: &[PickerOption]) {
    with_realizer(|realizer| realizer.set_options(node, options));
  }

  fn set_selection(_: NodeId, _: TextSelection) {}

  fn set_checked(node: NodeId, checked: bool) {
    with_realizer(|realizer| realizer.set_checked(node, checked));
  }

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

  /// The server renders once, so nothing scheduled ever runs.
  fn schedule(_: Duration, _: Box<dyn FnOnce()>) -> Timer {
    Timer(0)
  }

  fn cancel(_: Timer) {}

  fn stored_scheme() -> Option<ColorScheme> {
    stored_scheme()
  }
}

impl WebMarkup for Web {
  fn create_element(node: NodeId, tag: &'static str) {
    with_realizer(|realizer| {
      realizer.create_element(node, tag, Realized::Box);
    });
  }

  fn set_attribute(node: NodeId, name: &str, value: &str) {
    with_realizer(|realizer| realizer.set_attribute(node, name, value));
  }

  fn set_declarations(node: NodeId, declarations: &Declarations) {
    with_realizer(|realizer| realizer.set_declarations(node, declarations));
  }

  fn set_text(node: NodeId, text: &str) {
    <Self as Platform>::set_text(node, text);
  }
}
