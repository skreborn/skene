//! The app's start: a `GtkApplication` whose `activate` builds the root into a window, under an
//! owner that lives as long as the window.
//!
//! `SKENE_SCREENSHOT=<path>` renders the window into a PNG at `path`
//! `SKENE_SCREENSHOT_DELAY_MS` (300 by default) after its first frame is painted, prints what
//! the run did on standard error (`SKENE_SCREENSHOT <path> <width>x<height> ...`), then quits:
//! what a run's evidence is taken with. `SKENE_TRACE_START=1` prints how long after the app
//! started its first frame was painted (`SKENE_FIRST_FRAME <ms>`), then quits.

use std::{cell::RefCell, path::PathBuf, process::ExitCode, time::Duration};

use gtk::{gio, glib, graphene, prelude::*};
use skene_foundation::{NodeId, Owner, seam::activity};

use crate::{environment, widget};

/// Runs the app: a non-unique `GtkApplication` named `app_id` whose window shows `root`.
pub fn start(app_id: &str, root: Box<dyn FnOnce() -> NodeId>) -> ExitCode {
  let started = std::time::Instant::now();
  let application = gtk::Application::builder()
    .application_id(app_id)
    .flags(gio::ApplicationFlags::NON_UNIQUE)
    .build();
  let root = RefCell::new(Some(root));
  let title = app_id.rsplit('.').next().unwrap_or(app_id).to_owned();

  application.connect_activate(move |application| {
    if let Some(window) = application.active_window() {
      window.present();

      return;
    }

    let window = gtk::ApplicationWindow::new(application);

    window.set_title(Some(&title));
    window.set_default_size(480, 640);
    environment::attach_settings();
    environment::attach_window(window.upcast_ref());
    count_frames(window.upcast_ref());

    let owner = Owner::root();

    if let Some(root) = root.take()
      && let Some(child) = widget(owner.run(root))
    {
      window.set_child(Some(&child));
    }

    window.connect_destroy(move |_| owner.dispose());

    if std::env::var_os("SKENE_TRACE_START").is_some() {
      trace_first_frame(window.upcast_ref(), started, application);
    }

    if let Some(path) = std::env::var_os("SKENE_SCREENSHOT") {
      screenshot_after_first_frame(window.upcast_ref(), PathBuf::from(path), application);
    }

    window.present();
  });

  // The app's own arguments are its own: GApplication is given the program's name alone.
  let program: Vec<String> = std::env::args().take(1).collect();

  ExitCode::from(application.run_with_args(&program).get())
}

/// Prints how long after `started` `window` painted its first frame, then quits
/// `application`.
fn trace_first_frame(
  window: &gtk::Window,
  started: std::time::Instant,
  application: &gtk::Application,
) {
  let application = application.clone();

  window.connect_realize(move |window| {
    let Some(clock) = window.frame_clock() else {
      return;
    };
    let application = RefCell::new(Some(application.clone()));

    clock.connect_after_paint(move |_| {
      if let Some(application) = application.take() {
        eprintln!(
          "SKENE_FIRST_FRAME {:.1} ms after the app started",
          started.elapsed().as_secs_f64() * 1000.
        );
        application.quit();
      }
    });
  });
}

/// Counts each frame `window` paints ([`activity::frame`]).
pub fn count_frames(window: &gtk::Window) {
  window.connect_realize(|window| {
    if let Some(clock) = window.frame_clock() {
      clock.connect_after_paint(|_| activity::frame());
    }
  });
}

/// Draws `widget` as it is laid out now, through its window's own renderer.
///
/// # Panics
///
/// Where the widget is not in a shown window.
#[must_use]
pub fn render(widget: &gtk::Widget) -> gtk::gdk::Texture {
  let (width, height) = (widget.width(), widget.height());
  let paintable = gtk::WidgetPaintable::new(Some(widget));
  let snapshot = gtk::Snapshot::new();

  paintable.snapshot(&snapshot, f64::from(width), f64::from(height));

  let node = snapshot.to_node().expect("the widget draws something");
  let renderer = widget
    .native()
    .and_then(|native| native.renderer())
    .expect("a shown window has a renderer");

  renderer.render_texture(
    &node,
    Some(&graphene::Rect::new(0., 0., width as f32, height as f32)),
  )
}

