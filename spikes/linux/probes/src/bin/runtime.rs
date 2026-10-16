//! The runtime on glib's main context: tasks, timers and resources, each bound to the owner it
//! was made under.

use std::{
  cell::Cell,
  rc::Rc,
  time::{Duration, Instant},
};

use probes::{result, run, settle, show, wait};
use skene_gui::{Owner, platform::linux::gtk::prelude::*, prelude::*, schedule, sleep, spawn};

fn body() {
  // A task runs on the main loop; one whose owner is disposed first never finishes.
  let finished = Signal::new(false);

  spawn(async move {
    sleep(Duration::from_millis(50)).await;
    finished.set(true);
  });

  let dropped = Rc::new(Cell::new(false));
  let owner = Owner::new();

  {
    let dropped = Rc::clone(&dropped);

    owner.run(|| {
      spawn(async move {
        sleep(Duration::from_millis(100)).await;
        dropped.set(true);
      });
    });
  }

  wait(20);
  owner.dispose();
  wait(300);

  result(
    "spawn: a task finishes on the main loop; one whose owner is disposed never does",
    finished.get() && !dropped.get(),
    format!(
      "finished {}, disposed one finished {}",
      finished.get(),
      dropped.get()
    ),
  );

  // A timer fires after its delay; one whose owner is disposed first never does.
  let started = Instant::now();
  let fired = Rc::new(Cell::new(None::<Duration>));
  let cancelled = Rc::new(Cell::new(false));

  {
    let fired = Rc::clone(&fired);

    schedule(Duration::from_millis(80), move || {
      fired.set(Some(started.elapsed()))
    });
  }

  let owner = Owner::new();

  {
    let cancelled = Rc::clone(&cancelled);

    owner.run(|| schedule(Duration::from_millis(80), move || cancelled.set(true)));
  }

  owner.dispose();
  wait(300);

  result(
    "schedule: fires once after 80 ms; cancelled with its owner",
    fired
      .get()
      .is_some_and(|after| after >= Duration::from_millis(80))
      && !cancelled.get(),
    format!(
      "fired after {:?}, cancelled one fired {}",
      fired.get(),
      cancelled.get()
    ),
  );

  // A resource: a new source drops the load in flight; the value stays until the next load
  // finishes; a view showing it follows.
  let id = Signal::new(1);
  let completions = Rc::new(Cell::new(0));
  let loads = Rc::clone(&completions);
  let user = Resource::new(
    move || id.get(),
    move |id| {
      let loads = Rc::clone(&loads);

      async move {
        sleep(Duration::from_millis(100)).await;
        loads.set(loads.get() + 1);
        format!("user {id}")
      }
    },
  );
  let first = (user.get(), user.loading());
  let shown_text = NodeHandle::new();
  let shown = show(300, -1, || {
    Text::derive(move || user.get().unwrap_or_else(|| "Loading".to_owned())).set_handle(&shown_text)
  });
  let label = || {
    probes::widget(&shown_text)
      .downcast::<skene_gui::platform::linux::gtk::Label>()
      .map(|label| label.label().to_string())
      .unwrap_or_default()
  };
  let loaded = (label(), completions.get());

  wait(30);
  id.set(2);
  wait(30);
  id.set(3);
  wait(250);
  settle();

  result(
    "a resource: a change of source while one loads drops it; the last load shows",
    first == (None, true)
      && loaded == ("user 1".to_owned(), 1)
      && user.get().as_deref() == Some("user 3")
      && !user.loading()
      && completions.get() == 2
      && label() == "user 3",
    format!(
      "made {first:?}; shown {loaded:?}; after sources 2 and 3 {:?}, loading {}, loads finished {}, shown {:?}",
      user.get(),
      user.loading(),
      completions.get(),
      label()
    ),
  );

  id.set(4);
  wait(30);

  let stale = (user.get(), user.loading(), label());

  wait(200);
  settle();

  result(
    "while the next load runs the last value stays",
    stale == (Some("user 3".to_owned()), true, "user 3".to_owned()) && label() == "user 4",
    format!("during {stale:?}, after {:?}", label()),
  );

  shown.close();
}

fn main() -> std::process::ExitCode {
  run("runtime", body)
}
