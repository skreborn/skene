//! Context menus: the list opens pinned at the press and prevents the platform's menu,
//! its first usable item takes focus, the keys walk the enabled items, an item runs
//! and closes it, a standard item follows the platform's clipboard, and the deepest menu
//! along the path wins.

use std::{cell::RefCell, rc::Rc};

use skene_common::{ContextMenu, MenuItem};
use skene_core::{
  Availability,
  ClipboardAction,
  ControlKind,
  Dimension,
  FocusOrder,
  HasChildren,
  KeyEvent,
  Modifiers,
  Part,
  Placement,
  Point,
  PointerButton,
  PointerEvent,
  Property,
  PropertyKey,
  Scene,
  View,
};
use skene_headless::{
  Headless,
  children,
  control_props,
  control_state,
  is_focused,
  is_open,
  overlay_position,
  overlay_props,
  part,
  reset,
  resolve_length,
  set_clipboard_availability,
  take_clipboard_actions,
  trigger_click,
  trigger_context_menu,
  trigger_dismiss,
  trigger_key_down,
};
use skene_reactive::{Get, Owner, Set, Signal};

type Container = skene_common::Container<Headless>;
type Text = skene_common::Text<Headless>;
type Menu<C = skene_common::NoChild> = ContextMenu<Headless, C>;

/// The served view and its list, mounted in the segment `start` begins.
fn parts(start: skene_core::NodeId) -> (skene_core::NodeId, skene_core::NodeId) {
  let nodes = Scene::<Headless>::with(|scene| scene.segment_nodes(start));

  assert_eq!(nodes.len(), 2, "the served view and its list: {nodes:?}");

  (nodes[0], nodes[1])
}

/// The menu item controls under `root`, in tree order.
fn items(root: skene_core::NodeId) -> Vec<skene_core::NodeId> {
  fn walk(scene: &Scene<Headless>, node: skene_core::NodeId, out: &mut Vec<skene_core::NodeId>) {
    out.push(node);

    for child in scene.children(node) {
      walk(scene, child, out);
    }
  }

  let mut nodes = Vec::new();

  Scene::<Headless>::with(|scene| walk(scene, root, &mut nodes));

  nodes
    .into_iter()
    .filter(|node| control_props(*node).is_some_and(|props| props.kind == ControlKind::MenuItem))
    .collect()
}

fn press(x: f32, y: f32) -> PointerEvent {
  PointerEvent {
    x,
    y,
    local_x: x,
    local_y: y,
    button: Some(PointerButton::Secondary),
    modifiers: Modifiers::default(),
  }
}

fn key(key: &str) -> KeyEvent {
  KeyEvent::new(key)
}

/// A secondary press on the served view opens the list pinned at the press with the
/// platform's menu prevented, the first item takes focus, and clicking an item runs
/// it and closes the list.
#[test]
fn a_context_menu_opens_at_the_press_and_runs_the_chosen_item() {
  reset();

  let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
  let owner = Owner::new();
  let start = owner.enter(|| {
    Menu::new()
      .set_items([
        MenuItem::new("Rename").set_shortcut("F2").handle_select({
          let log = Rc::clone(&log);

          move || log.borrow_mut().push("rename")
        }),
        MenuItem::separator(),
        MenuItem::new("Delete").handle_select({
          let log = Rc::clone(&log);

          move || log.borrow_mut().push("delete")
        }),
      ])
      .set_child(Container::new().set_child(Text::new("note")))
      .build()
  });
  let (served, list) = parts(start);
  let props = overlay_props(list).expect("a popup list");

  assert!(!props.modal && !props.hint);
  assert_eq!(part(list), Some(Part::List));
  assert!(!is_open(list));
  assert_eq!(items(list).len(), 2, "a separator is no item");

  let verdict = trigger_context_menu(served, &press(40., 12.));

  assert!(verdict.platform, "the platform's menu is prevented");
  assert!(verdict.bubbling, "nothing above sees the request");
  assert!(is_open(list));
  assert_eq!(overlay_position(list), Some(Point::new(40., 12.)));

  let entries = items(list);

  assert!(is_focused(entries[0]), "the first item takes focus");

  let _ = trigger_click(entries[1]);

  assert_eq!(*log.borrow(), vec!["delete"]);
  assert!(!is_open(list), "running an item closes the list");

  let _ = trigger_context_menu(served, &press(1., 1.));

  assert!(is_open(list));

  trigger_dismiss(list);

  assert!(!is_open(list), "a dismiss closes it");
}

/// The arrows walk the enabled items with wrapping, skipping a disabled one, `Home`
/// and `End` reach the ends, and `Enter` runs the focused item.
#[test]
fn the_keys_walk_the_enabled_items() {
  reset();

  let chosen = Signal::new(String::new());
  let owner = Owner::new();
  let start = owner.enter(|| {
    let item = |label: &'static str| {
      MenuItem::new(label).handle_select(move || chosen.set(label.to_owned()))
    };

    Menu::new()
      .set_items([item("one"), item("two").set_disabled(true), item("three")])
      .set_child(Container::new().set_focus_order(FocusOrder::Auto))
      .build()
  });
  let (served, list) = parts(start);
  let _ = trigger_context_menu(served, &press(0., 0.));
  let entries = items(list);

  assert!(control_state(entries[1]).is_some_and(|state| state.disabled));
  assert!(is_focused(entries[0]));

  let _ = trigger_key_down(entries[0], &key("ArrowDown"));

  assert!(is_focused(entries[2]), "the disabled item is skipped");

  let _ = trigger_key_down(entries[2], &key("ArrowDown"));

  assert!(is_focused(entries[0]), "the walk wraps");

  let _ = trigger_key_down(entries[0], &key("ArrowUp"));

  assert!(is_focused(entries[2]), "and wraps back");

  let _ = trigger_key_down(entries[2], &key("Home"));

  assert!(is_focused(entries[0]));

  let _ = trigger_key_down(entries[0], &key("End"));

  assert!(is_focused(entries[2]));

  let _ = trigger_key_down(entries[2], &key("Enter"));

  assert_eq!(chosen.get(), "three");
  assert!(!is_open(list));
}

