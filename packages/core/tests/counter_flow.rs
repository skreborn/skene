//! End-to-end test of the full framework stack against a recording mock backend.
//!
//! No browser is involved: a `MockBackend` stands in for `skene_dom`, recording
//! the node-tree mutations the core issues and capturing the event sink. The
//! test builds the counter view, fires a click through the sink exactly as the
//! DOM backend would, and asserts the reactive label text was patched. This
//! exercises reactive engine → core view building → backend mutations → event
//! routing → effect re-run — everything except the literal DOM.

use std::{cell::RefCell, rc::Rc};

use skene_core::{Button, Flex, Label, RwSignal, SignalGet, SignalUpdate};
use skene_renderer::{
  Backend,
  ElementKind,
  Event,
  EventKind,
  EventSink,
  Modifiers,
  NodeId,
  PointerButton,
  PointerEvent,
  Style,
};

#[derive(Default)]
struct MockState {
  next: u64,
  text: Vec<(NodeId, String)>,
  listeners: Vec<(NodeId, EventKind)>,
  sink: Option<EventSink>,
}

#[derive(Clone)]
struct MockBackend(Rc<RefCell<MockState>>);

impl MockBackend {
  fn new() -> MockBackend {
    // id 0 is reserved for the root.
    MockBackend(Rc::new(RefCell::new(MockState {
      next: 1,
      ..Default::default()
    })))
  }
}

impl Backend for MockBackend {
  fn root(&self) -> NodeId {
    NodeId::from_raw(0)
  }
  fn create_element(&mut self, _kind: ElementKind) -> NodeId {
    let mut s = self.0.borrow_mut();
    let id = NodeId::from_raw(s.next);
    s.next += 1;
    id
  }
  fn set_style(&mut self, _node: NodeId, _style: &Style) {}
  fn set_text(&mut self, node: NodeId, text: &str) {
    self.0.borrow_mut().text.push((node, text.to_string()));
  }
  fn set_attribute(&mut self, _node: NodeId, _name: &str, _value: Option<&str>) {}
  fn insert_child(&mut self, _parent: NodeId, _child: NodeId, _before: Option<NodeId>) {}
  fn remove_child(&mut self, _parent: NodeId, _child: NodeId) {}
  fn destroy(&mut self, _node: NodeId) {}
  fn listen(&mut self, node: NodeId, kind: EventKind) {
    self.0.borrow_mut().listeners.push((node, kind));
  }
  fn set_event_sink(&mut self, sink: EventSink) {
    self.0.borrow_mut().sink = Some(sink);
  }
}

/// The latest text set on `node`, if any.
fn latest_text(state: &MockState, node: NodeId) -> Option<String> {
  state
    .text
    .iter()
    .rev()
    .find(|(n, _)| *n == node)
    .map(|(_, t)| t.clone())
}

#[test]
fn counter_increments_through_the_backend() {
  let mock = MockBackend::new();
  let state = mock.0.clone();
  let backend: Rc<RefCell<dyn Backend>> = Rc::new(RefCell::new(mock));
  skene_core::set_backend(backend);

  let count = RwSignal::new(0i32);

  skene_core::mount(move || {
    Flex::vertical()
      .child(Label::derived(move || format!("Count: {}", count.get())).font_size(32.0))
      .child(
        Flex::horizontal()
          .child(
            Button::new("-")
              .action(move || count.update(|c| *c -= 1))
              .build(),
          )
          .child(
            Button::new("+")
              .action(move || count.update(|c| *c += 1))
              .build(),
          )
          .build(),
      )
      .build()
  });

  // The reactive label should have produced its initial text exactly once.
  let label_node = {
    let s = state.borrow();
    let found: Vec<_> = s
      .text
      .iter()
      .filter(|(_, t)| t.starts_with("Count: "))
      .collect();
    assert_eq!(
      found.len(),
      1,
      "label text set once on build, got {found:?}"
    );
    let (node, text) = found[0];
    assert_eq!(text, "Count: 0");
    *node
  };

  // Two buttons should have registered click listeners.
  let (decrement_node, increment_node) = {
    let s = state.borrow();
    let clicks: Vec<NodeId> = s
      .listeners
      .iter()
      .filter(|(_, k)| *k == EventKind::Click)
      .map(|(n, _)| *n)
      .collect();
    assert_eq!(clicks.len(), 2, "two click listeners registered");
    (clicks[0], clicks[1])
  };

  let sink = state
    .borrow()
    .sink
    .clone()
    .expect("backend received an event sink");

  let click = |node: NodeId| {
    sink(
      node,
      Event::Click(PointerEvent {
        x: 0.0,
        y: 0.0,
        button: PointerButton::Primary,
        modifiers: Modifiers::default(),
      }),
    )
  };

  // Click "+": the label effect should re-run and patch the text.
  click(increment_node);
  assert_eq!(count.get(), 1);
  assert_eq!(
    latest_text(&state.borrow(), label_node).as_deref(),
    Some("Count: 1")
  );

  // Click "+" again, then "-": ends at 1.
  click(increment_node);
  click(decrement_node);
  assert_eq!(count.get(), 1);
  assert_eq!(
    latest_text(&state.borrow(), label_node).as_deref(),
    Some("Count: 1")
  );
}
