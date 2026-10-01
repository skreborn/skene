//! Dev-loop restoration: the focused node with its selection and the scrolled nodes
//! ride the snapshot by their snapshot positions, so a rebuilt page resumes where the
//! reader was.

use skene_foundation::{NodeId, Scene};
use skene_reactive::SnapshotPlace;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, HtmlInputElement, HtmlTextAreaElement};

use super::{DOM, Web, with_dom};

/// The snapshot entry's tag.
const TAG: &str = "skene-web:restore";

/// What the page carries across a restart: the focused node with its selection, and
/// every scrolled node's offsets, each by position.
type Carry = (
  Option<(SnapshotPlace, Option<(u32, u32)>)>,
  Vec<(SnapshotPlace, i32, i32)>,
);

/// The current page's carry; `None` when nothing is focused or scrolled, or no page
/// is installed.
fn current() -> Option<Carry> {
  DOM.with(|dom| {
    let dom = dom.borrow();
    let dom = dom.as_ref()?;
    let active = dom.document().active_element();
    let mut focused = None;
    let mut scrolls = Vec::new();

    Scene::<Web>::with(|scene| {
      for (id, node) in dom.realizer.records.iter() {
        let Some(position) = scene.record(id).map(|record| record.position().clone())
        else {
          continue;
        };

        if let Some(active) = &active
          && node.carrier_element() == Some(active)
        {
          focused = Some((position.clone(), selection_of(active)));
        }

        if let Some(element) = node.dom_element() {
          let (x, y) = (element.scroll_left(), element.scroll_top());

          if x != 0 || y != 0 {
            scrolls.push((position, x, y));
          }
        }
      }
    });

    (focused.is_some() || !scrolls.is_empty()).then_some((focused, scrolls))
  })
}

/// The selection inside a text control; `None` for any other element.
fn selection_of(element: &Element) -> Option<(u32, u32)> {
  if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
    return Some((input.selection_start().ok()??, input.selection_end().ok()??));
  }

  let area = element.dyn_ref::<HtmlTextAreaElement>()?;

  Some((area.selection_start().ok()??, area.selection_end().ok()??))
}

/// Selects `start..end` inside a text control; any other element is left alone.
fn select(element: &Element, start: u32, end: u32) {
  if let Some(input) = element.dyn_ref::<HtmlInputElement>() {
    let _ = input.set_selection_range(start, end);
  }
  else if let Some(area) = element.dyn_ref::<HtmlTextAreaElement>() {
    let _ = area.set_selection_range(start, end);
  }
}

/// Registers the page's focus, selection and scroll offsets as a snapshot provider; the
/// entry rides every snapshot alongside the tagged signals.
pub(crate) fn provide_restoration() {
  skene_reactive::provide_extra(TAG, || {
    current().map(|carry| skene_reactive::encode(&carry))
  });
}

/// Adopts the focus, selection and scroll offsets a restore staged, once the tree is
/// mounted: entries no live node carries are dropped.
pub fn adopt_restoration() {
  let Some((focused, scrolls)) =
    skene_reactive::take_extra(TAG).and_then(|bytes| skene_reactive::decode::<Carry>(&bytes))
  else {
    return;
  };
  let sink_at = |position: &SnapshotPlace| Scene::<Web>::with(|scene| scene.node_at(position));
  let scrolls: Vec<(NodeId, i32, i32)> = scrolls
    .iter()
    .filter_map(|(position, x, y)| Some((sink_at(position)?, *x, *y)))
    .collect();
  let focused = focused
    .as_ref()
    .and_then(|(position, selection)| Some((sink_at(position)?, *selection)));

  let (scrolled, focused) = with_dom(|dom| {
    let scrolled: Vec<_> = scrolls
      .into_iter()
      .filter_map(|(sink, x, y)| Some((dom.node(sink)?.dom_element()?.clone(), x, y)))
      .collect();
    let focused = focused
      .and_then(|(sink, selection)| Some((dom.node(sink)?.carrier_element()?.clone(), selection)));

    (scrolled, focused)
  });

  for (element, x, y) in scrolled {
    element.set_scroll_left(x);
    element.set_scroll_top(y);
  }

  if let Some((carrier, selection)) = focused {
    if let Some(html) = carrier.dyn_ref::<HtmlElement>() {
      let _ = html.focus();
    }

    if let Some((start, end)) = selection {
      select(&carrier, start, end);
    }
  }
}
