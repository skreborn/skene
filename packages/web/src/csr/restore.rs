//! Dev-loop restoration: the focused node and the scrolled nodes ride the snapshot,
//! keyed by creation index, so a rebuilt page resumes where the reader was.

use wasm_bindgen::JsCast;
use web_sys::HtmlElement;

use super::{DOM, Node, with_dom};

/// The snapshot entry's tag.
const TAG: &str = "skene-web:restore";

/// What the page carries across a restart: the focused node and every scrolled node's
/// offsets, each by creation index.
struct Carry {
  focused: Option<u32>,
  scrolls: Vec<(u32, i32, i32)>,
}

impl Carry {
  /// The current page's carry; `None` when nothing is focused or scrolled, or no page
  /// is installed.
  fn current() -> Option<Self> {
    DOM.with(|dom| {
      let dom = dom.borrow();
      let dom = dom.as_ref()?;
      let active = dom.document.active_element();
      let mut focused = None;
      let mut scrolls = Vec::new();

      for (index, node) in dom.nodes.iter().enumerate() {
        let Some(node) = node
        else {
          continue;
        };
        let index = u32::try_from(index).ok()?;

        if let Some(active) = &active
          && node.carrier() == Some(active)
        {
          focused = Some(index);
        }

        if let Some(element) = node.element() {
          let (x, y) = (element.scroll_left(), element.scroll_top());

          if x != 0 || y != 0 {
            scrolls.push((index, x, y));
          }
        }
      }

      (focused.is_some() || !scrolls.is_empty()).then_some(Self { focused, scrolls })
    })
  }

  /// The carry as bytes: the focused index or the sentinel, then the scrolled triples.
  fn encode(&self) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + self.scrolls.len() * 12);

    bytes.extend_from_slice(&self.focused.unwrap_or(u32::MAX).to_le_bytes());

    for (index, x, y) in &self.scrolls {
      bytes.extend_from_slice(&index.to_le_bytes());
      bytes.extend_from_slice(&x.to_le_bytes());
      bytes.extend_from_slice(&y.to_le_bytes());
    }

    bytes
  }

  /// The carry [`encode`](Self::encode) wrote; `None` for a malformed entry.
  fn decode(bytes: &[u8]) -> Option<Self> {
    let (head, rest) = bytes.split_at_checked(4)?;
    let focused = u32::from_le_bytes(head.try_into().ok()?);
    let mut scrolls = Vec::with_capacity(rest.len() / 12);

    for triple in rest.chunks_exact(12) {
      scrolls.push((
        u32::from_le_bytes(triple[0..4].try_into().ok()?),
        i32::from_le_bytes(triple[4..8].try_into().ok()?),
        i32::from_le_bytes(triple[8..12].try_into().ok()?),
      ));
    }

    Some(Self {
      focused: (focused != u32::MAX).then_some(focused),
      scrolls,
    })
  }
}

/// Registers the page's focus and scroll offsets as a snapshot provider; the entry
/// rides every snapshot alongside the tagged signals.
pub(crate) fn provide_restoration() {
  skene_reactive::provide(TAG, || Carry::current().map(|carry| carry.encode()));
}

/// Adopts the focus and scroll offsets a restore staged, once the tree is mounted:
/// entries no live node carries are dropped.
pub fn adopt_restoration() {
  let Some(carry) = skene_reactive::staged(TAG).and_then(|bytes| Carry::decode(&bytes))
  else {
    return;
  };

  with_dom(|dom| {
    for (index, x, y) in carry.scrolls {
      if let Some(element) = dom.node(index).and_then(Node::element) {
        element.set_scroll_left(x);
        element.set_scroll_top(y);
      }
    }

    if let Some(html) = carry
      .focused
      .and_then(|index| dom.node(index))
      .and_then(Node::carrier)
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.focus();
    }
  });
}
