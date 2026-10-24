//! Dispatch over the scene in cells, from real input: the climb follows the parents a
//! node had when it began, through the start of each segment the node is mounted in; the
//! pointer enters a segment through its content; a handler runs as the owner that
//! registered it, and work it starts counts toward no boundary; a notification reaches
//! the node's own handlers alone, and a reported edit the node's own controller.

use std::{cell::RefCell, rc::Rc};

use skene_foundation::{
  Boundary,
  ClickEvent,
  ClickHandler,
  Content,
  ContentValue,
  ControlHandle,
  Controller,
  Edit,
  EditTruth,
  EventKind,
  Handler,
  Prevent,
  Prevented,
  Report,
  Verdict,
  count_pending,
  dispatch_click,
  report,
};
use skene_terminal::{
  self as terminal,
  Behavior,
  NativeElement,
  NodeId,
  Role,
  Scene,
  platform::terminal::Terminal,
  prelude::*,
  reactive::{Owner, provide_context, use_context},
  testing,
};

use crate::common::{column, rows, stage, with_platform};

/// The names a log holds, in the order they were noted.
type Log = Rc<RefCell<Vec<&'static str>>>;

/// A box drawing `label`, detached.
fn labelled(label: &'static str) -> NodeId {
  Container::new().set_child(Text::new(label)).build()
}

/// An outer column holding a middle one holding a box drawing `i` in the top left
/// cell, mounted.
fn chain() -> (NodeId, NodeId, NodeId) {
  let (stage, [outer, middle, inner]) = stage(|| [column(), column(), labelled("i")]);

  Scene::<Terminal>::with(|scene| {
    scene.attach(stage, outer, None);
    scene.attach(outer, middle, None);
    scene.attach(middle, inner, None);
  });

  assert_eq!(rows(&testing::render_to_text(4, 1)), "i\n");

  (outer, middle, inner)
}

/// A click handler noting `name` in `log`.
fn record(log: &Log, name: &'static str) -> Rc<ClickHandler> {
  let log = Rc::clone(log);

  Rc::new(move |_: &ClickEvent| {
    log.borrow_mut().push(name);

    Prevent::None
  })
}

/// What `log` noted since it was last taken.
fn take(log: &Log) -> Vec<&'static str> {
  std::mem::take(&mut *log.borrow_mut())
}

/// The climb follows the parents a node had when the dispatch began, whatever a
/// handler changes on the way.
#[test]
fn the_path_is_fixed_before_any_handler_runs() {
  with_platform(|| {
    let (outer, middle, inner) = chain();
    let log = Log::default();

    Scene::<Terminal>::with(|scene| {
      scene.handle(
        inner,
        Handler::Click(Rc::new(move |_: &ClickEvent| {
          Scene::<Terminal>::with(|scene| scene.detach(outer, middle));

          Prevent::None
        })),
      );
      scene.handle(outer, Handler::Click(record(&log, "outer")));
    });

    testing::trigger_mouse_click(0, 0);

    assert_eq!(take(&log), ["outer"]);
    assert_eq!(
      rows(&testing::render_to_text(4, 1)),
      "\n",
      "the handler's detach took effect"
    );
  });
}

