Skene's copy of `accesskit_atspi_common` 0.21.0 (crates.io), carried until the changes are
proven in practice and offered upstream (decision: Skene carries its AccessKit changes itself).

Skene's changes, each marked `Skene:` in the source:
- context.rs, node.rs: an action asked for while the tree is read is queued and run once the
  tree's lock is released, so the app's handler may wait until it applied the action and pushed
  the tree's update: a reader's write (SetCaretOffset, SetTextContents, AddSelection, ...)
  reads back at once.
- node.rs: EditableText's insert, delete, copy, cut and paste, composed from SetTextSelection and
  ReplaceSelectedText, or a custom action (SKENE_COPY, SKENE_CUT, SKENE_PASTE) for the clipboard.
- node.rs, util.rs: GetTextAtOffset, GetTextBeforeOffset, GetTextAfterOffset (every
  TextBoundaryType), GetCharacterAtOffset, and sentence granularity in GetStringAtOffset.
- node.rs: the labelled-by, label-for, described-by and description-for relations; a
  description from the described-by nodes' text where a node has none of its own.
- node.rs: the expandable, expanded, has-popup and invalid-entry states.
- node.rs: a switch is a check box (checkable, checked while on), as GTK 4 reports GtkSwitch.
- node.rs: the exact role where AT-SPI's is shared (`switch`, `checkbox`, `option`,
  `listitem`) in the `xml-roles` attribute, as Chromium and Firefox report an ARIA role, and a
  heading's `level`: AT-SPI 2.52 has no switch role, and GTK 4 reports a row and an option
  alike as selectable list items.
- node.rs: a text entry and a text view (`TextInput`, `MultilineTextInput`) are text, as GTK 4
  reports GtkEntry and GtkTextView.
- node.rs: the first action's key binding is the node's keyboard shortcut (`key_binding`, and in
  `actions`).
- adapter.rs: a window gaining focus reports its focused object's focus before its own
  activation (`window:activate`), as GTK 4 reports its focused widget before its window's
  active state, so Orca presents the object in the window's context (`controls`, then `Name
  text`) rather than the bare window first (`controls frame`).
- Cargo.toml: unicode-segmentation for words and sentences.
- Cargo.toml: atspi-common 0.13 -> 0.14, the one atspi 0.30 builds on.

The licence files (LICENSE-MIT, LICENSE-APACHE, and LICENSE.chromium for the parts derived from Chromium) are copied from the AccessKit repository, since the published crate ships none.
