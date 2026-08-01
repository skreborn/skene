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
use skene_core::{BoxProp, BoxProps, InputProps, NodeId, Role, RunProp, RunProps};

#[cfg(target_os = "unknown")]
pub(crate) use self::csr::assign_class;
#[cfg(all(target_os = "unknown", feature = "test"))]
pub use self::csr::uninstall;
#[cfg(target_os = "unknown")]
pub use self::csr::{
  adopt_next,
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
  handle_input,
  handle_key_down,
  handle_pointer_down,
  handle_pointer_enter,
  handle_pointer_leave,
  handle_pointer_move,
  handle_pointer_up,
  handle_scroll,
  hydrate,
  hydrate_page,
  install,
  is_server,
  launch,
  mount,
  on_pop,
  override_style,
  push_path,
  register_source,
  set_attribute,
  set_input_value,
  set_style,
  set_text,
  tree::{attach, create_group, destroy, detach},
};
#[cfg(target_os = "wasi")]
pub(crate) use self::ssr::assign_class;
#[cfg(target_os = "wasi")]
pub use self::ssr::{
  adopt_next,
  blur_node,
  create_element,
  current_path,
  drive,
  focus_node,
  handle_blur,
  handle_change,
  handle_click,
  handle_focus,
  handle_input,
  handle_key_down,
  handle_pointer_down,
  handle_pointer_enter,
  handle_pointer_leave,
  handle_pointer_move,
  handle_pointer_up,
  handle_scroll,
  is_server,
  override_style,
  register_boundary,
  register_source,
  render_to_stream,
  render_to_string,
  set_attribute,
  set_input_value,
  set_request_path,
  set_style,
  set_text,
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
    None => "div",
  });

  if matches!(props.role, Some(Role::Button)) {
    set_attribute(node, "type", "button");
  }

  if let Some(tab_index) = props.tab_index {
    set_attribute(node, "tabindex", &tab_index.to_string());
  }

  if props.autofocus {
    set_attribute(node, "autofocus", "");
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

/// Realizes a single-line text input, with its full static style interned as a shared
/// class—or classless when it is unstyled.
#[cfg(target_arch = "wasm32")]
#[must_use]
pub fn create_input(props: &InputProps) -> NodeId {
  let node = create_element("input");

  if let Some(placeholder) = &props.placeholder {
    set_attribute(node, "placeholder", placeholder);
  }

  if let Some(tab_index) = props.tab_index {
    set_attribute(node, "tabindex", &tab_index.to_string());
  }

  if props.autofocus {
    set_attribute(node, "autofocus", "");
  }

  if let Some(template) = mapping::input_template(props) {
    assign_class(node, &template);
  }

  node
}

/// Applies one changed input style property as an inline override, winning over the
/// interned class.
#[cfg(target_arch = "wasm32")]
#[inline(always)]
pub fn update_input(node: NodeId, prop: &BoxProp) {
  override_style(node, &mapping::box_prop_declarations(prop));
}
