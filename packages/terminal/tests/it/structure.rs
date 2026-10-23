//! The scene's structure in cells: attaching, detaching, relocating and destroying
//! nodes and segments keep the scene's links whole and reach the frame and a reader in
//! the order the scene places them, a hidden subtree keeps its focus for its return, a
//! caption kept beside stands before its control, a segment's focus holder is in its
//! content and an overlay anchored on a segment is placed against its first box; reactive
//! structure: a derived subtree mounts only its content between its anchors and forwards its
//! annotation to each rebuild, derived children follow their source's shape and segments mount in
//! order, a derived list keeps its kept items and disposes a removed one's owner; a switch keeps
//! each branch it built, pauses a hidden one and annotates every branch its wrapper surrounds; a
//! show builds its child the first time it shows, keeps it in place, and hides and shows
//! the same node; a destroyed node holds no focus and no press; and a view given its
//! child still takes its setters.

use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use proptest::prelude::*;
use skene_foundation::{NodeKind, NodeRecord, Part};
use skene_simple_terminal::prelude::*;
use skene_terminal::{
  self as terminal,
  NodeId,
  Scene,
  TOOLTIP_DELAY,
  Terminal,
  prelude::*,
  testing,
};

use crate::common::{
  colors_at,
  column,
  is_live,
  red,
  rgb,
  rows,
  stage,
  text,
  white,
  with_platform,
};

/// Whether a reader finds `node` in the tree the turn ends with.
fn is_read(node: NodeId) -> bool {
  let id = node.access_id();

  testing::access_tree()
    .nodes
    .iter()
    .any(|(held, _)| *held == id)
}

/// The kinds of the nodes attached under `node`, in order.
fn child_kinds(node: NodeId) -> Vec<NodeKind> {
  Scene::<Terminal>::read(|scene| {
    scene
      .children(node)
      .filter_map(|child| scene.record(child).map(NodeRecord::kind))
      .collect()
  })
}

/// The run a reader finds reading `text`.
fn run(text: &str) -> NodeId {
  testing::find_by_text(text).unwrap_or_else(|| panic!("no run reads {text:?}"))
}

/// A row holding nothing, detached.
fn row() -> NodeId {
  Row::new().build()
}

/// A box taking focus around a run reading `label`, detached, bound to `handle`.
fn focusable(label: &'static str, handle: NodeHandle) -> NodeId {
  Container::new()
    .set_focus_order(FocusOrder::Auto)
    .bind_handle(handle)
    .set_child(Text::new(label))
    .build()
}

/// The frame's rows at 8 cells by `height`.
fn frame(height: u16) -> String {
  rows(&testing::render_to_text(8, height))
}

/// The nodes attached under `node`, in order.
fn children(node: NodeId) -> Vec<NodeId> {
  Scene::<Terminal>::read(|scene| scene.children(node).collect())
}

/// A container with a derived child holds its run between the derived segment's anchors
/// and nothing else, and a change builds a new run in place of the old one, which goes.
#[test]
fn a_derived_child_mounts_only_its_content_between_its_anchors() {
  with_platform(|| {
    let label = Signal::new("one".to_owned());
    let root = terminal::mount(move || {
      Container::new()
        .derive_child(move || Text::new(label.get()))
        .into_any()
        .build()
    });

    assert_eq!(testing::render_to_text(4, 1), "one\n");
    assert_eq!(
      child_kinds(root),
      [
        NodeKind::Anchor,
        NodeKind::Anchor,
        NodeKind::Run,
        NodeKind::Anchor,
        NodeKind::Anchor
      ],
      "the run alone between the anchors, which take no room"
    );

    let first = run("one");

    label.set("two".to_owned());

    assert_eq!(testing::render_to_text(4, 1), "two\n");
    assert_ne!(run("two"), first);
    assert!(!is_live(first), "the old run went");
    assert_eq!(child_kinds(root)[2], NodeKind::Run);
  });
}

/// A derived text rewrites its run's text without replacing the run.
#[test]
fn a_derived_text_rewrites_its_run_in_place() {
  with_platform(|| {
    let label = Signal::new("one".to_owned());

    let _ = terminal::mount(move || Text::derive(move || label.get()).into_any().build());

    assert_eq!(testing::render_to_text(4, 1), "one\n");

    let node = run("one");

    label.set("two".to_owned());

    assert_eq!(testing::render_to_text(4, 1), "two\n");
    assert_eq!(run("two"), node);
  });
}

/// A keyed item that leaves has its owner disposed, so a signal created while building
/// it is dead, and its run goes; kept items keep their runs, which follow the reorder.
#[test]
fn derived_children_dispose_a_removed_items_owner_and_keep_the_rest() {
  with_platform(|| {
    let items = Signal::new(vec![1, 2, 3]);
    let inner = Rc::new(Cell::new(None::<Signal<i32>>));

    let _ = terminal::mount({
      let inner = Rc::clone(&inner);

      move || {
        Column::new()
          .derive_children(
            move || items.get(),
            |item| *item,
            move |item| {
              if item == 2 {
                inner.set(Some(Signal::new(item)));
              }

              Text::new(item.to_string())
            },
          )
          .into_any()
          .build()
      }
    });

    assert_eq!(rows(&testing::render_to_text(2, 3)), "1\n2\n3\n");

    let (one, two, three) = (run("1"), run("2"), run("3"));
    let inner = inner.get().expect("the second item built");

    assert_eq!(inner.try_get(), Some(2));

    items.set(vec![3, 1]);

    assert_eq!(rows(&testing::render_to_text(2, 3)), "3\n1\n\n");
    assert_eq!(
      (run("3"), run("1")),
      (three, one),
      "the kept runs, reordered"
    );
    assert_eq!(
      inner.try_get(),
      None,
      "the removed item's owner is disposed"
    );
    assert!(!is_live(two));
  });
}

