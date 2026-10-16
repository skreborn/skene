Skene's copy of `accesskit_unix` 0.24.0 (crates.io), carried until the changes are proven in
practice and offered upstream.

Skene's changes, each marked `Skene:` in the source:
- atspi/interfaces/editable_text.rs: InsertText, DeleteText, CopyText, CutText and PasteText
  through Skene's accesskit_atspi_common.
- atspi/interfaces/text.rs: GetTextAtOffset, GetTextBeforeOffset, GetTextAfterOffset and
  GetCharacterAtOffset.
- atspi/interfaces/action.rs: GetKeyBinding answers the node's keyboard shortcut.
