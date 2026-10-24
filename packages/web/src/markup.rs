//! The element choices both web leaves share: which element a role or a control
//! realizes as, the attributes a link, a focus order and an accessibility property
//! carry, and the markers hydration matches by.

use skene_foundation::{
  AccessProp,
  Axis,
  BoxValue,
  CheckState,
  Conditional,
  ControlFacts,
  FocusOrder,
  KeptLeaf,
  NativeElement,
  NodeId,
  PopupKind,
  Property,
  PropertyKey,
  Range,
  Role,
  SlotValue,
  StaticLeaf,
};

use crate::compile::KeptStyle;

/// The id of the server's `<style>` block, which the browser leaf adopts.
pub(crate) const STYLE_ID: &str = "skene-styles";

/// The attribute every further `<style>` block the platform owns carries: the browser
/// leaf's per-class rules, and the delta a streamed chunk brings.
pub(crate) const STYLE_ATTRIBUTE: &str = "data-skene-style";

/// The attribute carrying an element's creation-order hydration marker.
pub(crate) const MARKER_ATTRIBUTE: &str = "data-h";

/// The attribute a scope's layout-invisible element carries.
pub(crate) const SCOPE_ATTRIBUTE: &str = "data-skene-scope";

/// The element a box is realized as: an anchor when it is a `link`, else the element
/// its role names, a heading's at its `level`.
pub(crate) const fn box_tag(role: Option<Role>, level: Option<usize>, link: bool) -> &'static str {
  if link { "a" } else { element_tag(role, level) }
}