/// A list derived from a store's entries updates an item in place when its entry is
/// written, and follows a reorder and a removal of the `Vec` by the elements' keys.
#[test]
fn derived_entries_update_in_place_and_follow_a_reorder() {
  #[derive(Store, Clone)]
  struct Entry {
    id: u32,
    label: String,
  }

  with_platform(|| {
    let entries = Store::new(vec![
      Entry {
        id: 1,
        label: "a".to_owned(),
      },
      Entry {
        id: 2,
        label: "b".to_owned(),
      },
    ]);

    let _ = terminal::mount(move || {
      Column::new()
        .derive_entries(entries.keyed_by(|row| row.id), |row| {
          Text::derive(move || row.label().get())
        })
        .into_any()
        .build()
    });

    assert_eq!(rows(&testing::render_to_text(2, 2)), "a\nb\n");

    let (a, b) = (run("a"), run("b"));

    entries
      .keyed_by(|row| row.id)
      .key(1)
      .label()
      .set("A".to_owned());

    assert_eq!(rows(&testing::render_to_text(2, 2)), "A\nb\n");
    assert_eq!(run("A"), a, "the entry's run, rewritten in place");

    entries.update(|list| list.swap(0, 1));

    assert_eq!(rows(&testing::render_to_text(2, 2)), "b\nA\n");
    assert_eq!((run("b"), run("A")), (b, a));

    entries.update(|list| list.retain(|row| row.id != 1));

    assert_eq!(rows(&testing::render_to_text(2, 2)), "b\n\n");
    assert_eq!(run("b"), b);
    assert!(!is_live(a));
  });
}

/// Flipping away and back reattaches the same branch node with its signal intact, the
/// branch builds exactly once, an unknown key shows nothing, and the branch goes with
/// the switch's owner.
#[test]
fn switch_keeps_a_branch_and_its_state_across_a_flip() {
  with_platform(|| {
    let shown = Signal::new(true);
    let key = Signal::new("a");
    let builds = Rc::new(Cell::new(0));
    let local = Rc::new(Cell::new(None::<Signal<i32>>));

    let _ = terminal::mount({
      let builds = Rc::clone(&builds);
      let local = Rc::clone(&local);

      move || {
        derived(move || {
          let builds = Rc::clone(&builds);
          let local = Rc::clone(&local);

          if !shown.get() {
            return Text::new("gone").build();
          }

          OneOf::new()
            .derive_key(move || key.get())
            .add_case("a", move || {
              builds.set(builds.get() + 1);
              local.set(Some(Signal::new(1)));

              Container::new().set_child(Text::new("a"))
            })
            .add_case("b", || Text::new("b"))
            .into_any()
            .build()
        })
      }
    });
    let local = local.get().expect("the first branch built");

    assert_eq!(testing::render_to_text(4, 1), "a\n");
    assert_eq!(builds.get(), 1);

    let branch = testing::find_by_text("a").expect("the first branch");

    local.set(5);
    key.set("b");

    assert_eq!(testing::render_to_text(4, 1), "b\n");
    assert!(
      !is_read(branch) && is_live(branch),
      "the branch waits aside"
    );
    assert_eq!(local.try_get(), Some(5));

    key.set("missing");

    assert_eq!(testing::render_to_text(4, 1), "\n");

    key.set("a");

    assert_eq!(testing::render_to_text(4, 1), "a\n");
    assert_eq!(testing::find_by_text("a"), Some(branch));
    assert_eq!(builds.get(), 1);
    assert_eq!(local.try_get(), Some(5));

    key.set("b");
    shown.set(false);

    assert_eq!(testing::render_to_text(4, 1), "gone\n");
    assert!(!is_live(branch));
    assert_eq!(local.try_get(), None);
  });
}

/// A branch not shown pauses its effects and catches up once it shows again.
#[test]
fn a_hidden_branch_pauses_until_it_shows() {
  with_platform(|| {
    let key = Signal::new("a");
    let count = Signal::new(0);
    let reads = Rc::new(Cell::new(0));

    let _ = terminal::mount({
      let reads = Rc::clone(&reads);

      move || {
        OneOf::new()
          .derive_key(move || key.get())
          .add_case("a", move || {
            Text::derive(move || {
              reads.set(reads.get() + 1);

              format!("a {}", count.get())
            })
          })
          .add_case("b", || Text::new("b"))
          .into_any()
          .build()
      }
    });

    assert_eq!(testing::render_to_text(4, 1), "a 0\n");

    key.set("b");
    count.set(1);

    assert_eq!(testing::render_to_text(4, 1), "b\n");
    assert_eq!(reads.get(), 1, "a hidden branch does not update");

    key.set("a");

    assert_eq!(
      testing::render_to_text(4, 1),
      "a 1\n",
      "it catches up as it shows"
    );
    assert_eq!(reads.get(), 2);
  });
}

/// A wrapper around a switch annotates whichever branch is built, including one built
/// after a later flip.
#[test]
fn switch_forwards_the_pending_annotation_to_every_branch() {
  with_platform(|| {
    let key = Signal::new(1);

    let _ = terminal::mount(move || {
      Container::new()
        .set_height(4)
        .set_child(
          Column::new().add_child(
            Flexible::new()
              .set_flexibility(Flexibility::grow(1.))
              .set_child(
                OneOf::new()
                  .derive_key(move || key.get())
                  .add_case(1, || Container::new().set_background(red()))
                  .add_case(2, || Container::new().set_background(red())),
              ),
          ),
        )
        .into_any()
        .build()
    });
    let painted = || {
      (0..4)
        .filter(|row| colors_at(2, 4, 0, *row).1 == Some(rgb(red())))
        .count()
    };

    assert_eq!(
      painted(),
      4,
      "the first branch grows to the column's height"
    );

    key.set(2);

    assert_eq!(painted(), 4, "and so does the branch a flip builds");
  });
}

/// A child hidden from the start is built the first time it shows, and while hidden
/// its effects pause, catching up once, when it shows again.
#[test]
fn a_hidden_child_builds_when_first_shown_and_pauses_while_hidden() {
  with_platform(|| {
    let shown = Signal::new(false);
    let source = Signal::new(0);
    let builds = Rc::new(Cell::new(0));
    let runs = Rc::new(Cell::new(0));

    let _ = terminal::mount({
      let builds = Rc::clone(&builds);
      let runs = Rc::clone(&runs);

      move || {
        Visibility::new()
          .derive_shown(move || shown.get())
          .set_child(AnyView::new(move || {
            let runs = Rc::clone(&runs);

            builds.set(builds.get() + 1);

            Effect::render(move |_: Option<()>| {
              let _ = source.get();

              runs.set(runs.get() + 1);
            });

            Text::new("child").build()
          }))
          .into_any()
          .build()
      }
    });

    assert_eq!(testing::render_to_text(6, 1), "\n");
    assert_eq!(builds.get(), 0, "a hidden child is not built");

    shown.set(true);

    assert_eq!(testing::render_to_text(6, 1), "child\n");
    assert_eq!((builds.get(), runs.get()), (1, 1));

    shown.set(false);

    let _ = testing::render_to_text(6, 1);

    source.set(1);
    source.set(2);

    assert_eq!(testing::render_to_text(6, 1), "\n");
    assert_eq!(runs.get(), 1, "a hidden child's effects pause");

    shown.set(true);

    assert_eq!(testing::render_to_text(6, 1), "child\n");
    assert_eq!(
      (builds.get(), runs.get()),
      (1, 2),
      "and catch up once, when it shows"
    );
  });
}

