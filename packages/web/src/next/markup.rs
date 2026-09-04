//! The element choices both web leaves share: which element a role or a control
//! realizes as, the attributes a link, a focus order and an accessibility property
//! carry, and the markers hydration matches by.

use skene_core::{
  AccessProp,
  FocusOrder,
  LinkAnnotation,
  LinkPolicy,
  Role,
  Toggle,
  next::ControlKind,
};

/// The class the native input inside a control's label carries.
pub(crate) const NATIVE_CLASS: &str = "skene-native";

/// The id of the server's `<style>` block, which the browser leaf adopts.
pub(crate) const STYLE_ID: &str = "skene-styles";

/// The attribute carrying an element's creation-order hydration marker.
pub(crate) const MARKER_ATTRIBUTE: &str = "data-h";

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

/// The native element a control keeps inside its label: the tag and its `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) struct NativeElement {
  /// The element's tag.
  pub tag: &'static str,
  /// The element's `type` attribute, when it is an `input`.
  pub kind: Option<&'static str>,
}

impl NativeElement {
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
  }
}

/// The `tabindex` a node carries for `focus_order`; `None` removes it. A control out
/// of focus keeps `-1`, so it stays focusable by script.
pub(crate) fn focus_order_attribute(
  focus_order: Option<FocusOrder>,
  control: bool,
) -> Option<String> {
  match focus_order {
    Some(focus_order) => Some(crate::render_focus_order(focus_order)),
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
