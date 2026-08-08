//! The skene web driver.
//!
//! One family, two leaves selected by target: the browser (`csr`,
//! `target_os = "unknown"`) realizes against a live DOM, and the server renderer (`ssr`,
//! `target_os = "wasi"`) against a string tree—both emitting the same web semantics, so
//! a node styled the same way gets the same content-addressed class on either leaf.
//!
//! The crate exposes three surfaces:
//!
//! - **The typed vocabulary** the built-in views realize through—[`create_box`], [`update_box`],
//!   [`create_run`], [`update_run`], [`set_text`], [`handle_click`], the tree mutations—plus the
//!   async and hydration seams ([`drive`], [`is_server`], [`adopt_next`], [`register_source`]).
//! - **The web escape hatch** for a view written directly against the web: [`create_element`],
//!   [`set_attribute`], [`set_style`]/[`override_style`] with raw [`Declarations`], and on the
//!   browser leaf the raw `element`/`document` handles—which do not exist on the server leaf, so
//!   DOM-reaching code fails to compile there.
//! - **The entries**: the browser's `launch`/`hydrate_page`, the server's raw
//!   `render_to_string`/`render_to_stream`.

mod declarations;
#[cfg(target_arch = "wasm32")]
mod interner;

pub use declarations::Declarations;

#[cfg(target_arch = "wasm32")]
mod hydration;
#[cfg(target_arch = "wasm32")]
mod mapping;
#[cfg(target_arch = "wasm32")]
mod styler;

#[cfg(target_arch = "wasm32")]
pub use hydration::{HydrationSource, STATE_SCRIPT_ID};
#[cfg(target_arch = "wasm32")]
pub use styler::Styler;

/// The reset stylesheet every web leaf applies—injected into a `<style>` by the
/// browser, serialized into the document by a server.
#[cfg(target_arch = "wasm32")]
const RESET_CSS: &str = include_str!("reset.css");

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod csr;
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
mod ssr;

// The active leaf's surface. Both leaves expose the same shared vocabulary and seams;
// they differ at the edges. The browser leaf adds the raw-handle escape hatch
// (`element`/`document`) and its entry points—`element` is browser-only, so a view
// reaching for it will not compile for the server—while the server leaf omits those and
// exposes `render_to_string`/`render_to_stream` instead. `uninstall` is the browser
// leaf's test teardown.
#[cfg(target_arch = "wasm32")]
use skene_core::{
  BoxProp,
  BoxProps,
  ButtonProps,
  CheckboxProps,
  ControlProps,
  NodeId,
  OverlayProps,
  PickerProps,
  RadioProps,
  Role,
  RunProp,
  RunProps,
  SliderProps,
  TextEditorProps,
  TextFieldProps,
};

#[cfg(target_os = "unknown")]
pub(crate) use self::csr::assign_class;
#[cfg(all(target_os = "unknown", feature = "snapshot"))]
pub use self::csr::boot_with_dev_state;
#[cfg(all(target_os = "unknown", feature = "test"))]
pub use self::csr::uninstall;
#[cfg(target_os = "unknown")]
pub use self::csr::{
  add_head_element,
  adopt_next,
  adopt_tagged,
  blur_node,
  create_element,
  current_path,
  document,
  drive,
  element,
  focus_node,
  handle_blur,
  handle_change,
  handle_click,
  handle_focus,
  handle_key_down,
  handle_overlay_dismiss,
  handle_pointer_down,
  handle_pointer_enter,
  handle_pointer_leave,
  handle_pointer_move,
  handle_pointer_up,
  handle_scroll,
  handle_toggle,
  hydrate,
  hydrate_page,
  install,
  is_server,
  launch,
  mount,
  observe_size,
  on_pop,
  override_style,
  push_path,
  register_overlay,
  register_source,
  register_tagged_source,
  set_attribute,
  set_checked,
  set_document_title,
  set_overlay_open,
  set_style,
  set_text,
  set_value,
  tree::{attach, create_group, destroy, detach},
};
#[cfg(target_os = "wasi")]
pub(crate) use self::ssr::assign_class;
#[cfg(target_os = "wasi")]
pub use self::ssr::{
  add_head_element,
  adopt_next,
  adopt_tagged,
  blur_node,
  create_element,
  current_path,
  drive,
  focus_node,
  handle_blur,
  handle_change,
  handle_click,
  handle_focus,
  handle_key_down,
  handle_overlay_dismiss,
  handle_pointer_down,
  handle_pointer_enter,
  handle_pointer_leave,
  handle_pointer_move,
  handle_pointer_up,
  handle_scroll,
  handle_toggle,
  is_server,
  observe_size,
  override_style,
  record_static_routes,
  recorded_static_routes,
  register_boundary,
  register_overlay,
  register_source,
  register_tagged_source,
  render_to_stream,
  render_to_string,
  set_attribute,
  set_checked,
  set_document_title,
  set_overlay_open,
  set_render_deadline,
  set_request_path,
  set_style,
  set_text,
  set_value,
  sleep,
  tree::{attach, create_group, destroy, detach},
};