/// A re-run that leaves the condition as it was keeps the child where it stands.
#[test]
fn an_unchanged_condition_keeps_the_child_where_it_stands() {
  with_platform(|| {
    let count = Signal::new(1);
    let builds = Rc::new(Cell::new(0));

    let _ = terminal::mount({
      let builds = Rc::clone(&builds);

      move || {
        Column::new()
          .set_children((
            Text::new("before"),
            Visibility::new()
              .derive_shown(move || count.get() > 0)
              .set_child(AnyView::new(move || {
                builds.set(builds.get() + 1);

                Text::new("child").build()
              })),
            Text::new("after"),
          ))
          .into_any()
          .build()
      }
    });

    assert_eq!(
      rows(&testing::render_to_text(6, 3)),
      "before\nchild\nafter\n"
    );

    let child = testing::find_by_text("child").expect("the child");
    let outline = testing::access_outline();

    count.set(2);

    assert_eq!(
      rows(&testing::render_to_text(6, 3)),
      "before\nchild\nafter\n"
    );
    assert_eq!(testing::find_by_text("child"), Some(child));
    assert_eq!(testing::access_outline(), outline);
    assert_eq!(builds.get(), 1);
  });
}

/// Hiding detaches the child and showing reattaches the same node in its place, never
/// rebuilding, its state kept; the child is destroyed with the show's owner.
#[test]
fn show_detaches_and_reattaches_the_same_node() {
  with_platform(|| {
    let alive = Signal::new(true);
    let shown = Signal::new(true);
    let local = Rc::new(Cell::new(None::<Signal<i32>>));

    let _ = terminal::mount({
      let local = Rc::clone(&local);

      move || {
        derived(move || {
          let local = Rc::clone(&local);

          if !alive.get() {
            return Text::new("gone").build();
          }

          Column::new()
            .set_children((
              Text::new("before"),
              Visibility::new()
                .derive_shown(move || shown.get())
                .set_child(AnyView::new(move || {
                  local.set(Some(Signal::new(1)));

                  Text::new("child").build()
                })),
              Text::new("after"),
            ))
            .into_any()
            .build()
        })
      }
    });
    let local = local.get().expect("the child built");

    assert_eq!(
      rows(&testing::render_to_text(6, 3)),
      "before\nchild\nafter\n"
    );

    let child = testing::find_by_text("child").expect("the child");

    local.set(5);
    shown.set(false);

    assert_eq!(rows(&testing::render_to_text(6, 3)), "before\nafter\n\n");
    assert!(!is_read(child) && is_live(child), "the child waits aside");

    shown.set(true);

    assert_eq!(
      rows(&testing::render_to_text(6, 3)),
      "before\nchild\nafter\n"
    );
    assert_eq!(testing::find_by_text("child"), Some(child));
    assert_eq!(local.try_get(), Some(5));

    shown.set(false);
    alive.set(false);

    assert_eq!(testing::render_to_text(6, 3), "gone\n\n\n");
    assert!(!is_live(child));
    assert_eq!(local.try_get(), None);
  });
}

/// A focused node holds focus and a press until it is destroyed, and the scene holds
/// neither after.
#[test]
fn a_destroyed_node_holds_no_focus() {
  with_platform(|| {
    let alive = Signal::new(true);
    let handle = NodeHandle::new();

    let _ = terminal::mount(move || {
      derived(move || {
        if alive.get() {
          Container::new()
            .set_focus_order(FocusOrder::Auto)
            .bind_handle(handle)
            .set_child(Text::new("box"))
            .into_any()
            .build()
        }
        else {
          Text::new("gone").build()
        }
      })
    });
    let _ = testing::render_to_text(4, 1);
    let node = handle.node().expect("the box built");

    let held = || Scene::<Terminal>::read(|scene| (scene.focused(), scene.pressed()));

    handle.focus();
    testing::trigger_mouse_press(0, 0);

    assert_eq!(testing::focused_node(), Some(node));
    assert!(
      matches!(held(), (Some(focused), Some(_)) if focused == node),
      "the box holds focus and the press"
    );

    alive.set(false);

    assert_eq!(testing::render_to_text(4, 1), "gone\n");
    assert!(!is_live(node));
    assert_eq!(testing::focused_node(), None);
    assert_eq!(held(), (None, None), "the scene holds neither");
  });
}

/// A view given its child takes its setters still: an overlay opened after its content
/// is set opens, and a tooltip whose delay is shortened after its described view is set
/// opens after the shorter delay.
#[test]
fn a_view_given_its_child_takes_its_setters() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      let overlay = Overlay::popup().set_child(Text::new("content"));
      let tooltip = Tooltip::new("A tip").set_child(Text::new("described"));

      Column::new()
        .set_children((tooltip.set_delay(TOOLTIP_DELAY / 4), overlay.set_open(true)))
        .into_any()
        .build()
    });

    let frame = testing::render_to_text(20, 6);

    assert!(
      frame.contains("content"),
      "set_open after set_child opened it: {frame}"
    );

    let (x, y) = crate::common::locate(&frame, "described");

    testing::trigger_mouse_move(x, y);

    let _ = testing::render_to_text(20, 6);

    testing::trigger_frame(TOOLTIP_DELAY / 2);

    let frame = testing::render_to_text(20, 6);

    assert!(
      frame.contains("A tip"),
      "set_delay after set_child shortened the delay: {frame}"
    );
  });
}

/// A node waiting in a detached segment and attached elsewhere leaves the segment: the
/// segment's destruction takes it no more.
#[test]
fn attaching_a_waiting_node_takes_it_from_its_segment() {
  with_platform(|| {
    let (stage, [parent, node]) = stage(|| [column(), text("a")]);
    let (start, _) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, parent, None);
      scene.insert_in_segment(start, node, None);
      scene.attach(parent, node, None);
      scene.destroy(start);
    });

    assert!(is_live(node));
    assert_eq!(children(parent), [node]);
    assert_eq!(frame(2), "a\n\n");
  });
}

