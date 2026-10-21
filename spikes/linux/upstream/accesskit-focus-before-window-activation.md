# accesskit_atspi_common: report a window's focused object before `window:activate`

Crate: `accesskit_atspi_common` 0.21.0 (with `accesskit_unix` 0.24.0), `adapter.rs`,
`AdapterChangeHandler::focus_moved`.

## What happens

When the host window gains focus (`Adapter::update_window_focus_state(true)`), focus moves
from no node to the tree's focused node, and `focus_moved` emits, in this order:

1. `window:activate` for the root window, then `object:state-changed:active` (true) for it and
   `object:active-descendant-changed` on the application root;
2. `object:state-changed:focused` (true) for the focused node.

Orca 46 (the version Ubuntu 24.04 ships) processes the events in order. Its
`default.onWindowActivated` makes the window the active window and sets the locus of focus to
the window itself, so it speaks the bare window ("controls frame."); the focus event that
follows then speaks the object ("Name text."). GTK 4 emits the focused widget's
`state-changed:focused` first and the frame's `state-changed:active` after (no
`window:activate`), so Orca speaks the object in its window's context instead: "controls",
"Name text.". An AccessKit window that opens with a text field focused therefore sounds
different from the same GTK 4 window on GNOME Wayland.

## Reproduction

A window with one focused text field named "Name", in a window titled "controls", opened on
GNOME Shell (Wayland) with Orca 46 running:

- AccessKit: `controls frame.` / `Name text.` / `opens popup`
- GTK 4.14 (`Gtk.Entry` in a `Gtk.Window`): `controls` / `Name text.` / `opens popup`

Orca's debug log shows `window:activate` dequeued and processed first, the locus set to
`[frame: 'controls']` by `default.onWindowActivated`, then the focus event.

## Change

Emit the focused node's `state-changed:focused` (and its text selection) before the window's
activation events when focus moves from no node to one. The window's states already hold
`active` when the events go out (the tree is updated before the handler runs), so Orca's
`can_be_active_window` accepts the focus event; its later `onWindowActivated` finds the window
already active and returns. A window opening with nothing but itself focused still speaks the
window, since its focused node is the window.

```rust
fn focus_moved(&mut self, old_node: Option<&NodeRef>, new_node: Option<&NodeRef>) {
    let activated = if let (None, Some(new_node)) = (old_node, new_node) {
        root_window(new_node.tree_state)
    } else {
        None
    };
    if let (Some(old_node), None) = (old_node, new_node) {
        if let Some(root_window) = root_window(old_node.tree_state) {
            self.adapter.window_deactivated(&NodeWrapper(&root_window));
        }
    }
    if let Some(node) = new_node {
        self.adapter
            .emit_object_event(node.id(), ObjectEvent::StateChanged(State::Focused, true));
        self.emit_text_selection_change(None, node);
    }
    if let Some(node) = old_node {
        self.adapter
            .emit_object_event(node.id(), ObjectEvent::StateChanged(State::Focused, false));
    }
    if let Some(root_window) = activated {
        self.adapter.window_activated(&NodeWrapper(&root_window));
    }
}
```

## Evidence

Skene's drawn Linux prototype carries the change (`spikes/linux/vendor/accesskit_atspi_common`,
marked `Skene:`). Its Orca check speaks the same keys over the drawn window and over the GTK 4
reference window and diffs what Orca said: on GNOME Wayland the one differing hunk
(`controls frame.` against `controls`) is gone with the change (0 hunks); on X11 (no window
manager) the words are unchanged.

## Not covered

Orca 47 and later were not run (only Orca 46.1 is installed where this was measured), nor any
other screen reader.
