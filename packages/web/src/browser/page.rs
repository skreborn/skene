//! The browser's sink: the document the app mounts in, the hydration adopting the
//! server's markup, and what the reader changed before the client arrived.

use rustc_hash::FxHashMap;
use skene_foundation::{ChoiceOption, Choices, NodeId, TextSelection};
use wasm_bindgen::JsCast;
use web_sys::{
  Document,
  Element,
  HtmlElement,
  HtmlInputElement,
  HtmlOptionElement,
  HtmlSelectElement,
  HtmlTextAreaElement,
};

use super::{COMMENT_NODE, Extra, events, sheet::Sheet};
use crate::{
  markup::{FormElement, MARKER_ATTRIBUTE},
  realize::Sink,
};

/// What the reader changed on a native element before the client arrived.
pub(super) enum Pending {
  /// A checkbox or a radio, checked or not.
  Toggle(bool),
  /// A slider's or a picker's new value.
  Change(String),
  /// Text typed in a text entry, which reports what it shows.
  Edit,
}

/// The server's nodes awaiting adoption, and what adoption found.
pub(super) struct Hydration {
  /// The server's elements not yet adopted, by marker.
  pub(super) elements: FxHashMap<u32, Element>,
  /// The server's anchors not yet adopted, by text.
  pub(super) comments: FxHashMap<String, web_sys::Node>,
  /// The server's elements the client built differently, removed once it finishes.
  pub(super) stale: Vec<Element>,
  /// How many nodes failed to adopt so far.
  pub(super) mismatches: u32,
  /// What the reader changed before the client arrived, by control.
  pub(super) pending: Vec<(NodeId, Pending)>,
  /// The adopted text entries holding text typed before the client arrived, which a
  /// signal's write leaves alone until the text is reported.
  pub(super) typed: Vec<web_sys::Node>,
  /// Whether each of the server's checkboxes and radios was checked as the hydration
  /// began, before the client wrote any.
  pub(super) checked: Vec<(web_sys::Node, bool)>,
}

impl Hydration {
  /// Indexes the server's elements under `root` by marker and its anchors by text.
  pub(super) fn seed(root: &Element) -> Self {
    let mut elements = FxHashMap::default();
    let marked = root
      .query_selector_all(&format!("[{MARKER_ATTRIBUTE}]"))
      .expect("query the hydration markers");

    for index in 0..marked.length() {
      if let Some(element) = marked
        .item(index)
        .and_then(|node| node.dyn_into::<Element>().ok())
        && let Some(marker) = element
          .get_attribute(MARKER_ATTRIBUTE)
          .and_then(|value| value.parse::<u32>().ok())
      {
        elements.insert(marker, element);
      }
    }

    let mut comments = FxHashMap::default();
    let mut pending: Vec<web_sys::Node> = vec![root.clone().into()];

    while let Some(node) = pending.pop() {
      let children = node.child_nodes();

      for index in 0..children.length() {
        if let Some(child) = children.item(index) {
          if child.node_type() == COMMENT_NODE {
            comments.insert(child.node_value().unwrap_or_default(), child);
          }
          else {
            pending.push(child);
          }
        }
      }
    }

    let toggles = root
      .query_selector_all("input[type=checkbox], input[type=radio]")
      .expect("query the toggles");
    let checked = (0..toggles.length())
      .filter_map(|index| toggles.item(index))
      .filter_map(|node| {
        let checked = node.dyn_ref::<HtmlInputElement>()?.checked();

        Some((node, checked))
      })
      .collect();

    Self {
      elements,
      comments,
      stale: Vec::new(),
      mismatches: 0,
      pending: Vec::new(),
      typed: Vec::new(),
      checked,
    }
  }
}

/// The browser's sink: the document the app mounts in, with the hydration in progress.
pub(super) struct Page {
  /// The document the app mounts in.
  pub(super) document: Document,
  /// The hydration in progress.
  pub(super) hydration: Option<Hydration>,
  /// The hydration marker the next element created takes.
  pub(super) markers: u32,
  /// The text entry an input method is composing in, whose value a signal's write
  /// leaves alone until the composition ends.
  pub(super) composing: Option<web_sys::Node>,
}

impl Page {
  /// Whether `element` is a text entry whose shown text is the reader's until it is
  /// reported, which a signal's write leaves alone: an input method is composing in it,
  /// or it holds text typed before the client arrived.
  pub(super) fn holds_reader_text(&self, element: &web_sys::Node) -> bool {
    self.composing.as_ref() == Some(element)
      || self
        .hydration
        .as_ref()
        .is_some_and(|hydration| hydration.typed.contains(element))
  }
}

impl Sink for Page {
  type Element = web_sys::Node;
  type Extra = Extra;
  type Sheet = Sheet;

  const TITLE_TOOLTIPS: bool = false;