/// Removing a node from a segment that does not hold it leaves it to the one that
/// does, which gives it up once it is attached elsewhere.
#[test]
fn removing_from_another_segment_changes_nothing() {
  with_platform(|| {
    let (stage, [elsewhere, node]) = stage(|| [column(), text("a")]);
    let (holding, _) = Scene::<Terminal>::create_segment();
    let (other, _) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, elsewhere, None);
      scene.insert_in_segment(holding, node, None);
      scene.remove_from_segment(other, node);
    });

    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.segment_nodes(holding)),
      [node]
    );

    Scene::<Terminal>::with(|scene| {
      scene.attach(elsewhere, node, None);
      scene.destroy(holding);
    });

    assert!(is_live(node));
    assert_eq!(children(elsewhere), [node]);
    assert_eq!(frame(2), "a\n\n");
  });
}

/// A node standing between a segment's anchors is its content: hidden with it, and
/// taken from it when attached elsewhere.
#[test]
fn what_stands_between_the_anchors_is_the_segments() {
  with_platform(|| {
    let (stage, [parent, elsewhere, node]) = stage(|| [column(), column(), text("a")]);
    let (start, end) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, parent, None);
      scene.attach(stage, elsewhere, None);
      scene.attach(parent, start, None);
      scene.attach(parent, node, Some(end));
    });

    assert_eq!(frame(2), "a\n\n");

    Scene::<Terminal>::with(|scene| scene.detach(parent, start));

    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.segment_nodes(start)),
      [node]
    );
    assert_eq!(frame(2), "\n\n", "hidden with its segment");
    assert!(!is_read(node));

    Scene::<Terminal>::with(|scene| {
      scene.attach(elsewhere, node, None);
      scene.destroy(start);
    });

    assert!(is_live(node));
    assert_eq!(children(elsewhere), [node]);
    assert_eq!(frame(2), "a\n\n");
  });
}

/// Attaching to a destroyed parent, or attaching a destroyed child, changes nothing:
/// the child stays where it stood, and the platform is told nothing.
#[test]
fn attach_needs_both_nodes_live() {
  with_platform(|| {
    let (stage, [parent, gone, child]) = stage(|| [column(), column(), text("a")]);

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, parent, None);
      scene.attach(parent, child, None);
      scene.destroy(gone);
    });

    assert_eq!(frame(2), "a\n\n");
    assert!(!testing::is_frame_pending());

    Scene::<Terminal>::with(|scene| {
      scene.attach(gone, child, None);
      scene.attach(parent, gone, None);
    });

    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.parent(child)),
      Some(parent)
    );
    assert_eq!(children(parent), [child]);
    assert!(!testing::is_frame_pending(), "nothing to draw anew");
  });
}

/// Attaching an attached node moves it from its previous parent, in the scene and in
/// the frame.
#[test]
fn attach_unlinks_the_previous_parent() {
  with_platform(|| {
    let (stage, [first, second, one, two, child]) =
      stage(|| [column(), column(), text("1"), text("2"), text("c")]);

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, first, None);
      scene.attach(stage, second, None);
      scene.attach(first, one, None);
      scene.attach(second, two, None);
      scene.attach(first, child, None);
    });

    assert_eq!(frame(3), "1\nc\n2\n");

    Scene::<Terminal>::with(|scene| scene.attach(second, child, None));

    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.parent(child)),
      Some(second)
    );
    assert_eq!(children(first), [one]);
    assert_eq!(children(second), [two, child]);
    assert_eq!(frame(3), "1\n2\nc\n");
  });
}

/// `before` places a child ahead of a sibling, in the scene and in the frame.
#[test]
fn attach_before_orders_the_children() {
  with_platform(|| {
    let (stage, [first, second]) = stage(|| [text("a"), text("b")]);

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, second, None);
      scene.attach(stage, first, Some(second));
    });

    assert_eq!(children(stage), [first, second]);
    assert_eq!(frame(2), "a\nb\n");
  });
}

/// Attaching a node where it already stands tells the platform nothing, and moving it
/// does.
#[test]
fn attach_in_place_is_a_no_op() {
  with_platform(|| {
    let (stage, [first, second]) = stage(|| [text("a"), text("b")]);

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, first, None);
      scene.attach(stage, second, None);
    });

    assert_eq!(frame(2), "a\nb\n");

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, first, Some(second));
      scene.attach(stage, second, None);
    });

    assert!(!testing::is_frame_pending(), "nothing to draw anew");

    Scene::<Terminal>::with(|scene| scene.attach(stage, first, None));

    assert!(testing::is_frame_pending());
    assert_eq!(children(stage), [second, first]);
    assert_eq!(frame(2), "b\na\n");
  });
}

/// Detaching a node hides it, no root while hidden, and attaching it again gives it
/// back from nowhere.
#[test]
fn detach_hides_and_attach_gives_back() {
  with_platform(|| {
    let (stage, [child]) = stage(|| [text("a")]);

    Scene::<Terminal>::with(|scene| scene.attach(stage, child, None));

    assert_eq!(frame(1), "a\n");

    Scene::<Terminal>::with(|scene| scene.detach(stage, child));

    assert_eq!(frame(1), "\n");
    assert!(is_live(child) && !is_read(child), "it waits aside");
    assert!(
      !Scene::<Terminal>::read(|scene| scene.roots().any(|root| root == child)),
      "a hidden node is no root"
    );

    Scene::<Terminal>::with(|scene| scene.attach(stage, child, None));

    assert_eq!(frame(1), "a\n");
    assert!(is_read(child));
  });
}

/// Moving a segment relocates its start, its content and its end as one run, in
/// order.
#[test]
fn moving_a_segment_relocates_its_run_in_order() {
  with_platform(|| {
    let (stage, [first, second, two, a, b]) =
      stage(|| [column(), column(), text("2"), text("a"), text("b")]);
    let (start, end) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, first, None);
      scene.attach(stage, second, None);
      scene.attach(second, two, None);
      scene.attach(first, start, None);
      scene.insert_in_segment(start, a, None);
      scene.insert_in_segment(start, b, None);
    });

    assert_eq!(frame(3), "a\nb\n2\n");

    Scene::<Terminal>::with(|scene| scene.attach(second, start, None));

    assert_eq!(children(first), []);
    assert_eq!(children(second), [two, start, a, b, end]);
    assert_eq!(frame(3), "2\na\nb\n");
  });
}