/// Realizes a box: the element for its role, with its full static style—base
/// properties, conditional variants, transitions and animations—interned as one shared
/// class.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_box(props: &BoxProps) -> NodeId {
  let node = create_element(match props.role {
    Some(Role::Button) => "button",
    Some(Role::Navigation) => "nav",
    Some(Role::Main) => "main",
    Some(Role::Header) => "header",
    Some(Role::Footer) => "footer",
    Some(Role::Aside) => "aside",
    Some(Role::Section) => "section",
    Some(Role::Article) => "article",
    Some(Role::Heading(level)) => match level.clamp(1, 6) {
      1 => "h1",
      2 => "h2",
      3 => "h3",
      4 => "h4",
      5 => "h5",
      _ => "h6",
    },
    Some(Role::List) => "ul",
    Some(Role::ListItem) => "li",
    None => "div",
  });

  if matches!(props.role, Some(Role::Button)) {
    set_attribute(node, "type", "button");
  }

  assign_class(node, &mapping::box_template(props));

  node
}

/// Applies one changed box property as an inline override, winning over the interned
/// class.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn update_box(node: NodeId, prop: &BoxProp) {
  override_style(node, &mapping::box_prop_declarations(prop));
}

/// Overlaps a stack child in the box's single cell.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn place_in_stack(node: NodeId) {
  override_style(node, &Declarations::new().set("grid-area", "1 / 1"));
}

/// Realizes a text run: an `<a>` when linked, else a `<span>`, with its full static
/// style interned as a shared class—or classless when it is entirely unstyled.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_run(props: &RunProps) -> NodeId {
  let node = match props.link.as_deref() {
    Some(href) => {
      let node = create_element("a");

      set_attribute(node, "href", href);

      node
    }
    None => create_element("span"),
  };

  if let Some(template) = mapping::run_template(props) {
    assign_class(node, &template);
  }

  node
}

/// Applies one changed text-run property as an inline override, winning over the interned
/// class.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn update_run(node: NodeId, prop: &RunProp) {
  override_style(node, &mapping::run_prop_declarations(prop));
}

/// Renders a [`FocusOrder`](skene_core::FocusOrder) to its `tabindex` value.
#[cfg(target_arch = "wasm32")]
fn render_focus_order(focus_order: skene_core::FocusOrder) -> String {
  match focus_order {
    skene_core::FocusOrder::Auto => "0".into(),
    skene_core::FocusOrder::Position(position) => (u32::from(position) + 1).to_string(),
    skene_core::FocusOrder::Skip => "-1".into(),
  }
}

