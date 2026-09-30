//! What a page keeps across the moves and hides of its content: the scroll offsets and
//! focus of discarded markup and of a hidden subtree, and the engine's state-keeping
//! move.

use skene_core::NodeId;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{Document, Element, HtmlElement};

use super::Dom;

/// What discarded markup held, keyed by the element's index in its document order: the
/// scrolled offsets and the focused element.
pub(super) struct Saved {
  tags: Vec<String>,
  scrolls: Vec<(usize, f64, f64)>,
  focused: Option<usize>,
}

impl Saved {
  /// What the subtrees `roots` hold, in document order; `None` when nothing is
  /// scrolled or focused inside them.
  pub(super) fn capture(document: &Document, roots: &[web_sys::Node]) -> Option<Self> {
    let elements = subtree_elements(roots);
    let active = document.active_element();
    let mut saved = Self {
      tags: elements.iter().map(Element::tag_name).collect(),
      scrolls: Vec::new(),
      focused: None,
    };

    for (index, element) in elements.iter().enumerate() {
      let (left, top) = (element.scroll_left(), element.scroll_top());

      if left != 0 || top != 0 {
        saved.scrolls.push((index, f64::from(left), f64::from(top)));
      }

      if active.as_ref() == Some(element) {
        saved.focused = Some(index);
      }
    }

    (!saved.scrolls.is_empty() || saved.focused.is_some()).then_some(saved)
  }

  /// Gives what was held to `root`'s subtree, when it has the shape the content had.
  pub(super) fn restore(self, root: &web_sys::Node) {
    let elements = subtree_elements(std::slice::from_ref(root));

    if elements.len() != self.tags.len()
      || elements
        .iter()
        .zip(&self.tags)
        .any(|(element, tag)| element.tag_name() != *tag)
    {
      return;
    }

    // Focus lands before the offsets, which it would otherwise scroll away.
    if let Some(html) = self
      .focused
      .and_then(|index| elements.get(index))
      .and_then(|element| element.dyn_ref::<HtmlElement>())
    {
      let _ = html.focus();
    }

    for (index, left, top) in self.scrolls {
      #[expect(
        clippy::cast_possible_truncation,
        reason = "a saved scroll offset is the whole number of pixels an element reported"
      )]
      if let Some(element) = elements.get(index) {
        element.set_scroll_left(left as i32);
        element.set_scroll_top(top as i32);
      }
    }
  }
}

/// Every element under `roots`, each root first, in document order.
fn subtree_elements(roots: &[web_sys::Node]) -> Vec<Element> {
  let mut elements = Vec::new();

  for root in roots {
    let Some(element) = root.dyn_ref::<Element>()
    else {
      continue;
    };

    elements.push(element.clone());

    let descendants = element.query_selector_all("*").expect("query the subtree");

    elements.extend((0..descendants.length()).filter_map(|index| {
      descendants
        .item(index)
        .and_then(|node| node.dyn_into::<Element>().ok())
    }));
  }

  elements
}

/// What a hidden node's subtree held that leaving the document loses: the focused
/// element and the offsets of its scrolled elements.
pub(super) struct Kept {
  focused: Option<HtmlElement>,
  scrolls: Vec<(Element, i32, i32)>,
}

impl Kept {
  /// What the subtree of `root`, realizing `nodes`, holds; `None` when nothing in it is
  /// focused or scrolled.
  pub(super) fn capture(dom: &Dom, root: &web_sys::Node, nodes: &[NodeId]) -> Option<Self> {
    let focused = dom
      .document()
      .active_element()
      .filter(|active| root.contains(Some(active)))
      .and_then(|active| active.dyn_into::<HtmlElement>().ok());
    let scrolls: Vec<_> = nodes
      .iter()
      .filter_map(|node| dom.node(*node)?.dom_element().cloned())
      .filter_map(|element| {
        let (left, top) = (element.scroll_left(), element.scroll_top());

        (left != 0 || top != 0).then_some((element, left, top))
      })
      .collect();

    (focused.is_some() || !scrolls.is_empty()).then_some(Self { focused, scrolls })
  }

  /// Gives the focus back where it has gone nowhere since, then each element its
  /// offsets.
  pub(super) fn restore(self, document: &Document) {
    // Focus lands before the offsets, which it would otherwise scroll away.
    if let Some(focused) = self.focused
      && document
        .active_element()
        .is_none_or(|active| Some(&active) == document.body().as_deref())
    {
      let _ = focused.focus();
    }

    for (element, left, top) in self.scrolls {
      element.set_scroll_left(left);
      element.set_scroll_top(top);
    }
  }
}

/// The engine's state-keeping move, `moveBefore`, where it has one.
#[derive(Clone)]
pub(super) struct Mover(Option<js_sys::Function>);

impl Mover {
  /// The engine's move, looked up once.
  pub(super) fn detect() -> Self {
    let function = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("Element"))
      .and_then(|element| js_sys::Reflect::get(&element, &JsValue::from_str("prototype")))
      .and_then(|prototype| js_sys::Reflect::get(&prototype, &JsValue::from_str("moveBefore")))
      .ok()
      .and_then(|function| function.dyn_into::<js_sys::Function>().ok());

    Self(function)
  }

  /// Moves `node` under `parent`, immediately before `before` when given, else last:
  /// in place, keeping its focus and animations, where the engine can, else by one
  /// insertion.
  pub(super) fn move_node(
    &self,
    parent: &web_sys::Node,
    node: &web_sys::Node,
    before: Option<&web_sys::Node>,
  ) {
    let moved = self.0.as_ref().is_some_and(|function| {
      function
        .call2(parent, node, before.map_or(&JsValue::NULL, AsRef::as_ref))
        .is_ok()
    });

    if !moved {
      let _ = parent.insert_before(node, before);
    }
  }
}
