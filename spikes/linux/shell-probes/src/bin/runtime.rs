//! The runtime on the realization's loop: a task that sleeps and ticks five times, a timer
//! that fires once, and a resource that loads after a sleep and again when its source
//! changes, each logged on standard output with the time since start.

use std::{
  cell::Cell,
  time::{Duration, Instant},
};

use skene_gui::{Resource, prelude::*, schedule, sleep, spawn};

thread_local! {
  static STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
}

fn log(what: &str) {
  let started = STARTED.with(|started| {
    let at = started.get().unwrap_or_else(Instant::now);

    started.set(Some(at));

    at
  });

  println!("{what} at {} ms", started.elapsed().as_millis());
}

#[skene_gui::main]
fn app() -> impl View {
  let ticks = Signal::new(0u32);
  let fired = Signal::new(false);
  let source = Signal::new(1u32);

  log("start");
  spawn(async move {
    for _ in 0..5 {
      sleep(Duration::from_millis(100)).await;
      ticks.update(|ticks| *ticks += 1);
      log(&format!("tick {}", ticks.get_untracked()));
    }

    source.set(2);
  });
  schedule(Duration::from_millis(250), move || {
    fired.set(true);
    log("scheduled");
  });

  let loaded = Resource::new(
    move || source.get(),
    |source| async move {
      sleep(Duration::from_millis(50)).await;
      log(&format!("loaded {source}"));

      source * 10
    },
  );

  Column::new()
    .set_padding(16)
    .set_spacing(8)
    .add_child(Text::derive(move || format!("Ticks: {}", ticks.get())))
    .add_child(Text::derive(move || format!("Scheduled: {}", fired.get())))
    .add_child(Text::derive(move || match loaded.get() {
      Some(value) => format!("Loaded: {value}"),
      None => "Loading".to_owned(),
    }))
}
