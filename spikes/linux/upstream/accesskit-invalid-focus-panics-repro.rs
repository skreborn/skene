//! accesskit_consumer panics when an update moves focus to a node the same update removes.
use accesskit::{Node, NodeId, Role, TreeId, TreeInfo, TreeUpdate};

fn main() {
  let window = NodeId(0);
  let button = NodeId(1);
  let mut root = Node::new(Role::Window);
  root.set_children(vec![button]);

  let initial = TreeUpdate {
    nodes: vec![(window, root), (button, Node::new(Role::Button))],
    tree: Some(TreeInfo::new(window)),
    tree_id: TreeId::ROOT,
    focus: button,
  };
  let mut tree = accesskit_consumer::Tree::new(initial, true);

  println!("initial tree built, focus on the button");

  // The focused button is removed: the window no longer lists it, and the update still names it
  // as the focus, as an app does that forgets to move focus before removing a node.
  let mut root = Node::new(Role::Window);
  root.set_children(vec![]);
  let removal = TreeUpdate {
    nodes: vec![(window, root)],
    tree: None,
    tree_id: TreeId::ROOT,
    focus: button,
  };
  let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    tree.update_and_process_changes(removal, &mut NoHandler);
  }));

  println!("update naming a removed focus: {}", if result.is_err() { "PANICKED" } else { "accepted" });
}

struct NoHandler;

impl accesskit_consumer::TreeChangeHandler for NoHandler {
  fn node_added(&mut self, _: &accesskit_consumer::NodeRef) {}
  fn node_updated(&mut self, _: &accesskit_consumer::NodeRef, _: &accesskit_consumer::NodeRef) {}
  fn focus_moved(
    &mut self,
    _: Option<&accesskit_consumer::NodeRef>,
    _: Option<&accesskit_consumer::NodeRef>,
  ) {
  }
  fn node_removed(&mut self, _: &accesskit_consumer::NodeRef) {}
}