/// An event at a node mounted in a segment climbs through the start of each segment
/// it is mounted in, innermost first, before the parent; a node taken out of the
/// segment and attached on its own climbs past it.
#[test]
fn an_event_in_a_segment_climbs_through_its_start() {
  with_platform(|| {
    let (stage, [parent, leaf]) = stage(|| [column(), labelled("l")]);
    let (outer, _) = Scene::<Terminal>::create_segment();
    let (inner, _) = Scene::<Terminal>::create_segment();
    let log = Log::default();

    Scene::<Terminal>::with(|scene| {
      scene.insert_in_segment(outer, inner, None);
      scene.insert_in_segment(inner, leaf, None);
      scene.attach(parent, outer, None);
      scene.attach(stage, parent, None);
      scene.handle(parent, Handler::Click(record(&log, "parent")));
      scene.handle(outer, Handler::Click(record(&log, "outer")));
      scene.handle(inner, Handler::Click(record(&log, "inner")));
      scene.handle(leaf, Handler::Click(record(&log, "leaf")));
    });

    assert_eq!(rows(&testing::render_to_text(4, 1)), "l\n");

    testing::trigger_mouse_click(0, 0);

    assert_eq!(take(&log), ["leaf", "inner", "outer", "parent"]);

    Scene::<Terminal>::with(|scene| {
      scene.remove_from_segment(inner, leaf);
      scene.attach(parent, leaf, None);
    });

    assert_eq!(rows(&testing::render_to_text(4, 1)), "l\n");

    testing::trigger_mouse_click(0, 0);

    assert_eq!(take(&log), ["leaf", "parent"]);
  });
}

/// The pointer reaching a segment's content enters the segment, and moving on to a
/// node outside it leaves the segment.
#[test]
fn the_pointer_enters_a_segment_through_its_content() {
  with_platform(|| {
    let (stage, [leaf, outside]) = stage(|| [labelled("l"), labelled("o")]);
    let (start, _) = Scene::<Terminal>::create_segment();
    let log = Log::default();
    let note = |name: &'static str| -> Rc<dyn Fn()> {
      let log = Rc::clone(&log);

      Rc::new(move || log.borrow_mut().push(name))
    };

    Scene::<Terminal>::with(|scene| {
      scene.attach(stage, start, None);
      scene.insert_in_segment(start, leaf, None);
      scene.attach(stage, outside, None);
      scene.handle(start, Handler::PointerEnter(note("enter")));
      scene.handle(start, Handler::PointerLeave(note("leave")));
    });

    assert_eq!(rows(&testing::render_to_text(4, 2)), "l\no\n");

    testing::trigger_mouse_move(0, 0);

    assert_eq!(take(&log), ["enter"]);

    testing::trigger_mouse_move(0, 1);

    assert_eq!(take(&log), ["leave"]);
  });
}

/// A handler runs as the owner that registered it, so it sees that owner's context
/// and what it creates ends with that owner.
#[test]
fn a_handler_runs_as_the_owner_that_registered_it() {
  with_platform(|| {
    let (_, _, inner) = chain();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner::new();

    owner.enter(|| {
      provide_context(7u32);

      let seen = Rc::clone(&seen);

      Scene::<Terminal>::with(|scene| {
        scene.handle(
          inner,
          Handler::Click(Rc::new(move |_: &ClickEvent| {
            seen
              .borrow_mut()
              .push((use_context::<u32>(), Owner::current() == owner));

            Prevent::None
          })),
        );
      });
    });

    testing::trigger_mouse_click(0, 0);

    assert_eq!(*seen.borrow(), [(Some(7), true)]);
    assert_eq!(
      dispatch_click::<Terminal>(inner, &ClickEvent::new(1)),
      Prevented::default()
    );
    assert_eq!(*seen.borrow(), [(Some(7), true), (Some(7), true)]);
  });
}

/// Work a handler starts is an action, counted toward no boundary.
#[test]
fn a_task_a_handler_starts_counts_toward_no_boundary() {
  with_platform(|| {
    let (_, _, inner) = chain();
    let boundary = Boundary::new();
    let owner = Owner::new();
    let tasks = Rc::new(RefCell::new(Vec::new()));

    owner.enter(|| {
      provide_context(boundary);

      let tasks = Rc::clone(&tasks);

      Scene::<Terminal>::with(|scene| {
        scene.handle(
          inner,
          Handler::Click(Rc::new(move |_: &ClickEvent| {
            tasks.borrow_mut().push(count_pending(Box::new(())));

            Prevent::None
          })),
        );
      });
    });

    testing::trigger_mouse_click(0, 0);

    assert_eq!(tasks.borrow().len(), 1);
    assert!(boundary.is_settled());
  });
}

