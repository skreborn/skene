//! Reactive structure in cells: a derived child mounts only its content between its
//! anchors, a derived text rewrites its run in place, and a derived list keeps its kept
//! items and disposes a removed one's owner; a switch keeps each branch it built, pauses
//! a hidden one and annotates every branch its wrapper surrounds; a show builds its
//! child the first time it shows, keeps it in place, and hides and shows the same node;
//! a destroyed node holds no focus and no press; and a view given its child still takes its
//! setters.

use std::{cell::Cell, rc::Rc};

use skene_foundation::{NodeKind, NodeRecord};
use skene_terminal::{
  self as terminal,
  NodeId,
  Scene,
  TOOLTIP_DELAY,
  Terminal,
  prelude::*,
  testing,
};

use crate::common::{colors_at, is_live, red, rgb, rows, with_platform};

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
