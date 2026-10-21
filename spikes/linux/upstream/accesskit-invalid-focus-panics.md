# accesskit_consumer: an update whose `focus` names a node it no longer holds panics, on the app's thread

**Crate:** accesskit_consumer 0.39.1 (through accesskit_unix 0.24.0, accesskit 0.25.1)

## What happens

`Tree::update_and_process_changes` validates the new state and panics in `TreeState::validate_global`
(`src/tree.rs:91`) when `TreeUpdate::focus` names a node that is not in the node list:

```
Focused ID #1 is not in the node list
```

An app reaches this whenever the focused node is removed in the same change that rebuilds the tree
(a list row's "Delete" button deleting its own row; a popup list whose focused option goes) and the
toolkit still names it as the focus. The panic happens inside `Adapter::update_if_active`, on the
app's UI thread, and poisons the adapter's mutex, so the AT-SPI thread then panics too
(`accesskit_atspi_common/src/context.rs:73`, `PoisonError`): with a screen reader running, the app
dies; without one, nothing happens, because the update is never built.

## Reproduction

`accesskit-invalid-focus-panics-repro.rs` (manifest `accesskit-invalid-focus-panics-repro.Cargo.toml`)
builds a window holding a focused button, then sends an update that drops the button from the
window's children while `focus` still names it. Output (`-repro.out`):

```
initial tree built, focus on the button
update naming a removed focus: PANICKED
```

## What we ask

The invalid update is the toolkit's bug, and Skene fixes its side (focus moves before a focused node
is removed). The request is about the failure mode: an invalid `TreeUpdate` from a toolkit should not
take the application down, and should not poison the adapter for the AT-SPI side. Options, in order
of preference:

1. Treat a `focus` that names no node as focus on the tree's root (or the previous focus), and log
   it, as browsers do when the focused element is removed (focus goes to the document).
2. Return an error from the update path instead of panicking, leaving the previous state in force.
3. At least document the invariant on `TreeUpdate::focus` ("must name a node present after this
   update") and catch the panic inside the platform adapters so the lock is not poisoned.