/// The element a box of `role` is realized as, a heading's at its `level`: the second
/// where it names none.
pub(crate) const fn element_tag(role: Option<Role>, level: Option<usize>) -> &'static str {
  match role {
    Some(Role::Navigation) => "nav",
    Some(Role::Main) => "main",
    Some(Role::Banner | Role::Header) => "header",
    Some(Role::ContentInfo | Role::Footer) => "footer",
    Some(Role::Complementary) => "aside",
    Some(Role::Region | Role::Section) => "section",
    Some(Role::Article) => "article",
    Some(Role::Heading) => match level {
      Some(0 | 1) => "h1",
      None | Some(2) => "h2",
      Some(3) => "h3",
      Some(4) => "h4",
      Some(5) => "h5",
      Some(_) => "h6",
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
pub(crate) struct FormElement {
  /// The element's tag.
  pub tag: &'static str,
  /// The element's `type` attribute, when it is an `input`.
  pub kind: Option<&'static str>,
}

impl FormElement {
  /// Whether the element stands bare: the control's own element, its caption a
  /// `<label for>` beside it, rather than an input kept inside its label.
  #[inline(always)]
  pub(crate) fn is_bare(self) -> bool {
    matches!(
      (self.tag, self.kind),
      ("textarea" | "select", _) | ("input", Some("text"))
    )
  }

  /// Whether the element is a text entry's: a text `<input>` or a `<textarea>`.
  #[inline(always)]
  pub(crate) fn is_text_entry(self) -> bool {
    matches!(
      (self.tag, self.kind),
      ("textarea", _) | ("input", Some("text"))
    )
  }

  /// Whether the element is a checkbox or a radio: one the platform toggles itself.
  #[cfg(not(skene_server))]
  #[inline(always)]
  pub(crate) fn is_toggle(self) -> bool {
    matches!(self.kind, Some("checkbox" | "radio"))
  }
}

/// The form element a control standing in `native` is realized with, where it is one.
pub(crate) const fn form_element(native: Option<NativeElement>) -> Option<FormElement> {
  let (tag, kind) = match native {
    Some(NativeElement::Checkbox) => ("input", Some("checkbox")),
    Some(NativeElement::Radio) => ("input", Some("radio")),
    Some(NativeElement::Slider) => ("input", Some("range")),
    Some(NativeElement::TextField) => ("input", Some("text")),
    Some(NativeElement::TextEditor) => ("textarea", None),
    Some(NativeElement::Picker) => ("select", None),
    Some(NativeElement::Button) | None => return None,
  };

  Some(FormElement { tag, kind })
}

/// The element a control without a form element is drawn as, and the attribute naming
/// what it is: a button's own element, else a generic one spelling its `role`.
pub(crate) const fn drawn_element(
  native: Option<NativeElement>,
  role: Option<Role>,
) -> (&'static str, Option<(&'static str, &'static str)>) {
  if let Some(NativeElement::Button) = native {
    return ("button", Some(("type", "button")));
  }

  let attribute = match role {
    Some(role) => match aria_role(role) {
      Some(aria) => Some(("role", aria)),
      None => None,
    },
    None => None,
  };

  ("div", attribute)
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

/// The ARIA attribute carrying `prop` and its value, where one exists.
fn access_attribute(prop: &AccessProp) -> Option<(&'static str, String)> {
  Some(match prop {
    AccessProp::Checked(toggle) => (
      "aria-checked",
      match toggle {
        CheckState::On => "true",
        CheckState::Off => "false",
        CheckState::Mixed => "mixed",
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
    AccessProp::Name(text) => ("aria-label", text.clone()),
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
    AccessProp::Controls(nodes) => ("aria-controls", element_ids(nodes)?),
    AccessProp::LabelledBy(nodes) => ("aria-labelledby", element_ids(nodes)?),
    AccessProp::DescribedBy(nodes) => ("aria-describedby", element_ids(nodes)?),
    AccessProp::Description(text) => ("title", text.clone()),
    AccessProp::Orientation(axis) => (
      "aria-orientation",
      match axis {
        Axis::Horizontal => "horizontal",
        Axis::Vertical => "vertical",
      }
      .to_owned(),
    ),
    AccessProp::Level(level) => ("aria-level", level.to_string()),
  })
}

/// Whether an access write replaces the attribute `name`: every ARIA attribute but the
/// label a control is created with, and a description's `title`.
pub(crate) fn replaced_by_access(name: &str) -> bool {
  (name.starts_with("aria-") && name != "aria-label") || name == "title"
}

/// The native attribute carrying `prop` on a form element and its value, where one
/// exists; the form element then enforces it without script.
const fn native_attribute(prop: &AccessProp) -> Option<(&'static str, String)> {
  match prop {
    AccessProp::Required(true) => Some(("required", String::new())),
    AccessProp::ReadOnly(true) => Some(("readonly", String::new())),
    _ => None,
  }
}

/// Whether a form element shows `prop` through its content rather than an attribute of
/// the report: a range's value, bounds and step.
const fn shown_by_content(prop: &AccessProp) -> bool {
  matches!(
    prop,
    AccessProp::Value(_)
      | AccessProp::ValueMin(_)
      | AccessProp::ValueMax(_)
      | AccessProp::ValueStep(_)
  )
}

/// The native attributes an access write clears before it rewrites them: every one
/// [`access_attributes`] gives a form element.
pub(crate) const NATIVE_ACCESS_ATTRIBUTES: [&str; 2] = ["required", "readonly"];

/// The attributes `access` gives an element, a form element taking the native
/// attribute where one exists and ARIA otherwise, and none for what its content shows.
pub(crate) fn access_attributes(access: &[AccessProp], form: bool) -> Vec<(&'static str, String)> {
  access
    .iter()
    .filter(|prop| !(form && shown_by_content(prop)))
    .filter_map(|prop| {
      form
        .then(|| native_attribute(prop))
        .flatten()
        .or_else(|| access_attribute(prop))
    })
    .collect()
}

/// The attributes a range input shows `range` with, its bounds and step before the value
/// they hold it within: a non-positive step leaves it continuous (`any`).
pub(crate) fn range_attributes(range: &Range) -> [(&'static str, String); 4] {
  [
    ("min", range.minimum.to_string()),
    ("max", range.maximum.to_string()),
    (
      "step",
      if range.is_continuous() {
        "any".to_owned()
      }
      else {
        range.step.to_string()
      },
    ),
    ("value", range.value.to_string()),
  ]
}

/// The attributes a control's native element carries: its `type`, the lines of a text
/// editor, and the accessible name when nothing visible supplies one.
pub(crate) fn native_attributes(
  native: FormElement,
  control: &ControlFacts,
) -> Vec<(&'static str, String)> {
  let mut attributes = Vec::new();

  if let Some(kind) = native.kind {
    attributes.push(("type", kind.to_owned()));
  }

  if native.tag == "textarea" {
    attributes.push(("rows", control.lines.to_string()));
  }

  if let Some(label) = &control.label {
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

/// The nodes a relation refers to, as a list of element ids; `None` for none.
fn element_ids(nodes: &[NodeId]) -> Option<String> {
  (!nodes.is_empty()).then(|| {
    nodes
      .iter()
      .map(|node| element_id(*node))
      .collect::<Vec<_>>()
      .join(" ")
  })
}

/// The nodes `access` refers to by id.
pub(crate) fn referenced_nodes(access: &[AccessProp]) -> impl Iterator<Item = NodeId> + '_ {
  access
    .iter()
    .flat_map(|prop| prop.referenced().iter().copied())
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

/// Whether a box's declared style scrolls its content: an overflow fixed on the base
/// value that scrolls on either axis once computed.
pub(crate) fn scrolls(style: &KeptStyle) -> bool {
  matches!(
    style
      .get(PropertyKey::Shared(Property::Overflow))
      .map(Conditional::base),
    Some(KeptLeaf::Static(StaticLeaf::Fixed(SlotValue::Box(BoxValue::Overflow(overflow)))))
      if overflow.scrolls()
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

/// The ARIA spelling of a role a box is realized with on a generic element; `None` for a
/// role realized as an element of its own, as a landmark is, or one ARIA has no word
/// for.
#[must_use]
pub(crate) const fn aria_role(role: Role) -> Option<&'static str> {
  Some(match role {
    Role::Alert => "alert",
    Role::AlertDialog => "alertdialog",
    Role::Application => "application",
    Role::Blockquote => "blockquote",
    Role::Button | Role::DefaultButton => "button",
    Role::Caption | Role::FigureCaption => "caption",
    Role::Cell | Role::LayoutTableCell => "cell",
    Role::CheckBox => "checkbox",
    Role::Code => "code",
    Role::ColumnHeader => "columnheader",
    Role::ComboBox | Role::EditableComboBox => "combobox",
    Role::Comment => "comment",
    Role::ContentDeletion => "deletion",
    Role::ContentInsertion => "insertion",
    Role::Definition => "definition",
    Role::Dialog => "dialog",
    Role::Document => "document",
    Role::Emphasis => "emphasis",
    Role::Feed => "feed",
    Role::Figure => "figure",
    Role::Form => "form",
    Role::GenericContainer => "generic",
    Role::Grid => "grid",
    Role::GridCell => "gridcell",
    Role::Group => "group",
    Role::Image => "img",
    Role::Link => "link",
    Role::ListBox => "listbox",
    Role::ListBoxOption | Role::MenuListOption => "option",
    Role::Log => "log",
    Role::Mark => "mark",
    Role::Marquee => "marquee",
    Role::Math => "math",
    Role::Menu | Role::MenuListPopup => "menu",
    Role::MenuBar => "menubar",
    Role::MenuItem => "menuitem",
    Role::MenuItemCheckBox => "menuitemcheckbox",
    Role::MenuItemRadio => "menuitemradio",
    Role::Meter => "meter",
    Role::Note => "note",
    Role::Paragraph => "paragraph",
    Role::ProgressIndicator => "progressbar",
    Role::RadioButton => "radio",
    Role::RadioGroup => "radiogroup",
    Role::Row | Role::LayoutTableRow => "row",
    Role::RowGroup => "rowgroup",
    Role::RowHeader => "rowheader",
    Role::ScrollBar => "scrollbar",
    Role::Search => "search",
    Role::SearchInput => "searchbox",
    Role::Slider => "slider",
    Role::SpinButton => "spinbutton",
    Role::Splitter => "separator",
    Role::Status => "status",
    Role::Strong => "strong",
    Role::Switch => "switch",
    Role::Tab => "tab",
    Role::TabList => "tablist",
    Role::TabPanel => "tabpanel",
    Role::Table | Role::LayoutTable => "table",
    Role::Term => "term",
    Role::TextInput | Role::MultilineTextInput => "textbox",
    Role::Time => "time",
    Role::Timer => "timer",
    Role::Toolbar => "toolbar",
    Role::Tooltip => "tooltip",
    Role::Tree => "tree",
    Role::TreeGrid => "treegrid",
    Role::TreeItem => "treeitem",
    _ => return None,
  })
}

#[cfg(test)]
mod tests {
  use skene_foundation::Role;

  use super::{aria_role, element_tag, leaves_origin, runs_script};

  /// A landmark is realized as its sectioning element under ARIA's name and the
  /// element's alike, and spells no role of its own.
  #[test]
  fn a_landmark_is_realized_as_its_element() {
    for (roles, tag) in [
      ([Role::Banner, Role::Header], "header"),
      ([Role::ContentInfo, Role::Footer], "footer"),
      ([Role::Region, Role::Section], "section"),
      ([Role::Complementary, Role::Complementary], "aside"),
    ] {
      for role in roles {
        assert_eq!(element_tag(Some(role), None), tag);
        assert_eq!(aria_role(role), None);
      }
    }
  }

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