/// Hiding a subtree holding focus clears it, and attaching the subtree again gives it
/// back where focus has gone nowhere since.
#[test]
fn a_hidden_subtree_keeps_its_focus_for_its_return() {
  with_platform(|| {
    let (field_handle, other_handle) = (NodeHandle::new(), NodeHandle::new());
    let (stage, [panel, field, other]) = stage(|| {
      [
        column(),
        focusable("f", field_handle),
        focusable("o", other_handle),
      ]
    });
    let focused = || Scene::<Terminal>::read(Scene::focused);

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, panel, None);
      scene.attach(panel, field, None);
      scene.attach(stage, other, None);
    });

    assert_eq!(frame(2), "f\no\n");

    field_handle.focus();

    assert_eq!(testing::focused_node(), Some(field));

    Scene::<Terminal>::with(|scene| scene.detach(stage, panel));

    assert_eq!(frame(2), "o\n\n");
    assert_eq!(focused(), None);
    assert_eq!(testing::focused_node(), None);

    Scene::<Terminal>::with(|scene| scene.attach(stage, panel, None));

    assert_eq!(frame(2), "o\nf\n");
    assert_eq!(testing::focused_node(), Some(field), "focus comes back");
    assert_eq!(focused(), Some(field));

    Scene::<Terminal>::with(|scene| scene.detach(stage, panel));
    other_handle.focus();
    Scene::<Terminal>::with(|scene| scene.attach(stage, panel, None));

    assert_eq!(
      testing::focused_node(),
      Some(other),
      "focus went elsewhere since"
    );
    assert_eq!(focused(), Some(other));
  });
}

/// A caption kept beside its control is placed before the control among its
/// siblings, not inside it, where one kept inside is placed where it stands.
#[test]
fn a_caption_kept_beside_is_placed_before_its_control() {
  with_platform(|| {
    let (stage, [inside, inside_caption, inside_track, beside, caption, track]) =
      stage(|| [row(), text("ci"), text("ti"), row(), text("cb"), text("tb")]);
    let placed = |node: NodeId| {
      Scene::<Terminal>::read(|scene| scene.placed_children(node).collect::<Vec<_>>())
    };

    Scene::<Terminal>::with(|scene| {
      for (control, label, rest) in [
        (inside, inside_caption, inside_track),
        (beside, caption, track),
      ] {
        scene.attach(control, label, None);
        scene.attach(control, rest, None);
        scene.set_part(label, Part::Label);
      }

      scene.keep_caption_beside(beside);
      scene.attach(stage, inside, None);
      scene.attach(stage, beside, None);
    });

    assert_eq!(placed(stage), [inside, caption, beside]);
    assert_eq!(placed(inside), [inside_caption, inside_track]);
    assert_eq!(placed(beside), [track]);
    assert_eq!(
      Scene::<Terminal>::read(|scene| (scene.placed_parent(caption), scene.placed_parent(track))),
      (Some(stage), Some(beside))
    );
    assert_eq!(frame(3), "citi\ncb\ntb\n");
  });
}

/// Destroying a node takes its whole subtree, which the frame and a reader lose, and
/// retires every id.
#[test]
fn destroy_takes_the_subtree_and_releases_every_record() {
  with_platform(|| {
    let (stage, [parent, child, grandchild]) = stage(|| [column(), column(), text("g")]);

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, parent, None);
      scene.attach(parent, child, None);
      scene.attach(child, grandchild, None);
    });

    assert_eq!(frame(1), "g\n");

    Scene::<Terminal>::with(|scene| scene.destroy(parent));

    assert_eq!(frame(1), "\n");
    assert_eq!(children(stage), []);
    assert!(
      [parent, child, grandchild]
        .into_iter()
        .all(|node| !is_live(node))
    );
    assert_eq!(testing::find_by_text("g"), None);

    let reused = text("r");

    assert_eq!(reused.index(), grandchild.index());
    assert_ne!(reused, grandchild);
    assert!(!is_live(grandchild));
  });
}

/// A node destroyed while it waits in a detached segment leaves the segment.
#[test]
fn a_destroyed_node_leaves_the_detached_segment_it_waits_in() {
  with_platform(|| {
    let (stage, [first, second]) = stage(|| [text("a"), text("b")]);
    let (start, _) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.insert_in_segment(start, first, None);
      scene.insert_in_segment(start, second, None);
      scene.destroy(first);
    });

    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.segment_nodes(start)),
      [second]
    );

    Scene::<Terminal>::with(|scene| scene.attach(stage, start, None));

    assert_eq!(frame(2), "b\n\n");
  });
}

/// A segment attaches and detaches with its content and its end anchor.
#[test]
fn segments_travel_with_their_content() {
  with_platform(|| {
    let (stage, [parent, other, one, content]) =
      stage(|| [column(), column(), text("1"), text("c")]);
    let (start, end) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, parent, None);
      scene.attach(stage, other, None);
      scene.attach(parent, one, None);
      scene.insert_in_segment(start, content, None);
    });

    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.segment_nodes(start)),
      [content]
    );
    assert_eq!(Scene::<Terminal>::read(|scene| scene.parent(content)), None);
    assert_eq!(frame(2), "1\n\n");

    Scene::<Terminal>::with(|scene| scene.attach(other, start, None));

    assert_eq!(children(other), [start, content, end]);
    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.record(start).map(NodeRecord::kind)),
      Some(NodeKind::Anchor)
    );
    assert_eq!(frame(2), "1\nc\n");

    Scene::<Terminal>::with(|scene| scene.attach(parent, start, Some(one)));

    assert_eq!(children(other), []);
    assert_eq!(children(parent), [start, content, end, one]);
    assert_eq!(frame(2), "c\n1\n");

    Scene::<Terminal>::with(|scene| scene.detach(parent, start));

    assert_eq!(children(parent), [one]);
    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.segment_nodes(start)),
      [content]
    );
    assert_eq!(frame(2), "1\n\n");

    Scene::<Terminal>::with(|scene| scene.destroy(start));

    assert!([start, content, end].into_iter().all(|node| !is_live(node)));
  });
}

