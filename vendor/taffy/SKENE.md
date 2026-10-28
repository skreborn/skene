Skene's copy of `taffy` 0.15.1 (crates.io), patched into the workspace through the root
manifest's `[patch.crates-io]` and carried until its change is released upstream. It is not a
workspace member: it keeps its own manifest and lints, and the workspace's formatting, lints and
house rules do not reach it.

Skene's change, marked `Skene:` in the source:
- tree/taffy_tree.rs (`compute_child_layout`): a box laid out hidden, by its own
  `Display::None` or an ancestor's, and a box with no children clear the out-of-flow children
  hoisted onto them. Neither path reaches `compute_oof_layout`, which rewrites the list, so a
  list recorded by an earlier pass outlived the boxes in it: a containing block whose last
  absolute child was removed, or one inside a hidden subtree, kept the removed child's id, and
  `round_layout` panicked on it (`invalid SlotMap key used`). `UPSTREAM.md` is the report to
  offer, its two reproductions held by skene-layout's tests.

The licence file (LICENSE) is the upstream repository's; the published crate carries none.