/// A standard item is listed where the platform offers its action, disabled where the
/// action does not apply, and runs the action through the platform; a menu with nothing
/// usable does not open but still prevents the platform's.
#[test]
fn a_standard_item_follows_the_platforms_clipboard() {
  reset();

  let owner = Owner::new();
  let start = owner.enter(|| {
    Menu::new()
      .set_items([MenuItem::copy(), MenuItem::paste()])
      .set_child(Container::new())
      .build()
  });
  let (served, list) = parts(start);

  set_clipboard_availability(ClipboardAction::Paste, Availability::Absent);

  let _ = trigger_context_menu(served, &press(0., 0.));
  let entries = items(list);

  assert_eq!(entries.len(), 1, "an absent action is not listed");
  assert!(is_open(list));

  let _ = trigger_click(entries[0]);

  assert_eq!(take_clipboard_actions(), vec![ClipboardAction::Copy]);
  assert!(!is_open(list));

  set_clipboard_availability(ClipboardAction::Copy, Availability::Disabled);
  set_clipboard_availability(ClipboardAction::Paste, Availability::Enabled);

  let _ = trigger_context_menu(served, &press(0., 0.));
  let entries = items(list);

  assert_eq!(entries.len(), 2, "an offered action is listed again");
  assert!(control_state(entries[0]).is_some_and(|state| state.disabled));
  assert!(is_focused(entries[1]), "focus lands on the usable item");

  trigger_dismiss(list);
  set_clipboard_availability(ClipboardAction::Paste, Availability::Disabled);

  let verdict = trigger_context_menu(served, &press(0., 0.));

  assert!(verdict.platform, "the platform's menu is still prevented");
  assert!(!is_open(list), "nothing usable, nothing opens");
}

/// A menu serving an existing node hears a request from anywhere beneath it, and a
/// closer menu takes the request before it.
#[test]
fn the_deepest_menu_along_the_path_wins() {
  reset();

  let owner = Owner::new();
  let (root, plain, inner_start, outer_list) = owner.enter(|| {
    let inner = Menu::new()
      .set_items([MenuItem::new("inner")])
      .set_child(Container::new().set_child(Text::new("leaf")));
    let root = skene_common::Column::<Headless>::new()
      .set_children((Text::new("plain"), inner))
      .build();
    let (plain, inner_start) = Scene::<Headless>::with(|scene| {
      // A column's children sit in a segment of its own, behind its start anchor.
      let children: Vec<_> = scene.children(root).collect();

      (children[1], children[2])
    });
    let outer_list = Menu::new().set_items([MenuItem::new("outer")]).serve(root);

    (root, plain, inner_start, outer_list)
  });
  let (inner_served, inner_list) = parts(inner_start);

  assert_eq!(
    Scene::<Headless>::with(|scene| scene.parent(outer_list)),
    Some(root),
    "a served menu mounts its list under the node"
  );

  let _ = trigger_context_menu(plain, &press(0., 0.));

  assert!(is_open(outer_list), "the root's menu serves the plain run");
  assert!(!is_open(inner_list));

  trigger_dismiss(outer_list);

  let leaf =
    Scene::<Headless>::with(|scene| scene.children(inner_served).next().expect("the leaf"));
  let _ = trigger_context_menu(leaf, &press(0., 0.));

  assert!(is_open(inner_list), "the closer menu takes the request");
  assert!(!is_open(outer_list), "and the root's never hears it");
}

/// Whether a node at or under `node` is `height` tall.
fn holds_height(node: skene_core::NodeId, height: f32) -> bool {
  resolve_length(node, PropertyKey::Shared(Property::Height)) == Some(height)
    || children(node)
      .into_iter()
      .any(|child| holds_height(child, height))
}

/// The list opens with its top-start corner at the press unless the menu places it
/// elsewhere, and a separator is as thick as the menu says.
#[test]
fn a_context_menu_places_its_list_and_sizes_its_separators() {
  reset();

  let owner = Owner::new();
  let start = owner.enter(|| {
    Menu::new()
      .set_placement(Placement::BottomEnd)
      .set_separator_thickness(Dimension::Length(3.))
      .set_items([
        MenuItem::new("A"),
        MenuItem::separator(),
        MenuItem::new("B"),
      ])
      .set_child(Container::new())
      .build()
  });
  let (_, list) = parts(start);

  assert_eq!(
    overlay_props(list).map(|props| props.placement),
    Some(Placement::BottomEnd)
  );

  assert!(holds_height(list, 3.), "the separator is three units thick");

  owner.dispose();
}