  /// The server's element at this creation index while hydrating, else a fresh `tag`.
  fn create_element(&mut self, tag: &'static str) -> (web_sys::Node, bool) {
    let marker = self.markers;

    self.markers += 1;

    if let Some(hydration) = &mut self.hydration {
      match hydration.elements.remove(&marker) {
        Some(element) if element.tag_name().eq_ignore_ascii_case(tag) => {
          let _ = element.remove_attribute(MARKER_ATTRIBUTE);

          return (element.into(), true);
        }
        Some(element) => {
          hydration.mismatches += 1;
          tracing::error!(
            "hydration mismatch: server rendered <{}> where the client builds <{tag}> (marker {marker})",
            element.tag_name().to_lowercase(),
          );
          hydration.stale.push(element);
        }
        None => {
          tracing::debug!("no serialized <{tag}> for marker {marker}; building fresh");
        }
      }
    }

    (
      self
        .document
        .create_element(tag)
        .expect("an element")
        .into(),
      false,
    )
  }

  /// The server's comment of `text` while hydrating, else a fresh one.
  fn create_comment(&mut self, text: String) -> (web_sys::Node, bool) {
    if let Some(hydration) = &mut self.hydration {
      if let Some(comment) = hydration.comments.remove(&text) {
        return (comment, true);
      }

      tracing::debug!("no serialized anchor {text}; building fresh");
    }

    (self.document.create_comment(&text).into(), false)
  }

  fn bind(&mut self, element: &web_sys::Node, node: NodeId) {
    if let Some(element) = element.dyn_ref::<Element>() {
      events::stamp(element, node);
    }
  }

  fn discard(&mut self, _: &web_sys::Node) {}

  fn append_child(&mut self, parent: &web_sys::Node, child: &web_sys::Node) {
    if child.parent_node().as_ref() != Some(parent) {
      let _ = parent.append_child(child);
    }
  }

  fn set_attribute(&mut self, element: &web_sys::Node, name: &str, value: &str) {
    if let Some(element) = element.dyn_ref::<Element>() {
      let _ = element.set_attribute(name, value);
    }
  }

  fn remove_attribute(&mut self, element: &web_sys::Node, name: &str) {
    if let Some(element) = element.dyn_ref::<Element>() {
      let _ = element.remove_attribute(name);
    }
  }

  fn remove_attributes(&mut self, element: &web_sys::Node, matches: impl Fn(&str) -> bool) {
    let Some(element) = element.dyn_ref::<Element>()
    else {
      return;
    };
    let names = element.get_attribute_names();

    for index in 0..names.length() {
      if let Some(name) = names.get(index).as_string()
        && matches(&name)
      {
        let _ = element.remove_attribute(&name);
      }
    }
  }

  fn set_property(&mut self, element: &web_sys::Node, property: &str, value: &str) {
    if let Some(html) = element.dyn_ref::<HtmlElement>() {
      let _ = html.style().set_property(property, value);
    }
  }

  fn remove_property(&mut self, element: &web_sys::Node, property: &str) {
    if let Some(html) = element.dyn_ref::<HtmlElement>() {
      let _ = html.style().remove_property(property);
    }
  }

  fn custom_properties(&self, element: &web_sys::Node) -> Vec<String> {
    let Some(html) = element.dyn_ref::<HtmlElement>()
    else {
      return Vec::new();
    };
    let style = html.style();

    (0..style.length())
      .map(|index| style.item(index))
      .filter(|property| property.starts_with("--"))
      .collect()
  }

  fn text(&self, element: &web_sys::Node) -> Option<String> {
    element.text_content()
  }

  fn set_text(&mut self, element: &web_sys::Node, text: &str) {
    element.set_text_content(Some(text));
  }

  fn swap_class(&mut self, element: &web_sys::Node, previous: Option<&str>, class: &str) {
    if let Some(element) = element.dyn_ref::<Element>() {
      swap_class_attribute(element, previous, class);
    }
  }

