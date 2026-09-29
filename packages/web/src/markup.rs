//! The element choices both web leaves share: which element a role or a control
//! realizes as, the attributes a link, a focus order and an accessibility property
//! carry, and the markers hydration matches by.

use skene_core::{
  AccessProp,
  Axis,
  Conditional,
  ControlKind,
  ControlProps,
  FocusOrder,
  LinkAnnotation,
  LinkPolicy,
  NodeId,
  Overflow,
  Platform,
  PlatformLeaf,
  PopupKind,
  Role,
  StaticLeaf,
  Toggle,
};

/// The id of the server's `<style>` block, which the browser leaf adopts.
pub(crate) const STYLE_ID: &str = "skene-styles";

/// The attribute every further `<style>` block the platform owns carries: the browser
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

/// The native element a control stands in: a text entry or a picker is the element
/// itself, and every other kind keeps it inside its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) struct NativeElement {
  /// The element's tag.
  pub tag: &'static str,
  /// The element's `type` attribute, when it is an `input`.
  pub kind: Option<&'static str>,
}

impl NativeElement {
  /// Whether the element stands bare: the control's own element, its caption a
  /// `<label for>` beside it, rather than an input kept inside its label.
  #[inline(always)]
  pub(crate) fn is_bare(self) -> bool {
    matches!(
      (self.tag, self.kind),
      ("textarea" | "select", _) | ("input", Some("text"))
    )
  }

  /// Whether the element is a checkbox or a radio: one the platform toggles itself.
  #[cfg(skene_leaf = "browser")]
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
    ControlKind::NativePicker => ("select", None),
    ControlKind::Button
    | ControlKind::RadioGroup
    | ControlKind::Listbox
    | ControlKind::ListboxOption
    | ControlKind::Combobox
    | ControlKind::Menu
    | ControlKind::MenuItem => return None,
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
    ControlKind::Menu => ("div", Some(("role", "menu"))),
    ControlKind::MenuItem => ("div", Some(("role", "menuitem"))),
    ControlKind::Checkbox
    | ControlKind::Radio
    | ControlKind::Range
    | ControlKind::TextField
    | ControlKind::TextEditor
    | ControlKind::NativePicker => ("div", None),
  }
}

/// The elements a head holds that a head registration adds.
const HEAD_TAGS: [&str; 4] = ["base", "link", "meta", "script"];

/// Checks a head registration: `tag` is an element a head holds and every attribute
/// name is one as HTML reads it, a letter then letters, digits, `-`, `_`, `.` and `:`.
///
/// # Panics
///
/// Panics when either is not.
pub(crate) fn check_head_element(tag: &str, attributes: &[(&str, &str)]) {
  assert!(
    HEAD_TAGS.contains(&tag),
    "a head element is one of {HEAD_TAGS:?}, not {tag:?}"
  );

  for (name, _) in attributes {
    let mut characters = name.chars();
    let valid = characters
      .next()
      .is_some_and(|first| first.is_ascii_alphabetic())
      && characters.all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | ':')
      });

    assert!(valid, "{name:?} is not an attribute name");
  }
}

/// The schemes whose URLs always name a host.
const SPECIAL_SCHEMES: [&str; 5] = ["ftp", "http", "https", "ws", "wss"];

/// `href` as a browser reads it before parsing: the spaces and controls around it
/// trimmed, and the tabs and newlines within it removed.
fn as_parsed(href: &str) -> String {
  href
    .trim_matches(|character: char| character <= ' ')
    .chars()
    .filter(|character| !matches!(character, '\t' | '\n' | '\r'))
    .collect()
}

/// The scheme `href` begins with, if any: a letter, then letters, digits, `+`, `-`
/// and `.`, up to the first `:`.
fn scheme(href: &str) -> Option<&str> {
  let (scheme, _) = href.split_once(':')?;
  let mut characters = scheme.chars();
  let valid = characters.next()?.is_ascii_alphabetic()
    && characters
      .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.'));

  valid.then_some(scheme)
}

