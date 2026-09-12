//! The element choices both web leaves share: which element a role or a control
//! realizes as, the attributes a link, a focus order and an accessibility property
//! carry, and the markers hydration matches by.

use skene_core::{
  AccessProp,
  Axis,
  ControlKind,
  ControlProps,
  Driver,
  FocusOrder,
  LinkAnnotation,
  LinkPolicy,
  NodeId,
  PopupKind,
  Role,
  Toggle,
};

/// The class the native input a control keeps inside its label carries.
pub(crate) const NATIVE_CLASS: &str = "skene-native";

/// The id of the server's `<style>` block, which the browser leaf adopts.
pub(crate) const STYLE_ID: &str = "skene-styles";

/// The attribute every further `<style>` block the driver owns carries: the browser
/// leaf's per-class rules, and the delta a streamed chunk brings.
pub(crate) const STYLE_ATTRIBUTE: &str = "data-skene-style";

/// The attribute carrying an element's creation-order hydration marker.
pub(crate) const MARKER_ATTRIBUTE: &str = "data-h";

/// The attribute a scope's layout-invisible element carries.
pub(crate) const SCOPE_ATTRIBUTE: &str = "data-skene-scope";

/// The element a box is realized as: an anchor when it has a destination, else the
/// element its role names.
pub(crate) const fn box_tag(role: Option<Role>, destination: bool) -> &'static str {
  if destination { "a" } else { element_tag(role) }
}

/// The element a box of `role` is realized as.
pub(crate) const fn element_tag(role: Option<Role>) -> &'static str {
  match role {
    Some(Role::Navigation) => "nav",
    Some(Role::Main) => "main",
    Some(Role::Header) => "header",
    Some(Role::Footer) => "footer",
    Some(Role::Aside) => "aside",
    Some(Role::Section) => "section",
    Some(Role::Article) => "article",
    Some(Role::Heading(level)) => match level {
      0 | 1 => "h1",
      2 => "h2",
      3 => "h3",
      4 => "h4",
      5 => "h5",
      _ => "h6",
    },
    Some(Role::List) => "ul",
    Some(Role::ListItem) => "li",
    Some(Role::Label) => "label",
    Some(_) | None => "div",
  }
}

/// The native element a control stands in: a text entry is the element itself, and
/// every other kind keeps it inside its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) struct NativeElement {
  /// The element's tag.
  pub tag: &'static str,
  /// The element's `type` attribute, when it is an `input`.
  pub kind: Option<&'static str>,
}

impl NativeElement {
  /// Whether the element is a text entry: the control's own element rather than one
  /// kept inside its label.
  #[inline(always)]
  pub(crate) fn is_entry(self) -> bool {
    matches!(
      (self.tag, self.kind),
      ("textarea", _) | ("input", Some("text"))
    )
  }

  /// Whether the element is a checkbox or a radio: one the platform toggles itself.
  #[cfg(target_os = "unknown")]
  #[inline(always)]
  pub(crate) fn is_toggle(self) -> bool {
    matches!(self.kind, Some("checkbox" | "radio"))
  }
}

/// The native element a control of `kind` stands in, if the platform has one.
pub(crate) const fn native_element(kind: ControlKind) -> Option<NativeElement> {
  let (tag, kind) = match kind {
    ControlKind::Checkbox => ("input", Some("checkbox")),
    ControlKind::Radio => ("input", Some("radio")),
    ControlKind::Range => ("input", Some("range")),
    ControlKind::TextField => ("input", Some("text")),
    ControlKind::TextEditor => ("textarea", None),
    ControlKind::Button
    | ControlKind::RadioGroup
    | ControlKind::Listbox
    | ControlKind::ListboxOption
    | ControlKind::Combobox => return None,
  };

  Some(NativeElement { tag, kind })
}

/// The element a control without a native counterpart is drawn as, and the attribute
/// naming what it is.
pub(crate) const fn drawn_element(
  kind: ControlKind,
) -> (&'static str, Option<(&'static str, &'static str)>) {
  match kind {
    ControlKind::Button => ("button", Some(("type", "button"))),
    ControlKind::RadioGroup => ("div", Some(("role", "radiogroup"))),
    ControlKind::Listbox => ("div", Some(("role", "listbox"))),
    ControlKind::ListboxOption => ("div", Some(("role", "option"))),
    ControlKind::Combobox => ("div", Some(("role", "combobox"))),
    ControlKind::Checkbox
    | ControlKind::Radio
    | ControlKind::Range
    | ControlKind::TextField
    | ControlKind::TextEditor => ("div", None),
  }
}

/// The attributes of a link to `href` under `policy`: `href`, `target` and `rel`.
pub(crate) fn link_attributes(href: &str, policy: LinkPolicy) -> Vec<(&'static str, String)> {
  let external = href.starts_with("//") || href.contains("://");
  let new_page = policy.new_page.unwrap_or(external);
  let mut rel = Vec::new();
  let mut attributes = vec![("href", href.to_owned())];

  if new_page && !policy.expose_opener {
    rel.push("noopener");
  }

  if external && !policy.send_referrer {
    rel.push("noreferrer");
  }

  match policy.annotation {
    Some(LinkAnnotation::NoFollow) => rel.push("nofollow"),
    Some(LinkAnnotation::Sponsored) => rel.push("sponsored"),
    Some(LinkAnnotation::Ugc) => rel.push("ugc"),
    None => {}
  }

  if new_page {
    attributes.push(("target", "_blank".to_owned()));
  }

  if !rel.is_empty() {
    attributes.push(("rel", rel.join(" ")));
  }

  attributes
}