/// Folds a control's shared surface into `node`: the focus order and autofocus
/// attributes, plus the interned class carrying the control's reset and its style.
#[cfg(target_arch = "wasm32")]
fn apply_control(node: NodeId, control: &ControlProps, reset: &'static str) {
  if let Some(focus_order) = control.focus_order {
    set_attribute(node, "tabindex", &render_focus_order(focus_order));
  }

  if control.autofocus {
    set_attribute(node, "autofocus", "");
  }

  assign_class(node, &mapping::control_template(control, reset));
}

/// Realizes a single-line text input, with its full static style interned as a shared
/// class—or classless when it is unstyled.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_text_field(props: &TextFieldProps) -> NodeId {
  let node = create_element("input");

  if let Some(placeholder) = &props.placeholder {
    set_attribute(node, "placeholder", placeholder);
  }

  apply_control(node, &props.control, mapping::CONTROL_RESET);

  node
}

/// Realizes a button: natively focusable and clickable, its content the attached
/// children.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_button(props: &ButtonProps) -> NodeId {
  let node = create_element("button");

  set_attribute(node, "type", "button");
  apply_control(node, &props.control, mapping::CONTROL_RESET);

  node
}

/// Realizes an overlay: a `<dialog>` when modal, a manual popover otherwise—top-layer
/// elements either way, mounted in place and opened through [`set_overlay_open`].
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_overlay(props: &OverlayProps) -> NodeId {
  let node = if props.modal {
    create_element("dialog")
  }
  else {
    let node = create_element("div");

    set_attribute(node, "popover", "manual");

    node
  };

  assign_class(node, &mapping::overlay_template(props));
  register_overlay(node, props.modal, props.placement);

  node
}

/// Realizes a checkbox: a two-state toggle, checked through [`set_checked`] and
/// reported through [`handle_toggle`].
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_checkbox(props: &CheckboxProps) -> NodeId {
  let node = create_element("input");

  set_attribute(node, "type", "checkbox");
  apply_control(node, &props.control, mapping::CONTROL_RESET);

  node
}

/// Realizes one radio button of a group; the platform keeps one member checked and
/// moves between members with the arrow keys.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_radio(props: &RadioProps) -> NodeId {
  let node = create_element("input");

  set_attribute(node, "type", "radio");
  set_attribute(node, "name", &props.group);
  set_attribute(node, "value", &props.value);
  apply_control(node, &props.control, mapping::CONTROL_RESET);

  node
}

/// Realizes a select control with its options; the chosen value is set through
/// [`set_value`] and reported through [`handle_change`].
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_picker(props: &PickerProps) -> NodeId {
  let node = create_element("select");

  apply_control(node, &props.control, mapping::SELECT_RESET);

  for option in &props.options {
    let child = create_element("option");

    set_attribute(child, "value", &option.value);
    set_text(child, &option.label);
    attach(node, child, None);
  }

  node
}

/// Realizes a multi-line text area, with its value set through [`set_value`] and
/// edits reported through [`handle_change`].
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_text_editor(props: &TextEditorProps) -> NodeId {
  let node = create_element("textarea");

  if let Some(placeholder) = &props.placeholder {
    set_attribute(node, "placeholder", placeholder);
  }

  if let Some(lines) = props.lines {
    set_attribute(node, "rows", &lines.to_string());
  }

  apply_control(node, &props.control, mapping::CONTROL_RESET);

  node
}

/// Realizes a slider over the given range, with its value set through
/// [`set_value`] and moves reported through [`handle_change`].
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_slider(props: &SliderProps) -> NodeId {
  let node = create_element("input");

  set_attribute(node, "type", "range");
  set_attribute(node, "min", &props.min.to_string());
  set_attribute(node, "max", &props.max.to_string());

  if let Some(step) = props.step {
    set_attribute(node, "step", &step.to_string());
  }

  apply_control(node, &props.control, mapping::CONTROL_RESET);

  node
}

/// Applies one changed input style property as an inline override, winning over the
/// interned class.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn update_text_field(node: NodeId, prop: &BoxProp) {
  override_style(node, &mapping::box_prop_declarations(prop));
}