/// Whether following `href` would run script: a `javascript:` URL, its scheme read as
/// a browser reads it, whatever its case, the spaces and controls around it and the
/// tabs and newlines within it.
pub(crate) fn runs_script(href: &str) -> bool {
  scheme(&as_parsed(href)).is_some_and(|scheme| scheme.eq_ignore_ascii_case("javascript"))
}

/// Whether following `href` leaves for another host, read as a browser reads it: a
/// network-path reference (`//host`, either slash reversed), a URL of a scheme that
/// always names a host (`https:`), or one whose scheme is followed by `//`.
pub(crate) fn leaves_origin(href: &str) -> bool {
  let href = as_parsed(href);
  let network_path = |rest: &str| {
    rest.get(..2).is_some_and(|start| {
      start
        .chars()
        .all(|character| matches!(character, '/' | '\\'))
    })
  };

  match scheme(&href) {
    Some(scheme) => {
      SPECIAL_SCHEMES
        .iter()
        .any(|special| scheme.eq_ignore_ascii_case(special))
        || href[scheme.len() + 1..].starts_with("//")
    }
    None => network_path(&href),
  }
}

/// The attributes of a link to `href` under `policy`: `href`, `target` and `rel`, with
/// no `href` where following it would run script.
pub(crate) fn link_attributes(href: &str, policy: LinkPolicy) -> Vec<(&'static str, String)> {
  let external = leaves_origin(href);
  let new_page = policy.new_page.unwrap_or(external);
  let mut rel = Vec::new();
  let mut attributes = Vec::new();

  if !runs_script(href) {
    attributes.push(("href", href.to_owned()));
  }

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

/// The ARIA attribute carrying `prop` and its value, where one exists.
fn access_attribute(prop: &AccessProp) -> Option<(&'static str, String)> {
  Some(match prop {
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
    AccessProp::ValueStep(_) => return None,
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
    AccessProp::LabelledBy(node) => ("aria-labelledby", element_id(*node)),
    AccessProp::DescribedBy(node) => ("aria-describedby", element_id(*node)),
    AccessProp::Description(text) => ("title", text.clone()),
    AccessProp::Orientation(axis) => (
      "aria-orientation",
      match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
      }
      .to_owned(),
    ),
  })
}

/// Whether an access write replaces the attribute `name`: every ARIA attribute but the
/// label a control is created with, and a description's `title`.
pub(crate) fn replaced_by_access(name: &str) -> bool {
  (name.starts_with("aria-") && name != "aria-label") || name == "title"
}

/// The native attribute carrying `prop` on a form element and its value, where one
/// exists; the form element then enforces it without script.
fn native_attribute(prop: &AccessProp) -> Option<(&'static str, String)> {
  match prop {
    AccessProp::Required(true) => Some(("required", String::new())),
    AccessProp::ReadOnly(true) => Some(("readonly", String::new())),
    AccessProp::Value(value) => Some(("value", value.to_string())),
    AccessProp::ValueMin(value) => Some(("min", value.to_string())),
    AccessProp::ValueMax(value) => Some(("max", value.to_string())),
    AccessProp::ValueStep(step) if *step > 0. => Some(("step", step.to_string())),
    AccessProp::ValueStep(_) => Some(("step", "any".to_owned())),
    _ => None,
  }
}

/// The native attributes an access write clears before it rewrites them: every one
/// [`access_attributes`] gives a form element but `value`, which it only overwrites.
pub(crate) const NATIVE_ACCESS_ATTRIBUTES: [&str; 5] =
  ["required", "readonly", "min", "max", "step"];

/// The attributes `access` gives an element, a form element taking the native
/// attribute where one exists and ARIA otherwise; a native `value` comes last, after
/// the bounds it is held within.
pub(crate) fn access_attributes(access: &[AccessProp], form: bool) -> Vec<(&'static str, String)> {
  let mut attributes: Vec<_> = access
    .iter()
    .filter_map(|prop| {
      form
        .then(|| native_attribute(prop))
        .flatten()
        .or_else(|| access_attribute(prop))
    })
    .collect();

  attributes.sort_by_key(|(name, _)| *name == "value");

  attributes
}