  fn set_checked(&mut self, element: &web_sys::Node, checked: bool) {
    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
      input.set_checked(checked);
    }
  }

  fn set_indeterminate(&mut self, element: &web_sys::Node, indeterminate: bool) {
    if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
      input.set_indeterminate(indeterminate);
    }
  }

  /// Sets the live value, keeping a focused entry's selection within it; an entry an
  /// input method is composing in, or one holding text typed before the client arrived,
  /// is left to the report of its text.
  fn set_value(&mut self, element: &web_sys::Node, value: &str) {
    if self.holds_reader_text(element) {
      return;
    }

    let focused = self.document.active_element().as_ref().map(AsRef::as_ref) == Some(element);
    let length = u32::try_from(value.encode_utf16().count()).unwrap_or(u32::MAX);

    if let Some(input) = element.dyn_ref::<HtmlInputElement>()
      && input.value() != value
    {
      let kept = focused
        .then(|| Option::zip(input.selection_start().ok()?, input.selection_end().ok()?))
        .flatten();

      input.set_value(value);

      if let Some((start, end)) = kept {
        let _ = input.set_selection_range(start.min(length), end.min(length));
      }
    }
    else if let Some(area) = element.dyn_ref::<HtmlTextAreaElement>()
      && area.value() != value
    {
      let kept = focused
        .then(|| Option::zip(area.selection_start().ok()?, area.selection_end().ok()?))
        .flatten();

      area.set_value(value);

      if let Some((start, end)) = kept {
        let _ = area.set_selection_range(start.min(length), end.min(length));
      }
    }
  }

  /// Places the selection, leaving an entry an input method is composing in, or one
  /// holding text typed before the client arrived, to the report of its text.
  fn select(&mut self, element: &web_sys::Node, selection: TextSelection) {
    if self.holds_reader_text(element) {
      return;
    }

    if let Some(element) = element.dyn_ref::<Element>() {
      events::select(element, selection);
    }
  }

  /// Rebuilds the options where they differ from those shown, then selects the chosen
  /// one, or none where the chosen key names no option.
  fn set_choices(&mut self, element: &web_sys::Node, choices: &Choices) {
    let Some(select) = element.dyn_ref::<HtmlSelectElement>()
    else {
      return;
    };

    if !shows_options(select, &choices.options) {
      select.set_inner_html("");

      for option in choices.options.iter() {
        let element = self.document.create_element("option").expect("an option");

        let _ = element.set_attribute("value", &option.key);

        if option.disabled {
          let _ = element.set_attribute("disabled", "");
        }

        element.set_text_content(Some(&option.name));

        let _ = select.append_child(&element);
      }
    }

    match choices.chosen_option() {
      Some(option) => select.set_value(&option.key),
      None => select.set_selected_index(-1),
    }
  }

  /// Notes the change against what the server serialized, for the control to report
  /// once the tree is built: a toggle's as it was checked when the hydration began.
  fn note_live_state(&mut self, node: NodeId, input: &web_sys::Node, native: FormElement) {
    let pending = if native.is_text_entry() {
      let (value, serialized) = if let Some(input) = input.dyn_ref::<HtmlInputElement>() {
        (
          input.value(),
          input.get_attribute("value").unwrap_or_default(),
        )
      }
      else if let Some(area) = input.dyn_ref::<HtmlTextAreaElement>() {
        (area.value(), area.text_content().unwrap_or_default())
      }
      else {
        return;
      };

      (value != serialized).then_some(Pending::Edit)
    }
    else if let Some(input) = input.dyn_ref::<HtmlInputElement>() {
      if native.is_toggle() {
        let serialized = input.has_attribute("checked");
        let live = self
          .hydration
          .as_ref()
          .and_then(|hydration| {
            hydration
              .checked
              .iter()
              .find(|(toggle, _)| toggle == AsRef::<web_sys::Node>::as_ref(input))
              .map(|(_, checked)| *checked)
          })
          .unwrap_or_else(|| input.checked());

        (live != serialized).then_some(Pending::Toggle(live))
      }
      else {
        let serialized = input.get_attribute("value").unwrap_or_default();

        (input.value() != serialized).then(|| Pending::Change(input.value()))
      }
    }
    else if let Some(select) = input.dyn_ref::<HtmlSelectElement>() {
      let serialized = select
        .query_selector("option[selected]")
        .ok()
        .flatten()
        .and_then(|option| option.get_attribute("value"))
        .unwrap_or_default();

      (select.value() != serialized).then(|| Pending::Change(select.value()))
    }
    else {
      None
    };

    if let (Some(pending), Some(hydration)) = (pending, &mut self.hydration) {
      if matches!(pending, Pending::Edit) {
        hydration.typed.push(input.clone());
      }

      hydration.pending.push((node, pending));
    }
  }
}

/// Whether `select` shows `options` already: the same keys, names and disabled ones, in
/// order.
fn shows_options(select: &HtmlSelectElement, options: &[ChoiceOption]) -> bool {
  let shown = select.options();

  shown.length() as usize == options.len()
    && options.iter().enumerate().all(|(index, option)| {
      u32::try_from(index)
        .ok()
        .and_then(|index| shown.item(index))
        .and_then(|element| element.dyn_into::<HtmlOptionElement>().ok())
        .is_some_and(|element| {
          element.value() == *option.key
            && element.text() == *option.name
            && element.disabled() == option.disabled
        })
    })
}

/// Replaces `previous` with `class` in `element`'s class list.
pub(super) fn swap_class_attribute(element: &Element, previous: Option<&str>, class: &str) {
  let list = element.class_list();

  if let Some(previous) = previous
    && previous != class
  {
    let _ = list.remove_1(previous);
  }

  let _ = list.add_1(class);
}