/// A notification runs the node's own handlers only: a popup's dismiss is not heard by
/// the column around it.
#[test]
fn notifications_do_not_bubble() {
  with_platform(|| {
    let log = Log::default();
    let column = terminal::mount({
      let log = Rc::clone(&log);

      move || {
        Column::new()
          .set_children((Overlay::popup()
            .set_open(true)
            .handle_dismiss(move || log.borrow_mut().push("popup"))
            .set_child(Text::new("popup")),))
          .into_any()
          .build()
      }
    });

    Scene::<Terminal>::with(|scene| {
      let log = Rc::clone(&log);

      scene.handle(
        column,
        Handler::Dismiss(Rc::new(move || log.borrow_mut().push("column"))),
      );
    });

    assert!(testing::render_to_text(8, 2).contains("popup"));

    testing::trigger_key("Escape");

    assert_eq!(take(&log), ["popup"]);
  });
}

/// A drawn entry whose truth is `truth`: a third party's behavior registering an edit
/// controller that notes each text it is asked for and keeps it unless it holds a `!`.
struct Noted {
  truth: Signal<String>,
  asked: Rc<RefCell<Vec<String>>>,
}

impl Behavior<Terminal> for Noted {
  fn native(&self) -> Option<NativeElement> {
    Some(NativeElement::TextField)
  }

  fn role(&self) -> Option<Role> {
    None
  }

  fn listens(&self) -> &'static [EventKind] {
    &[]
  }

  fn attach(&self, control: &ControlHandle<Terminal>) {
    let (truth, asked) = (self.truth, Rc::clone(&self.asked));

    control.register(Controller::new(
      move || EditTruth::new(Some(Rc::from(truth.get())), None),
      move |change| {
        if let Some(text) = &change.text {
          asked.borrow_mut().push(text.to_string());

          if !text.contains('!') {
            truth.set(text.to_string());
          }
        }
      },
    ));
  }
}

/// A reported edit is answered by the entry's own controller alone, whether or not the
/// toolkit shows it already, and never by an ancestor's.
#[test]
fn an_edit_is_answered_by_the_entrys_own_controller_alone() {
  with_platform(|| {
    let value = Signal::new(String::new());
    let field = NodeHandle::new();
    let asked = Rc::new(RefCell::new(Vec::new()));
    let noted = Rc::clone(&asked);
    let column = terminal::mount(move || {
      Column::new()
        .set_children((Control::new(Noted {
          truth: value,
          asked: noted,
        })
        .bind_handle(field),))
        .into_any()
        .build()
    });
    let field = field.node().expect("the field");
    let proposed = |text: &str, applied: bool| {
      let edit = Edit::new(text);

      move |node: NodeId| {
        report::<Terminal>(
          node,
          Report::Change {
            content: Content::Edit,
            proposed: ContentValue::Edit(&edit),
            applied,
          },
        )
      }
    };

    let _ = testing::render_to_text(10, 1);

    testing::trigger_key("Tab");
    testing::trigger_key("x");

    assert_eq!(value.get_untracked(), "x");
    assert_eq!(std::mem::take(&mut *asked.borrow_mut()), ["x"]);

    for (text, applied) in [("y", true), ("z", false)] {
      assert_eq!(proposed(text, applied)(field), Verdict::Accept);
      assert_eq!(value.get_untracked(), text);
      assert_eq!(std::mem::take(&mut *asked.borrow_mut()), [text]);
    }

    assert_eq!(
      proposed("z!", true)(field),
      Verdict::Replace(Edit::new("z")),
      "a change the controller refuses is answered with the truth"
    );
    assert_eq!(
      proposed("w", true)(column),
      Verdict::Accept,
      "the column controls no content"
    );
    assert_eq!(std::mem::take(&mut *asked.borrow_mut()), ["z!"]);
    assert_eq!(value.get_untracked(), "z");
  });
}