/// The attributes a control's native element carries: its `type`, the placeholder and
/// the lines of a text entry, and the accessible name when nothing visible supplies one.
pub(crate) fn native_attributes<P: Platform>(
  native: NativeElement,
  props: &ControlProps<P>,
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
pub(crate) const fn takes_disabled(tag: &str) -> bool {
  tag.eq_ignore_ascii_case("button") || is_form_element(tag)
}

/// Whether `tag` is a form element that enforces its native attributes.
pub(crate) const fn is_form_element(tag: &str) -> bool {
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
    AccessProp::Controls(node) | AccessProp::LabelledBy(node) | AccessProp::DescribedBy(node) => {
      Some(*node)
    }
    _ => None,
  })
}

/// The `tabindex` a node carries for `focus_order`; `None` removes it. A control out
/// of focus keeps `-1`, so it stays focusable by script, and a box that `scrolls`
/// takes its place in the order, so the keyboard reaches its content.
pub(crate) fn focus_order_attribute(
  focus_order: Option<FocusOrder>,
  control: bool,
  scrolls: bool,
) -> Option<String> {
  match focus_order {
    Some(focus_order) => Some(render_focus_order(focus_order)),
    None if control => Some("-1".to_owned()),
    None if scrolls => Some("0".to_owned()),
    None => None,
  }
}

/// Whether a box's declared style scrolls its content: an overflow of `Scroll` or
/// `Auto` fixed on the base value.
pub(crate) fn scrolls<P: Platform>(style: &skene_core::StaticBoxStyle<P>) -> bool {
  matches!(
    style.overflow.as_ref().map(Conditional::base),
    Some(PlatformLeaf::Static(StaticLeaf::Fixed(
      Overflow::Scroll | Overflow::Auto
    )))
  )
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
/// Landmark roles never reach here—they realize as their semantic element instead;
/// see [`Role::is_spelled`].
#[must_use]
pub(crate) const fn aria_role(role: Role) -> &'static str {
  match role {
    Role::Button => "button",
    Role::Checkbox => "checkbox",
    Role::Radio => "radio",
    Role::RadioGroup => "radiogroup",
    Role::Slider => "slider",
    Role::Listbox => "listbox",
    Role::ListboxOption => "option",
    Role::Combobox => "combobox",
    Role::Menu => "menu",
    Role::MenuItem => "menuitem",
    Role::Dialog => "dialog",
    Role::Image => "img",
    Role::Status => "status",
    Role::Alert => "alert",
    _ => "",
  }
}

#[cfg(test)]
mod tests {
  use super::{leaves_origin, runs_script};

  /// A link leaves the origin by where it points, read as a browser reads it, and not
  /// by containing `://` anywhere.
  #[test]
  fn a_link_leaves_the_origin_by_its_scheme() {
    for href in [
      "https://example.com",
      "HTTP://example.com",
      "https:example.com",
      "//cdn.example.com/a.js",
      "\\\\example.com",
      "/\\example.com",
      " https://example.com",
      "ht\ttps://example.com",
      "git+ssh://example.com/repo",
    ] {
      assert!(leaves_origin(href), "{href:?}");
    }

    for href in [
      "/search?next=https://example.com",
      "page.html",
      "#top",
      "?from=a://b",
      "mailto:a@b",
      "tel:+123",
      "javascript:alert(1)",
      "/",
    ] {
      assert!(!leaves_origin(href), "{href:?}");
    }
  }

  /// A `javascript:` URL is recognized as a browser reads its scheme.
  #[test]
  fn a_script_url_is_recognized_however_it_is_spelled() {
    for href in [
      "javascript:alert(1)",
      "JavaScript:alert(1)",
      " \u{1}javascript:alert(1)",
      "java\tscript:alert(1)",
      "jav\nascript:alert(1)",
    ] {
      assert!(runs_script(href), "{href:?}");
    }

    for href in [
      "/javascript:",
      "https://example.com",
      "javascript",
      "#top",
      "mailto:a@b",
    ] {
      assert!(!runs_script(href), "{href:?}");
    }
  }
}