/// A segment nested in a detached segment keeps its own content pending, so a
/// removal from it while detached holds when the outer segment is reattached.
#[test]
fn nested_segments_keep_their_own_content_while_detached() {
  with_platform(|| {
    let (stage, [before, inner_content]) = stage(|| [text("b"), text("i")]);
    let (outer, outer_end) = Scene::<Terminal>::create_segment();
    let (inner, inner_end) = Scene::<Terminal>::create_segment();
    let segment_nodes = |start: NodeId| Scene::<Terminal>::read(|scene| scene.segment_nodes(start));

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, outer, None);
      scene.insert_in_segment(outer, before, None);
      scene.insert_in_segment(outer, inner, None);
      scene.insert_in_segment(inner, inner_content, None);
    });

    assert_eq!(
      children(stage),
      [outer, before, inner, inner_content, inner_end, outer_end]
    );
    assert_eq!(segment_nodes(outer), [before, inner]);
    assert_eq!(frame(2), "b\ni\n");

    Scene::<Terminal>::with(|scene| scene.detach(stage, outer));

    assert_eq!(segment_nodes(outer), [before, inner]);
    assert_eq!(segment_nodes(inner), [inner_content]);
    assert_eq!(frame(2), "\n\n");

    Scene::<Terminal>::with(|scene| {
      scene.remove_from_segment(inner, inner_content);
      scene.attach(stage, outer, None);
    });

    assert_eq!(
      children(stage),
      [outer, before, inner, inner_end, outer_end]
    );
    assert!(is_live(inner_content));
    assert_eq!(frame(2), "b\n\n");

    Scene::<Terminal>::with(|scene| scene.insert_in_segment(inner, inner_content, None));

    assert_eq!(
      children(stage),
      [outer, before, inner, inner_content, inner_end, outer_end]
    );
    assert_eq!(frame(2), "b\ni\n");
  });
}

/// A derived subtree contributes its content directly to its parent's children, with
/// nothing between its anchors but the content, and rebuilds in place.
#[test]
fn derived_mounts_only_its_content_between_the_anchors() {
  with_platform(|| {
    let version = Signal::new(0);
    let builds = Rc::new(Cell::new(0));
    let (parent, [start]) = stage(|| {
      let builds = Rc::clone(&builds);

      [derived(move || {
        builds.set(builds.get() + 1);

        Text::new(format!("v{}", version.get())).build()
      })]
    });

    Scene::<Terminal>::with(|scene| scene.attach(parent, start, None));

    let end = Scene::<Terminal>::read(|scene| scene.segment_end(start)).expect("its end");
    let first = run("v0");

    assert_eq!(children(parent), [start, first, end]);
    assert_eq!(
      child_kinds(parent),
      [NodeKind::Anchor, NodeKind::Run, NodeKind::Anchor]
    );
    assert_eq!(builds.get(), 1);
    assert_eq!(testing::render_to_text(4, 1), "v0\n");

    version.set(1);

    let second = run("v1");

    assert_eq!(builds.get(), 2);
    assert_ne!(second, first);
    assert_eq!(children(parent), [start, second, end]);
    assert!(!is_live(first), "the old content went");
    assert_eq!(testing::render_to_text(4, 1), "v1\n");
  });
}

/// The annotation pending when a derived subtree is created lands on the content of
/// every rebuild, and is pending no more.
#[test]
fn derived_forwards_the_pending_annotation_to_each_rebuild() {
  with_platform(|| {
    let version = Signal::new(0);
    let content = NodeHandle::new();

    let _ = terminal::mount(move || {
      Container::new()
        .set_height(3)
        .set_child(
          Column::new().set_children((
            Flexible::new()
              .set_flexibility(Flexibility::grow(2.))
              .set_child(AnyView::new(move || {
                derived(move || {
                  let _ = version.get();

                  Container::new()
                    .bind_handle(content)
                    .set_background(red())
                    .build()
                })
              })),
            Flexible::new()
              .set_flexibility(Flexibility::grow(1.))
              .set_child(Container::new().set_background(white())),
          )),
        )
        .into_any()
        .build()
    });
    let painted = || {
      (0..3)
        .map(|row| colors_at(2, 3, 0, row).1)
        .collect::<Vec<_>>()
    };
    let expected = [Some(rgb(red())), Some(rgb(red())), Some(rgb(white()))];

    assert_eq!(painted(), expected, "the content takes two thirds");

    let first = content.node();

    version.set(1);

    assert_eq!(
      painted(),
      expected,
      "and so does the content a rebuild builds"
    );
    assert_ne!(content.node(), first, "a rebuild");
    assert!(
      Scene::<Terminal>::with(Scene::take_pending_annotation).is_none(),
      "nothing is left pending"
    );
  });
}

/// A derived child rebuilds when a signal it reads changes, its segment's anchors kept.
#[test]
fn derive_child_rebuilds_on_change() {
  with_platform(|| {
    let version = Signal::new(0);
    let parent = terminal::mount(move || {
      Column::new()
        .derive_child(move || Text::new(format!("v{}", version.get())))
        .into_any()
        .build()
    });
    let first = children(parent);

    assert_eq!(
      child_kinds(parent),
      [
        NodeKind::Anchor,
        NodeKind::Anchor,
        NodeKind::Run,
        NodeKind::Anchor,
        NodeKind::Anchor
      ]
    );

    version.set(1);

    assert_eq!(testing::render_to_text(4, 1), "v1\n");

    let second = children(parent);

    assert_ne!(second[2], first[2]);
    assert_eq!(second[..2], first[..2], "the starts kept");
    assert_eq!(second[3..], first[3..], "the ends kept");
    assert!(!is_live(first[2]));
  });
}

/// Children derived from a keyed store location follow its shape: an entry written
/// beneath rebuilds nothing, an insertion builds one child, a removal destroys one.
#[test]
fn derive_entries_follow_the_locations_shape() {
  with_platform(|| {
    let store = Store::new(BTreeMap::from([(1, "a"), (2, "b")]));
    let builds = Rc::new(Cell::new(0));
    let parent = terminal::mount({
      let builds = Rc::clone(&builds);

      move || {
        Column::new()
          .derive_entries(store, move |entry| {
            builds.set(builds.get() + 1);

            Text::derive(move || entry.get().to_owned())
          })
          .into_any()
          .build()
      }
    });
    let first = children(parent);

    assert_eq!(frame(3), "a\nb\n\n");
    assert_eq!((builds.get(), first.len()), (2, 4));

    store.key(1).set("A");

    assert_eq!(frame(3), "A\nb\n\n");
    assert_eq!((builds.get(), children(parent)), (2, first.clone()));

    store.update(|entries| {
      entries.insert(3, "c");
    });

    assert_eq!(frame(3), "A\nb\nc\n");

    let second = children(parent);

    assert_eq!((builds.get(), second.len()), (3, 5));
    assert_eq!(second[..3], first[..3]);

    store.update(|entries| {
      entries.remove(&1);
    });

    assert_eq!(frame(3), "b\nc\n\n");
    assert_eq!(
      (builds.get(), children(parent)),
      (3, vec![first[0], first[2], second[3], first[3]])
    );
    assert!(!is_live(first[1]));
  });
}