/// Renders `window` into a PNG at `path` once its first frame is painted and the sheet has
/// settled, then quits `application`.
fn screenshot_after_first_frame(
  window: &gtk::Window,
  path: PathBuf,
  application: &gtk::Application,
) {
  let pending = RefCell::new(Some((path, application.clone())));

  window.connect_realize(move |window| {
    let Some(clock) = window.frame_clock() else {
      return;
    };
    let pending = RefCell::new(
      pending
        .take()
        .map(|(path, application)| (path, application, window.clone())),
    );

    clock.connect_after_paint(move |_| {
      let Some((path, application, window)) = pending.take() else {
        return;
      };

      let delay = std::env::var("SKENE_SCREENSHOT_DELAY_MS")
        .ok()
        .and_then(|delay| delay.parse().ok())
        .unwrap_or(300);

      glib::timeout_add_local_once(Duration::from_millis(delay), move || {
        let texture = render(window.upcast_ref());

        texture
          .save_to_png(&path)
          .expect("the screenshot is written");
        let (sheet, activity, environment) = (
          crate::sheet::stats(),
          activity::now(),
          skene_foundation::Environment::current(),
        );

        eprintln!(
          "SKENE_SCREENSHOT {} {}x{} dark={} high_contrast={} width={} frames={} reports={} applies={} rules={} reloads={} token_reloads={} slot_writes={}",
          path.display(),
          texture.width(),
          texture.height(),
          environment.dark.get_untracked(),
          environment.high_contrast.get_untracked(),
          environment.width.get_untracked(),
          activity.frames,
          activity.reports,
          sheet.applies,
          crate::sheet::rule_count(),
          sheet.reloads,
          sheet.token_reloads,
          sheet.slot_writes,
        );

        application.quit();
      });
    });
  });
}

type Job = Box<dyn FnOnce() + Send>;

/// The process's UI thread: GTK initialized once on it with a steady caret (a focused entry
/// paints no frame of its own), every test body run on it in turn.
fn ui_thread() -> &'static std::sync::mpsc::SyncSender<Job> {
  static UI: std::sync::OnceLock<std::sync::mpsc::SyncSender<Job>> = std::sync::OnceLock::new();

  UI.get_or_init(|| {
    let (sender, receiver) = std::sync::mpsc::sync_channel::<Job>(0);

    std::thread::Builder::new()
      .name("skene-ui".to_owned())
      .spawn(move || {
        gtk::init().expect("GTK initializes on the test display");

        if let Some(settings) = gtk::Settings::default() {
          settings.set_gtk_cursor_blink(false);
        }

        environment::attach_settings();

        for job in receiver {
          job();
        }
      })
      .expect("the UI thread starts");

    sender
  })
}

/// Dispatches everything ready on the main context.
pub fn drain() {
  let context = glib::MainContext::default();

  while context.pending() {
    context.iteration(false);
  }
}

/// Runs a test's `body` on the process's UI thread, GTK initialized once, under an owner of
/// its own and a reset environment; its windows are destroyed and its owner disposed when it
/// returns or panics, and a panic fails the calling test with the body's message.
///
/// # Panics
///
/// Where `body` panicked, and before it runs where the process is not on the display runner's
/// private display (`SKENE_DISPLAY`, which `skene-runner` sets to the `DISPLAY` it made): a
/// test never runs on the user's display.
pub fn run_test(name: &'static str, body: fn()) {
  let display = std::env::var_os("DISPLAY");

  assert!(
    display.is_some()
      && display == std::env::var_os("SKENE_DISPLAY")
      && std::env::var_os("WAYLAND_DISPLAY").is_none(),
    "{name}: a test runs on the display runner's private display, never on the user's; run it through `skene-runner` (the target runner `skene new` writes into .cargo/config.toml, or `SKENE_RUNNER=test`), not with DISPLAY={display:?}"
  );

  let (sender, receiver) = std::sync::mpsc::sync_channel(1);

  ui_thread()
    .send(Box::new(move || {
      skene_foundation::Environment::current().reset();
      environment::reset();
      crate::testing::reset();

      let owner = Owner::root();
      let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner.run(body)));

      for window in gtk::Window::list_toplevels() {
        if let Ok(window) = window.downcast::<gtk::Window>() {
          window.destroy();
        }
      }

      owner.dispose();
      drain();

      let outcome = outcome.map_err(|panic| {
        panic
          .downcast_ref::<String>()
          .cloned()
          .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
          .unwrap_or_else(|| "a panic without a message".to_owned())
      });

      let _ = sender.send(outcome);
    }))
    .expect("the UI thread runs");

  if let Err(message) = receiver.recv().expect("the UI thread answers") {
    panic!("{name}: {message}");
  }
}

/// Shows the node `root` in a new window `width` by `height` pixels (`-1` for its natural
/// height), then runs the main loop until its first frame is painted.
#[must_use]
pub fn present(root: NodeId, width: i32, height: i32) -> gtk::Window {
  let window = gtk::Window::new();
  let painted = std::rc::Rc::new(std::cell::Cell::new(false));

  window.set_default_size(width, height);
  environment::attach_window(&window);
  count_frames(&window);

  if let Some(child) = widget(root) {
    window.set_child(Some(&child));
  }

  {
    let painted = std::rc::Rc::clone(&painted);

    window.connect_realize(move |window| {
      if let Some(clock) = window.frame_clock() {
        let painted = std::rc::Rc::clone(&painted);

        clock.connect_after_paint(move |_| painted.set(true));
      }
    });
  }

  window.present();

  let context = glib::MainContext::default();

  while !painted.get() {
    context.iteration(true);
  }

  drain();

  window
}
