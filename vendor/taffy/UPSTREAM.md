# `TaffyTree` keeps a stale hoisted out-of-flow list, and `round_layout` panics on it

Ready to file at https://github.com/DioxusLabs/taffy/issues, by the author's choice.

## Summary

`compute_child_layout` (src/tree/taffy_tree.rs) records a containing block's hoisted
out-of-flow children only through `compute_oof_layout`, which runs after a container layout
and returns early when the output has no positioning area. Two paths never reach it: the leaf
path (a box with no children, measured through the measure function) and the hidden path (a
box laid out under `RunMode::PerformHiddenLayout`, or its own `Display::None`). A list recorded
by an earlier visible pass therefore outlives the boxes in it, and once one of them is removed,
`round_layout` indexes the removed node and panics with `invalid SlotMap key used`
(taffy_tree.rs:564 in 0.15.1). It reproduces on 0.15.1 and on `main` at 4204e36.

## Reproductions

```rust
use taffy::{TaffyTree, prelude::AvailableSpace, style::{Display, Position, Style}};

fn space() -> taffy::geometry::Size<AvailableSpace> {
  taffy::geometry::Size { width: AvailableSpace::Definite(10.), height: AvailableSpace::Definite(10.) }
}

// A containing block whose last child, an absolute one, is removed becomes a leaf.
#[test]
fn a_leaf_keeps_a_stale_hoisted_list() {
  let mut taffy: TaffyTree<()> = TaffyTree::new();
  let child = taffy.new_leaf(Style { position: Position::Absolute, ..Style::default() }).unwrap();
  let root = taffy.new_with_children(Style { position: Position::Relative, ..Style::default() }, &[child]).unwrap();
  taffy.compute_layout(root, space()).unwrap();
  taffy.remove(child).unwrap();
  taffy.compute_layout(root, space()).unwrap(); // panics: invalid SlotMap key used
}

// A containing block inside a `Display::None` subtree keeps its list while hidden.
#[test]
fn a_hidden_box_keeps_a_stale_hoisted_list() {
  let mut taffy: TaffyTree<()> = TaffyTree::new();
  let pinned = taffy.new_leaf(Style { position: Position::Absolute, ..Style::default() }).unwrap();
  let other = taffy.new_leaf(Style { position: Position::Relative, ..Style::default() }).unwrap();
  let boxed = taffy.new_with_children(Style { position: Position::Relative, ..Style::default() }, &[pinned, other]).unwrap();
  let overlay = taffy.new_with_children(Style { position: Position::Absolute, ..Style::default() }, &[boxed]).unwrap();
  let root = taffy.new_with_children(Style { position: Position::Relative, ..Style::default() }, &[overlay]).unwrap();
  taffy.compute_layout(root, space()).unwrap();
  taffy.set_style(overlay, Style { position: Position::Absolute, display: Display::None, ..Style::default() }).unwrap();
  taffy.compute_layout(root, space()).unwrap();
  taffy.remove(pinned).unwrap();
  taffy.compute_layout(root, space()).unwrap(); // panics: invalid SlotMap key used
}
```

## Proposed fix

A box with no children and a box laid out hidden hoist nothing, so both paths clear the list:

```diff
         if inputs.run_mode == RunMode::PerformHiddenLayout {
             debug_log!("HIDDEN");
             self.taffy.nodes[node_id.into()].final_layout = Layout::with_order(0);
+            self.taffy.nodes[node_id.into()].hoisted_children.clear();
             return compute_hidden_layout(self, node_id);
         }
 ...
-                (Display::None, _) => compute_hidden_layout(tree, node_id),
+                (Display::None, _) => {
+                    tree.taffy.nodes[node_id.into()].hoisted_children.clear();
+                    compute_hidden_layout(tree, node_id)
+                }
 ...
                 (_, false) => {
                     let node_key = node_id.into();
+                    tree.taffy.nodes[node_key].hoisted_children.clear();
                     let style = &tree.taffy.nodes[node_key].style;
```

An alternative is for `TaffyTree::remove` to drop the removed node from every hoisted list, which
fixes the panic but keeps a hidden box's list of boxes it no longer lays out.