/// Segments mount in order: a static child, a keyed list and a static pair under one
/// parent, the list growing in place between them.
#[test]
fn segments_mount_in_order() {
  with_platform(|| {
    let items = Signal::new(vec![1, 2]);
    let parent = terminal::mount(move || {
      Column::new()
        .add_child(Text::new("a"))
        .derive_children(
          move || items.get(),
          |item| *item,
          |item| Text::new(item.to_string()),
        )
        .set_children((Text::new("y"), Text::new("z")))
        .into_any()
        .build()
    });
    let kinds = |count: usize| {
      let mut kinds = vec![
        NodeKind::Anchor,
        NodeKind::Run,
        NodeKind::Anchor,
        NodeKind::Anchor,
      ];

      kinds.extend(std::iter::repeat_n(NodeKind::Run, count));
      kinds.extend([
        NodeKind::Anchor,
        NodeKind::Anchor,
        NodeKind::Run,
        NodeKind::Run,
        NodeKind::Anchor,
      ]);

      kinds
    };

    assert_eq!(child_kinds(parent), kinds(2));
    assert_eq!(frame(6), "a\n1\n2\ny\nz\n\n");

    items.set(vec![3, 1, 2]);

    assert_eq!(frame(6), "a\n3\n1\n2\ny\nz\n");
    assert_eq!(child_kinds(parent), kinds(3));
  });
}

/// The node taking focus for a segment is in the segment's content, and the content's
/// enclosing segment is that segment.
#[test]
fn the_focus_holder_of_a_segment_is_in_its_content() {
  with_platform(|| {
    let handle = NodeHandle::new();
    let (stage, [plain, holder]) = stage(|| [text("p"), focusable("f", handle)]);
    let (start, _) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, start, None);
      scene.insert_in_segment(start, plain, None);
      scene.insert_in_segment(start, holder, None);
    });

    assert_eq!(
      Scene::<Terminal>::read(|scene| (
        scene.focus_holder_within(start),
        scene.enclosing_segment(holder)
      )),
      (holder, Some(start))
    );

    testing::trigger_key("Tab");

    assert_eq!(
      testing::focused_node(),
      Some(holder),
      "the tab stop it holds"
    );
  });
}

/// An overlay anchored on a segment or a scope is placed against the first node inside
/// it with a box of its own, an empty segment standing for none.
#[test]
fn a_segment_is_placed_against_its_first_box() {
  with_platform(|| {
    let (stage, [scope, after]) = stage(|| {
      [
        Theme::new(SimplePalette::default())
          .set_child_builder(|| Text::new("inside"))
          .build(),
        text("after"),
      ]
    });
    let inside = run("inside");
    let (outer, _) = Scene::<Terminal>::create_segment();
    let (empty, _) = Scene::<Terminal>::create_segment();

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, outer, None);
      scene.insert_in_segment(outer, empty, None);
      scene.insert_in_segment(outer, scope, None);
      scene.insert_in_segment(outer, after, None);
    });

    assert_eq!(
      Scene::<Terminal>::read(|scene| scene.record(scope).map(NodeRecord::kind)),
      Some(NodeKind::Scope)
    );
    assert_eq!(
      Scene::<Terminal>::read(
        |scene| [outer, scope, empty, after].map(|node| scene.placement(node))
      ),
      [Some(inside), Some(inside), None, Some(after)]
    );
    assert_eq!(frame(2), "inside\nafter\n");
  });
}

/// One structural change a platform's views make to the scene, over indices into a
/// pool of boxes and segments.
#[derive(Debug, Clone)]
enum Change {
  Attach(usize, usize, Option<usize>),
  Detach(usize, usize),
  Insert(usize, usize, Option<usize>),
  Remove(usize, usize),
  Destroy(usize),
}

/// The most links a walk of the pool follows before it is taken for a cycle.
const WALK: usize = 64;

/// The children of `node`, at most [`WALK`] of them.
fn bounded_children(scene: &Scene<Terminal>, node: NodeId) -> Vec<NodeId> {
  scene.children(node).take(WALK).collect()
}

/// Whether `ancestor` is `node` or stands above it, at most [`WALK`] links up.
fn stands_above(scene: &Scene<Terminal>, ancestor: NodeId, node: NodeId) -> bool {
  let mut current = Some(node);

  for _ in 0..WALK {
    let Some(visited) = current
    else {
      return false;
    };

    if visited == ancestor {
      return true;
    }

    current = scene.parent(visited);
  }

  true
}

/// The nodes that move with `node`: a segment start's content and end anchor with it,
/// and a pending content's nested ones, following at most `depth` pending segments
/// down.
fn moving(scene: &Scene<Terminal>, node: NodeId, depth: usize) -> Vec<NodeId> {
  let Some(end) = scene.segment_end(node).filter(|_| depth > 0)
  else {
    return vec![node];
  };

  if let Some(parent) = scene.parent(node) {
    return bounded_children(scene, parent)
      .into_iter()
      .skip_while(|child| *child != node)
      .take_while(|child| *child != end)
      .chain([end])
      .collect();
  }

  [node]
    .into_iter()
    .chain(
      scene
        .segment_nodes(node)
        .into_iter()
        .flat_map(|pending| moving(scene, pending, depth - 1)),
    )
    .chain([end])
    .collect()
}