/// The ARIA attribute carrying `prop`.
pub(crate) fn access_attribute(prop: &AccessProp) -> (&'static str, String) {
  match prop {
    AccessProp::Checked(toggle) => (
      "aria-checked",
      match toggle {
        Toggle::On => "true",
        Toggle::Off => "false",
        Toggle::Mixed => "mixed",
      }
      .to_owned(),
    ),
    AccessProp::Value(value) => ("aria-valuenow", value.to_string()),
    AccessProp::ValueMin(value) => ("aria-valuemin", value.to_string()),
    AccessProp::ValueMax(value) => ("aria-valuemax", value.to_string()),
    AccessProp::ValueText(text) => ("aria-valuetext", text.clone()),
    AccessProp::Expanded(state) => ("aria-expanded", state.to_string()),
    AccessProp::Selected(state) => ("aria-selected", state.to_string()),
    AccessProp::Disabled(state) => ("aria-disabled", state.to_string()),
    AccessProp::Label(text) => ("aria-label", text.clone()),
    AccessProp::Required(state) => ("aria-required", state.to_string()),
    AccessProp::Invalid(state) => ("aria-invalid", state.to_string()),
    AccessProp::ReadOnly(state) => ("aria-readonly", state.to_string()),
    AccessProp::HasPopup(kind) => (
      "aria-haspopup",
      match kind {
        PopupKind::Listbox => "listbox",
        PopupKind::Menu => "menu",
        PopupKind::Dialog => "dialog",
      }
      .to_owned(),
    ),
    AccessProp::Controls(node) => ("aria-controls", element_id(*node)),
    AccessProp::DescribedBy(node) => ("aria-describedby", element_id(*node)),
    AccessProp::Orientation(axis) => (
      "aria-orientation",
      match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
      }
      .to_owned(),
    ),
  }
}

/// The native attribute carrying `prop` on a form element, where one exists; the form
/// element then enforces it without script.
pub(crate) const fn native_attribute(prop: &AccessProp) -> Option<&'static str> {
  match prop {
    AccessProp::Required(true) => Some("required"),
    AccessProp::ReadOnly(true) => Some("readonly"),
    _ => None,
  }
}

/// The native attributes [`native_attribute`] may set, cleared before a rewrite.
pub(crate) const NATIVE_ACCESS_ATTRIBUTES: [&str; 2] = ["required", "readonly"];

/// The attributes a control's native element carries: its `type`, the placeholder and
/// the lines of a text entry, and the accessible name when nothing visible supplies one.
pub(crate) fn native_attributes<D: Driver>(
  native: NativeElement,
  props: &ControlProps<D>,
) -> Vec<(&'static str, String)> {
  let mut attributes = Vec::new();

  if let Some(kind) = native.kind {
    attributes.push(("type", kind.to_owned()));
  }

  if let Some(placeholder) = &props.placeholder {
    attributes.push(("placeholder", placeholder.clone()));
  }

  if native.tag == "textarea" {
    attributes.push(("rows", props.lines.to_string()));
  }

  if let Some(label) = &props.label {
    attributes.push(("aria-label", label.clone()));
  }

  attributes
}

/// Whether `tag` is an element the `disabled` attribute disables natively.
pub(crate) fn takes_disabled(tag: &str) -> bool {
  tag.eq_ignore_ascii_case("button") || is_form_element(tag)
}

/// Whether `tag` is a form element that enforces its native attributes.
pub(crate) fn is_form_element(tag: &str) -> bool {
  tag.eq_ignore_ascii_case("input")
    || tag.eq_ignore_ascii_case("textarea")
    || tag.eq_ignore_ascii_case("select")
}

/// The node another node's accessibility refers to, as an element id.
pub(crate) fn element_id(node: NodeId) -> String {
  format!("skene-{}-{}", node.index(), node.generation())
}

/// The nodes `access` refers to by id.
pub(crate) fn referenced_nodes(access: &[AccessProp]) -> impl Iterator<Item = NodeId> + '_ {
  access.iter().filter_map(|prop| match prop {
    AccessProp::Controls(node) | AccessProp::DescribedBy(node) => Some(*node),
    _ => None,
  })
}

/// The `tabindex` a node carries for `focus_order`; `None` removes it. A control out
/// of focus keeps `-1`, so it stays focusable by script.
pub(crate) fn focus_order_attribute(
  focus_order: Option<FocusOrder>,
  control: bool,
) -> Option<String> {
  match focus_order {
    Some(focus_order) => Some(render_focus_order(focus_order)),
    None if control => Some("-1".to_owned()),
    None => None,
  }
}

/// The text of the `ordinal`-th anchor comment: `s:N` starts the `N`-th segment,
/// `e:N` ends it.
pub(crate) fn anchor_text(ordinal: u32) -> String {
  if ordinal.is_multiple_of(2) {
    format!("s:{}", ordinal / 2)
  }
  else {
    format!("e:{}", ordinal / 2)
  }
}

/// Renders a [`FocusOrder`] to its `tabindex` value.
pub(crate) fn render_focus_order(focus_order: FocusOrder) -> String {
  match focus_order {
    FocusOrder::Auto => "0".into(),
    FocusOrder::Position(position) => (u32::from(position) + 1).to_string(),
    FocusOrder::Skip => "-1".into(),
  }
}

/// A widget [`Role`]'s ARIA spelling.
///
/// Landmark roles never reach here—they realize as their semantic element instead.
#[must_use]
pub(crate) fn aria_role(role: Role) -> &'static str {
  match role {
    Role::Button => "button",
    Role::Checkbox => "checkbox",
    Role::Radio => "radio",
    Role::RadioGroup => "radiogroup",
    Role::Slider => "slider",
    Role::Listbox => "listbox",
    Role::ListboxOption => "option",
    Role::Combobox => "combobox",
    _ => "",
  }
}
