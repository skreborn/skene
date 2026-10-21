# Virtualized collections: expose the whole set, and let a reader reach an item that is not in the tree

Repository: AccessKit/accesskit. Checked against `main` at a39329d281a9381510496c17e81e87763edbb8bb
(2026-10-18) and against the published `accesskit` 0.25.1, `accesskit_consumer` 0.39.1 and
`accesskit_atspi_common` 0.21.0.

## Summary

A toolkit that virtualizes a long list (a lazy column, a recycler) puts only the rows near the
viewport into the AccessKit tree. AccessKit already carries what such a list needs to say about
the rest: `Node::set_size_of_set` on the container and `Node::set_position_in_set` on each item.
Two adapters map them (AT-SPI and UIA); three do not (Android, macOS, iOS). And no adapter offers a
reader a way to reach an item that is not in the tree yet, which each platform has a protocol for.

## What happens today

Labels: VERIFIED (ran) means observed in a running program; VERIFIED (read) means read in the
source at the commit above; JUDGMENT is not checked.

1. AT-SPI maps both properties. VERIFIED (ran): a tree of a `Role::List` with `size_of_set(1000)`
   holding 14 `Role::ListItem` children with `position_in_set(500..=513)`, served by
   `accesskit_unix` 0.24.0, reads over AT-SPI as 14 children whose object attributes are
   `posinset` 501..514 and `setsize` 1000 (`adapters/atspi-common/src/node.rs`,
   `position_in_set`, `size_of_set`, `attributes`). A reader's `Component.ScrollTo` on a row
   arrives as `Action::ScrollIntoView` with `ActionData::ScrollHint(TopEdge)`. Orca reads
   `posinset` and `setsize` in every toolkit from 47.0 on (VERIFIED (read):
   `src/orca/ax_utilities.py`, `get_position_in_set`, `get_set_size`, tags `ORCA_47_0` through
   `50.0` and `main`); Orca 46 reads them in its web script only.
2. UIA maps both (`adapters/windows/src/node.rs`, `UIA_PositionInSetPropertyId`,
   `UIA_SizeOfSetPropertyId`), and `ScrollItemPattern.ScrollIntoView` maps to
   `Action::ScrollIntoView`. VERIFIED (read).
3. Android maps neither. VERIFIED (read): `adapters/android/src` sets no
   `AccessibilityNodeInfo.CollectionInfo` or `CollectionItemInfo` anywhere, so TalkBack hears
   neither "in list, 1000 items" nor "item 501 of 1000" from an AccessKit app.
   `ACTION_SCROLL_FORWARD` and `ACTION_SCROLL_BACKWARD` do map, to `ScrollDown`/`ScrollUp` (or
   the horizontal pair, by `orientation`) with `ScrollUnit::Page` (`adapters/android/src/adapter.rs`).
4. macOS and iOS map neither. VERIFIED (read): no use of `size_of_set` or `position_in_set` in
   `adapters/macos/src` or `adapters/ios/src`. iOS reports `UIAccessibilityContainerType::List` for
   `Role::List` and `Role::ListBox`, and maps `accessibilityScroll:` to the scroll actions; macOS's
   `accessibilityRows` lists the items the tree holds. What VoiceOver needs to announce a position
   in a list it cannot see whole is JUDGMENT here (no Apple hardware was available).
5. No adapter can reach an item outside the tree. A reader that wants item 700 of a list whose
   tree holds items 500 to 513 has no action to ask for it. The platforms have one each:
   - UIA: `ItemContainerPattern.FindItemByProperty` returns a placeholder for a virtualized item,
     and `VirtualizedItemPattern.Realize` makes it real (JUDGMENT: Narrator uses these on WinUI's
     `ListView`).
   - Android: `AccessibilityAction.ACTION_SCROLL_TO_POSITION` (API 23) with row and column
     arguments, beside `CollectionInfo` (JUDGMENT: Compose's `LazyColumn` exposes it through its
     `scrollToIndex` semantics action).
   - AT-SPI has no such call; a reader moves by the rows the tree holds and asks them to scroll
     (`ScrollTo`), which works when the toolkit keeps at least one row realized beyond each edge of
     the viewport.

## What we propose

1. Android: set `CollectionInfo(rowCount = size_of_set, columnCount = 1, hierarchical = false,
   selectionMode)` on a container with `size_of_set` (and from `row_count`/`column_count` on a grid),
   and `CollectionItemInfo(rowIndex = position_in_set, rowSpan = 1, columnIndex = 0, columnSpan = 1,
   selected)` on its items. This is a mapping of properties AccessKit already has.
2. macOS and iOS: map the same two properties where AppKit and UIKit have a slot for them. We have
   not checked which attributes VoiceOver reads for a list's count and an item's index, and would
   welcome the maintainers' knowledge here.
3. A new action for items outside the tree, for example `Action::ScrollToItem` with
   `ActionData::ScrollToItem { row: usize, column: Option<usize> }` targeted at the container: the
   toolkit scrolls the item into view and puts it in the tree, and the adapter answers the
   platform's request once the update arrives (UIA's `Realize`, Android's
   `ACTION_SCROLL_TO_POSITION`). On UIA it would back `ItemContainerPattern` and
   `VirtualizedItemPattern` for containers with `size_of_set`, the placeholder carrying the index.

We are happy to send the Android mapping first, since it is self-contained, and to write the
action once its shape is agreed.

## Why it matters

A toolkit on AccessKit that virtualizes a long list meets this as soon as it does (Skene's lazy
lists are our case). Without the count, a reader
reports the realized rows as the whole list ("item 3 of 14" in a list of 1000); without a way to
reach an unrealized item, a reader's own navigation stops at the realized edge unless the toolkit
keeps a row beyond it.

## Reproducing the AT-SPI observation

A 60-line program: `accesskit` 0.25.1 and `accesskit_unix` 0.24.0 (feature `async-io`), an
activation handler returning a window holding a `Role::List` with `size_of_set(1000)` and 14
`Role::ListItem` children with labels, bounds, `position_in_set(500 + i)` and the `ScrollIntoView`
action; run under `dbus-run-session` with `at-spi-bus-launcher --launch-immediately`, and read
with `gi.repository.Atspi`: `get_child_count()` is 14, `get_attributes()` of child 0 is
`{"posinset": "501", "setsize": "1000"}`, and `scroll_to(Atspi.ScrollType.TOP_EDGE)` on child 3
delivers `ScrollIntoView` with `ScrollHint(TopEdge)` to the action handler.