/// Every link the scene keeps agreeing with every other: each live node stands once
/// among its parent's children, every child is live and names its parent, the sibling
/// links run both ways and end, no node stands under itself, and an attached segment's
/// end stands after its start under the same parent.
fn assert_whole(scene: &Scene<Terminal>, nodes: &[NodeId]) {
  for &node in nodes {
    if !scene.contains(node) {
      continue;
    }

    let mut previous = None;
    let mut count = 0;

    for child in scene.children(node) {
      assert!(scene.contains(child), "a child is live");
      assert_eq!(scene.parent(child), Some(node), "a child names its parent");
      assert_eq!(
        scene.previous_sibling(child),
        previous,
        "siblings link both ways"
      );

      if let Some(previous) = previous {
        assert_eq!(scene.next_sibling(previous), Some(child));
      }

      previous = Some(child);
      count += 1;

      assert!(count <= WALK, "the children end");
    }

    if let Some(last) = previous {
      assert_eq!(scene.next_sibling(last), None, "the last child is the last");
    }

    if let Some(parent) = scene.parent(node) {
      assert_eq!(
        bounded_children(scene, parent)
          .into_iter()
          .filter(|child| *child == node)
          .count(),
        1,
        "a node stands once among its parent's children"
      );
    }

    let mut ancestor = scene.parent(node);
    let mut depth = 0;

    while let Some(visited) = ancestor {
      assert_ne!(visited, node, "no node stands under itself");
      depth += 1;

      assert!(depth <= WALK, "the ancestry ends");

      ancestor = scene.parent(visited);
    }

    if let Some(end) = scene.segment_end(node) {
      assert!(scene.contains(end), "a live start has a live end");
      assert_eq!(
        scene.parent(end),
        scene.parent(node),
        "an end stands with its start"
      );

      if let Some(parent) = scene.parent(node) {
        let order = bounded_children(scene, parent);
        let start = order.iter().position(|child| *child == node);
        let end = order.iter().position(|child| *child == end);

        assert!(start < end, "an end stands after its start");
      }
    }
  }
}

/// The labels of the runs placed beneath `node`, in document order.
fn placed_labels(scene: &Scene<Terminal>, node: NodeId, labels: &[(NodeId, char)]) -> String {
  scene
    .placed_children(node)
    .map(|child| {
      labels.iter().find(|(run, _)| *run == child).map_or_else(
        || placed_labels(scene, child, labels),
        |(_, label)| label.to_string(),
      )
    })
    .collect()
}

/// The labels of the runs a reader finds beneath `node` in `tree`, in tree order.
fn read_labels(
  tree: &accesskit::TreeUpdate,
  node: accesskit::NodeId,
  labels: &[(NodeId, char)],
) -> String {
  let Some((_, access)) = tree.nodes.iter().find(|(id, _)| *id == node)
  else {
    return String::new();
  };

  access
    .children()
    .iter()
    .map(|child| {
      labels
        .iter()
        .find(|(run, _)| run.access_id() == *child)
        .map_or_else(
          || read_labels(tree, *child, labels),
          |(_, label)| label.to_string(),
        )
    })
    .collect()
}

proptest! {
  /// Any run of attaching, detaching, mounting in and out of segments and destroying
  /// keeps the scene's links whole, and after every change the terminal draws and
  /// reports its runs in the order the scene places them.
  #[test]
  fn any_run_of_changes_keeps_the_scene_whole(
    changes in proptest::collection::vec(
      prop_oneof![
        4 => (0..9_usize, 0..9_usize, proptest::option::of(0..9_usize))
          .prop_map(|(parent, child, before)| Change::Attach(parent, child, before)),
        2 => (0..9_usize, 0..9_usize).prop_map(|(parent, child)| Change::Detach(parent, child)),
        3 => (0..2_usize, 0..9_usize, proptest::option::of(0..9_usize))
          .prop_map(|(segment, node, before)| Change::Insert(segment, node, before)),
        2 => (0..2_usize, 0..9_usize).prop_map(|(segment, node)| Change::Remove(segment, node)),
        1 => (0..9_usize).prop_map(Change::Destroy),
      ],
      0..30,
    ),
  ) {
    with_platform(|| {
      let (stage, boxes) = stage(|| std::array::from_fn::<_, 5, _>(|_| column()));
      let labels: Vec<(NodeId, char)> = boxes
        .iter()
        .zip('0'..)
        .map(|(column, label)| {
          let run = Text::new(label.to_string()).build();

          Scene::<Terminal>::with(|scene| scene.attach(*column, run, None));

          (run, label)
        })
        .collect();
      let segments = [
        Scene::<Terminal>::create_segment(),
        Scene::<Terminal>::create_segment(),
      ];
      let pool: Vec<NodeId> = boxes
        .iter()
        .copied()
        .chain(segments.iter().map(|(start, _)| *start))
        .chain(segments.iter().map(|(_, end)| *end))
        .collect();
      let mut whole: Vec<NodeId> = pool.clone();

      whole.extend(labels.iter().map(|(run, _)| *run));
      whole.push(stage);
      Scene::<Terminal>::with(|scene| scene.attach(stage, boxes[0], None));

      let root = Scene::<Terminal>::read(|scene| scene.parent(stage)).expect("the root");

      for change in changes {
        Scene::<Terminal>::with(|scene| {
          let holds = |moved: NodeId, node: NodeId| {
            moving(scene, moved, WALK)
              .into_iter()
              .any(|ancestor| stands_above(scene, ancestor, node))
          };

          match change {
            Change::Attach(parent, child, before) => {
              let (parent, child) = (boxes[parent % boxes.len()], pool[child % 7]);
              let before = before.map(|before| pool[before]);

              if !holds(child, parent)
                && before.is_none_or(|before| !moving(scene, child, WALK).contains(&before))
              {
                scene.attach(parent, child, before);
              }
            }
            Change::Detach(parent, child) => {
              scene.detach(boxes[parent % boxes.len()], pool[child % 7]);
            }
            Change::Insert(segment, node, before) => {
              let start = segments[segment].0;
              let node = pool[node % 7];
              let parent = scene.parent(start);
              let before = before.map(|before| pool[before]);

              if !moving(scene, node, WALK).contains(&start)
                && parent.is_none_or(|parent| !holds(node, parent))
                && before.is_none_or(|before| !moving(scene, node, WALK).contains(&before))
              {
                scene.insert_in_segment(start, node, before);
              }
            }
            Change::Remove(segment, node) => {
              scene.remove_from_segment(segments[segment].0, pool[node % 7]);
            }
            Change::Destroy(node) => scene.destroy(pool[node % 7]),
          }

          assert_whole(scene, &whole);
        });

        let placed = Scene::<Terminal>::read(|scene| placed_labels(scene, root, &labels));
        let drawn: String = testing::render_to_text(4, 6)
          .lines()
          .map(str::trim)
          .collect();

        assert_eq!(drawn, placed, "the frame draws the runs in the placed order");
        assert_eq!(
          read_labels(&testing::access_tree(), root.access_id(), &labels),
          placed,
          "a reader finds them in the same order"
        );
      }

    });
  }
}
