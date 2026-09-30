//! The interactive terminal loop, and the seams that drive it without a terminal.
//!
//! [`run`] takes over the terminal (raw mode, the alternate screen, bracketed paste,
//! the mouse when the launch takes it), builds the app, and runs the event loop:
//! keyboard and mouse events dispatch inside a reactive batch through core's dispatch,
//! a paste the terminal brackets lands in the focused entry, and every change
//! repaints. `Ctrl-Q` quits.
//!
//! The terminal's replies arrive as events in the stream the keys come through. Launch
//! writes its queries and paints at once under its assumptions—a dark background, 16
//! colors, the legacy key encoding, text at one cell—and each reply upgrades one
//! whenever it lands. A clipboard read fires on demand and inserts when its report
//! arrives.

mod keys;
mod writer;

use std::{
  io::{self, Write},
  time::{Duration, Instant},
};

use skene_core::{ClipboardAction, IntoView};

pub(crate) use self::keys::{key_event, press};
use self::{
  keys::{KEYBOARD_FLAGS, dispatch_key, portable_key},
  writer::{
    GRAPHICS_PROBE,
    Placed,
    Rendition,
    SIZED_COLUMN,
    dec_mode,
    draw,
    emit_title,
    pointer_shape,
    sync_pointer_shape,
    write_color_queries,
    write_hand_back,
    write_mouse,
    write_queries,
    write_setup,
  },
};
use crate::{
  Launch,
  Tui,
  View as _,
  detect::{self, ColorDepth},
  device::{Device, WindowSize},
  escape::{self, QueryMode},
  frame,
  interaction::{self, WHEEL_STEP},
  paint,
  parse::{ClipboardStatus, Event, KeyKind, Mouse, MouseKind, Parser},
  platform::end_turn,
  text,
  tty::Tty,
  with_store,
};

/// How long the loop waits for input while nothing moves.
const IDLE_POLL: Duration = Duration::from_millis(100);

/// How long an escape the input ends in waits for the rest of its sequence before it
/// reads as a key, while a lone escape can be one.
const ESCAPE_GRACE: Duration = Duration::from_millis(50);

/// The terminal's input as the loop reads it: the decoder, when the terminal last sent
/// anything, whether a lone escape can be a key, and the window size last seen.
struct Input {
  parser: Parser,
  last_read: Instant,
  unambiguous_escapes: bool,
  size: Option<WindowSize>,
}

impl Input {
  /// Input from a terminal whose window is `size`.
  fn new(size: Option<WindowSize>) -> Self {
    Self {
      parser: Parser::default(),
      last_read: Instant::now(),
      unambiguous_escapes: false,
      size,
    }
  }

  /// Opens a reply window for queries just written, which must end with the primary
  /// device attributes request: until its reply, a string sequence reads as a reply.
  const fn open_reply_window(&mut self) {
    self.parser.open_reply_window();
  }

  /// Sets whether every key arrives as an escape sequence of its own, as under the
  /// kitty keyboard protocol's flags the loop pushes: then an escape the input ends in
  /// always waits for the rest of its sequence, and otherwise reads as a key after
  /// [`ESCAPE_GRACE`] without more input.
  const fn set_unambiguous_escapes(&mut self, unambiguous: bool) {
    self.unambiguous_escapes = unambiguous;
  }

  /// Waits on `terminal` up to `timeout` for input or a note, then takes the notes,
  /// every event the input so far decodes to, the dev launcher's lines, and a resize
  /// where the window's size changed since the last call; an error when the terminal
  /// is gone. An escape the input ends in shortens the wait to what remains of its
  /// grace.
  fn wait(&mut self, terminal: &mut impl Device, timeout: Duration) -> io::Result<Vec<Event>> {
    let held = self.parser.holds_escape() && !self.unambiguous_escapes;
    let timeout = if held {
      timeout.min(ESCAPE_GRACE.saturating_sub(self.last_read.elapsed()))
    }
    else {
      timeout
    };
    let mut bytes = Vec::new();
    let notes = terminal.wait(timeout, &mut bytes)?;

    if !bytes.is_empty() {
      self.last_read = Instant::now();
      self.parser.feed(&bytes);
    }
    else if held && self.last_read.elapsed() >= ESCAPE_GRACE {
      self.parser.resolve_escape();
    }

    let mut events: Vec<Event> = notes.into_iter().map(Event::from).collect();

    while let Some(event) = self.parser.pop() {
      events.push(event);
    }

    events.extend(crate::dev::control_lines().into_iter().map(Event::Control));

    if let Some(size) = terminal.size()
      && self.size != Some(size)
    {
      self.size = Some(size);
      events.push(Event::Resized(size));
    }

    Ok(events)
  }
}

/// Takes the terminal over: raw mode, the setup controls with the mouse when `mouse`,
/// and the queries, whose reply window opens on `input`.
fn take_over(terminal: &mut impl Device, input: &mut Input, mouse: bool) -> io::Result<()> {
  terminal.enter_raw_mode()?;
  write_setup(terminal, mouse)?;
  write_queries(terminal)?;
  terminal.flush()?;
  input.open_reply_window();

  Ok(())
}

/// The terminal's pixel mouse reporting: the cell's size in pixels while the platform
/// reports the window's, and whether the terminal confirmed SGR-Pixels mode is set.
struct PixelMouse {
  cell: Option<(u16, u16)>,
  on: bool,
}

impl PixelMouse {
  /// The cell size `size` implies, when the platform reports the window in pixels.
  fn cell_of(size: &WindowSize) -> Option<(u16, u16)> {
    let width = size.pixel_width.filter(|pixels| *pixels > 0)? / size.columns.max(1);
    let height = size.pixel_height.filter(|pixels| *pixels > 0)? / size.rows.max(1);

    (width > 0 && height > 0).then_some((width, height))
  }

  /// Takes the cell size from `size`, asking the terminal for pixel reports once it
  /// is known and for cell reports once it is not; each request is followed by a
  /// query, whose reply says which the reports are from then on.
  fn resize(&mut self, out: &mut impl Write, size: &WindowSize) {
    let cell = Self::cell_of(size);
    let known = cell.is_some();

    if known != self.cell.is_some() {
      let _ = write!(
        out,
        "{}{}",
        dec_mode(escape::SGR_PIXELS_MOUSE, known),
        QueryMode(escape::SGR_PIXELS_MOUSE),
      );
      let _ = out.flush();
    }

    self.cell = cell;
  }

  /// A mouse report as the cell it lands on and the pointer's position within it:
  /// pixels divided by the cell size while pixel reports are on, else the cell at its
  /// center.
  fn locate(&self, mouse: Mouse) -> (Mouse, (f32, f32)) {
    let Some((width, height)) = self.cell.filter(|_| self.on)
    else {
      return (mouse, crate::store::CELL_CENTER);
    };

    (
      Mouse {
        column: mouse.column / width,
        row: mouse.row / height,
        ..mouse
      },
      (within(mouse.column, width), within(mouse.row, height)),
    )
  }
}

/// Where a pixel sits along its cell of `size` pixels, `0..1`: the pixel's center.
fn within(pixel: u16, size: u16) -> f32 {
  (f32::from(pixel % size) + 0.5) / f32::from(size)
}

/// Mounts and runs a skene application in the process's terminal under `launch`; see
/// [`Launch::run`].
pub(crate) fn run<V, F>(launch: Launch, app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Tui>,
{
  let terminal = Tty::open().expect("open the terminal");

  terminal.set_panic_hook(|out| {
    let _ = write_hand_back(out);
  });
  run_on(terminal, launch, app).close();
}

/// Mounts and runs a skene application on `terminal` under `launch` until it quits or
/// the terminal is gone, handing the terminal back as it found it; see
/// [`Launch::run_on`].
#[expect(
  clippy::too_many_lines,
  reason = "the loop body in one place; length is the event count, not complexity"
)]
pub(crate) fn run_on<D, V, F>(mut terminal: D, launch: Launch, app: F) -> D
where
  D: Device,
  F: FnOnce() -> V + 'static,
  V: IntoView<Tui>,
{
  launch.install();

  let mouse = with_store(|store| {
    store.clock = crate::store::Clock::Real(Instant::now());

    store.mouse
  });

  let _ = crate::dev::install();

  let _ = crate::mount(|| app().into_view().build());
  let waker = terminal.waker();

  skene_core::executor::set_notifier(move || waker());

  {
    crate::dev::adopt_restoration();

    if let Some(notice) = crate::dev::notice_signal() {
      let _ = crate::mount(|| crate::dev::notice_view(notice).into_view().build());
    }
  }

  let mut input = Input::new(terminal.size());

  take_over(&mut terminal, &mut input, mouse).expect("take the terminal over");

  let mut pixels = PixelMouse {
    cell: None,
    on: false,
  };
  // The window's size in force: the platform's at launch, then every resize
  // notification's, the terminal's in-band one (mode 2048) or the platform's own.
  let mut window = terminal.size();
  let mut in_band = false;

  if let Some(size) = &window {
    set_cell_pixels(size);

    if mouse {
      pixels.resize(&mut terminal, size);
    }
  }

  let mut depth = ColorDepth::Ansi16;
  let mut palette = detect::Palette::DEFAULT;
  let mut styled_underlines = false;
  let mut synchronized = false;
  // The first cursor position report answers the sizing probe; a later one is a
  // legacy `F3` the parser mistook.
  let mut sizing_probed = false;
  let mut last_frame: Option<paint::Buffer> = None;
  let mut placed = Placed::default();
  // Whether the clipboard answers (mode 5522), and the text a read has returned so
  // far.
  let mut clipboard_answers = false;
  let mut clipboard_read = Vec::new();
  // The first flags report answers the launch query; a later one reports the flags the
  // terminal kept of those pushed.
  let mut keyboard_flags_pushed = false;
  let mut shape = pointer_shape(None);
  let mut mouse_taken = mouse;

  'run: loop {
    run_due_timers();

    // The mouse key moves the mouse between the app and the terminal; the modes and
    // the pixel reports follow, asked for afresh each time the app takes it back.
    let taken = with_store(|store| store.mouse_taken());

    if taken != mouse_taken {
      mouse_taken = taken;

      let _ = write_mouse(&mut terminal, taken);

      if taken {
        if let Some(size) = &window {
          pixels.resize(&mut terminal, size);
        }
      }
      else {
        let _ = write!(
          &mut terminal,
          "{}",
          dec_mode(escape::SGR_PIXELS_MOUSE, false)
        );

        pixels.cell = None;
        pixels.on = false;
      }

      let _ = terminal.flush();
    }

    let _ = skene_core::executor::pump();

    end_turn();

    let title = with_store(|store| {
      store
        .title_dirty
        .then(|| store.title.clone().unwrap_or_default())
    });

    if let Some(title) = title {
      with_store(|store| store.title_dirty = false);
      emit_title(&mut terminal, &title);
    }

    let dirty = with_store(|store| store.needs_frame());

    if dirty {
      // A degenerate reported size (a fresh pseudo-terminal) falls back to a classic
      // 80x24 rather than painting nothing.
      let (width, height) = match &window {
        Some(size) if size.columns > 0 && size.rows > 0 => (size.columns, size.rows),
        _ => crate::store::DEFAULT_SIZE,
      };
      let frame = frame::render(width, height);

      draw(
        &mut terminal,
        &frame,
        Rendition {
          depth,
          palette,
          styled_underlines,
        },
        last_frame.as_ref(),
        synchronized,
        &mut placed,
      );
      last_frame = Some(frame);
    }

    // The shape follows the frame as much as the pointer: what sits under a still
    // pointer changes as the content does.
    if mouse {
      sync_pointer_shape(&mut terminal, &mut shape);
    }

    let (quit, copy, paste_asked, open) = with_store(|store| {
      (
        store.quit,
        store.pending_copy.take(),
        std::mem::take(&mut store.pending_paste),
        store.pending_open.take(),
      )
    });

    if let Some(text) = copy {
      write_copy(&mut terminal, &mut input, &text, clipboard_answers);
    }

    if paste_asked {
      let request = if clipboard_answers {
        escape::READ_CLIPBOARD
      }
      else {
        escape::QUERY_CLIPBOARD
      };

      let _ = write!(terminal, "{request}{}", escape::REQUEST_PRIMARY_ATTRIBUTES);
      let _ = terminal.flush();

      input.open_reply_window();
    }

    // Over a remote session the system's link handler is the remote machine's, so a
    // followed link goes to the clipboard, where copying is offered.
    if let Some(url) = open {
      if !is_remote(|name| std::env::var_os(name).is_some()) {
        open_url(&url);
      }
      else if with_store(|store| store.offers(ClipboardAction::Copy)) {
        write_copy(&mut terminal, &mut input, &url, clipboard_answers);
      }
    }

    if quit {
      break;
    }

    let wait = with_store(|store| {
      let frame = if store.frame_clock_running() {
        crate::store::FRAME_INTERVAL
      }
      else {
        IDLE_POLL
      };

      store.next_timer_due().map_or(frame, |due| due.min(frame))
    });

    // Every event already queued is handled before the next frame, and a run of
    // pointer motion collapses to where it ends, as a browser coalesces `pointermove`.
    // The terminal is gone when its input ends.
    let Ok(events) = input.wait(&mut terminal, wait)
    else {
      break;
    };
    let mut events = events.into_iter().peekable();

    while let Some(event) = events.next() {
      if let Event::Mouse(motion) = &event
        && matches!(motion.kind, MouseKind::Move | MouseKind::Drag(_))
        && matches!(events.peek(), Some(Event::Mouse(next)) if next.kind == motion.kind && next.modifiers == motion.modifiers)
      {
        continue;
      }

      match event {
        Event::Key(key) if key.kind == KeyKind::Release => {
          interaction::key_up(&portable_key(&key));
        }
        Event::Key(key) => {
          dispatch_key(&key);
        }
        Event::Mouse(mouse) => {
          let (mouse, within) = pixels.locate(mouse);

          with_store(|store| store.pointer_within = within);
          dispatch_mouse(mouse);
        }
        Event::Paste(text) => {
          interaction::paste(&text);
        }
        Event::Control(line) => {
          if let Some(path) = crate::dev::handle(&line) {
            finish(&mut terminal);

            let _ = terminal.restore_mode();

            crate::dev::swap(&path);
          }
        }
        Event::Resized(size) => {
          // A full rewrite: the previous frame no longer matches what the terminal
          // shows.
          last_frame = None;

          with_store(|store| store.paint_dirty = true);

          // A notification without a pixel size keeps the known one while the
          // terminal reports resizes in band, since that report carries the true one.
          let known = window.filter(|_| in_band);
          let size = WindowSize {
            pixel_width: size
              .pixel_width
              .or(known.and_then(|known| known.pixel_width)),
            pixel_height: size
              .pixel_height
              .or(known.and_then(|known| known.pixel_height)),
            ..size
          };

          window = Some(size);
          set_cell_pixels(&size);

          if mouse {
            pixels.resize(&mut terminal, &size);
          }
        }
        Event::EndRequested(_) => {
          break 'run;
        }
        // A stop hands the terminal back until the process continues; any continue
        // takes it over afresh, since whatever ran meanwhile may have changed it.
        job @ (Event::StopRequested | Event::Continued) => {
          if job == Event::StopRequested {
            finish(&mut terminal);

            let _ = terminal.restore_mode();

            input.set_unambiguous_escapes(false);
            terminal.stop();
          }

          let _ = take_over(&mut terminal, &mut input, mouse_taken);

          pixels = PixelMouse {
            cell: None,
            on: false,
          };

          if mouse_taken && let Some(size) = &window {
            pixels.resize(&mut terminal, size);
          }

          shape = pointer_shape(None);
          placed = Placed::default();
          last_frame = None;

          with_store(|store| {
            store.paint_dirty = true;
            store.title_dirty = store.title.is_some();
          });
        }
        // A theme change moves the terminal's own colors too, which are asked for
        // afresh.
        Event::Theme { dark } => {
          set_detected_scheme(dark);

          let _ = write_color_queries(&mut terminal);
          let _ = terminal.flush();

          input.open_reply_window();
        }
        // The probe's space advanced the cursor two cells where the terminal sizes
        // text; every size is then honored, and the frame laid out afresh.
        Event::CursorPosition { column, .. } if !sizing_probed => {
          sizing_probed = true;

          if apply_sizing(column == SIZED_COLUMN) {
            last_frame = None;
          }
        }
        Event::Mode {
          mode: escape::SGR_PIXELS_MOUSE,
          setting,
        } => {
          pixels.on = setting.enabled();
        }
        Event::Mode {
          mode: escape::IN_BAND_RESIZE,
          setting,
        } => {
          in_band = setting.enabled();
        }
        // A terminal that can have synchronized output on has every frame written under
        // it; one that cannot, or does not know it, gets plain frames.
        Event::Mode {
          mode: escape::SYNCHRONIZED_OUTPUT,
          setting,
        } => {
          synchronized = setting.available();
        }
        // A terminal with grapheme clustering on, set or permanently, measures text by
        // clusters, as the layout then does; one without it, or not knowing it, gets
        // text measured by code points, and a change of measurement lays the frame out
        // afresh.
        Event::Mode {
          mode: escape::GRAPHEME_CLUSTERING,
          setting,
        } => {
          let changed = apply_clustering(setting.enabled());

          if changed {
            last_frame = None;
          }
        }
        // The probe answered `OK` proves the graphics protocol; every painted box then
        // renders and places its picture.
        Event::Graphics {
          id: GRAPHICS_PROBE,
          ok: true,
        } => {
          let proven = apply_graphics(true);

          if proven {
            last_frame = None;
          }
        }
        Event::Background(red, green, blue) => {
          set_detected_background((red, green, blue));
        }
        Event::Foreground(red, green, blue) => {
          with_store(|store| {
            store.detected_foreground = Some((red, green, blue));
            store.paint_dirty = true;
          });
        }
        // A base color the terminal shows otherwise than the defaults moves what a
        // 16-color paint picks, so the frame is written afresh at that depth.
        Event::Palette(colors) => {
          let mut moved = false;

          for (index, color) in colors {
            moved |= palette.set(index, color);
          }

          if moved && depth == ColorDepth::Ansi16 {
            last_frame = None;

            with_store(|store| store.paint_dirty = true);
          }
        }
        Event::Clipboard(content) => {
          interaction::paste_reply(&content);
        }
        // A read's answer arrives in parts: the text once it is done, nothing when it
        // was refused; a write's answers fall through.
        Event::ClipboardAnswer { read: true, status } => match status {
          ClipboardStatus::Ok => clipboard_read.clear(),
          ClipboardStatus::Data(bytes) => clipboard_read.extend(bytes),
          ClipboardStatus::Done => {
            let text = String::from_utf8_lossy(&std::mem::take(&mut clipboard_read)).into_owned();

            interaction::paste_reply(&text);
          }
          ClipboardStatus::Refused => {
            clipboard_read.clear();

            with_store(|store| store.paste_target = None);
          }
        },
        // A terminal whose clipboard answers offers every operation from its reply.
        Event::Mode {
          mode: escape::ANSWERING_CLIPBOARD,
          setting,
        } => {
          clipboard_answers = setting.available();

          with_store(|store| store.clipboard_answers = clipboard_answers);
        }
        // A terminal speaking the kitty keyboard protocol disambiguates modified
        // keys—`Ctrl+Backspace` above all; the rest keep the legacy encoding, where
        // `Ctrl+W` stands in for it. Its report of the flags it kept says whether it
        // reports releases.
        Event::KeyboardFlags(flags) => {
          if keyboard_flags_pushed {
            with_store(|store| store.key_releases = flags & escape::REPORT_EVENT_TYPES != 0);
          }
          else {
            keyboard_flags_pushed = true;

            let _ = write!(
              terminal,
              "{}{}",
              escape::KeyboardFlags(KEYBOARD_FLAGS),
              escape::QUERY_KEYBOARD_FLAGS,
            );
            let _ = terminal.flush();

            input.set_unambiguous_escapes(true);
          }
        }
        Event::GraphicRendition(attributes) => {
          if let Some(reported) = ColorDepth::reported(&attributes)
            && reported > depth
          {
            depth = reported;
            last_frame = None;

            with_store(|store| store.paint_dirty = true);
          }

          if detect::styled_underlines_reported(&attributes) && !styled_underlines {
            styled_underlines = true;
            last_frame = None;

            with_store(|store| store.paint_dirty = true);
          }
        }
        _ => {}
      }

      end_turn();
    }
  }

  finish(&mut terminal);

  terminal
}

/// Runs every scheduled action whose reading has come, earliest due first, as one
/// turn.
pub(crate) fn run_due_timers() {
  let batch = with_store(|store| store.timers.next_timer());
  let mut ran = false;

  while let Some(action) = with_store(|store| {
    let now = store.now();

    store.timers.pop_due(now, batch)
  }) {
    action();
    ran = true;
  }

  if ran {
    end_turn();
  }
}

/// Frees every picture and hands the terminal back.
fn finish(terminal: &mut impl Device) {
  let _ = write_hand_back(terminal);
  let _ = terminal.flush();
}

/// Records the terminal's own background, which a paint mixes against, and the scheme
/// its luminance implies; the frame paints afresh.
pub(crate) fn set_detected_background(background: (u8, u8, u8)) {
  with_store(|store| {
    store.detected_background = Some(background);
    store.paint_dirty = true;
  });
  set_detected_scheme(detect::is_dark(background));
}

/// Records the detected background, moving the scheme in force where it changes.
pub(crate) fn set_detected_scheme(dark: bool) {
  let write = with_store(|store| {
    let before = store.scheme();

    store.detected_dark = dark;

    let after = store.scheme();

    if before == after {
      return None;
    }

    store.invalidate_styles();

    Some((store.media().set_scheme, after))
  });

  if let Some((write, scheme)) = write {
    skene_reactive::Set::set(&write, scheme);
  }
}

/// Dispatches one mouse event: hover tracking (with the pointer shape), the pointer
/// handlers through core's dispatch, and left-button clicks.
fn dispatch_mouse(mouse: Mouse) {
  let modifiers = mouse.modifiers;

  match mouse.kind {
    MouseKind::Move | MouseKind::Drag(_) => {
      let button = match mouse.kind {
        MouseKind::Drag(button) => Some(button),
        _ => None,
      };

      interaction::pointer_move(mouse.column, mouse.row, button, modifiers);
    }
    MouseKind::Press(button) => {
      with_store(|store| store.buttons.insert(button));
      interaction::pointer_press(mouse.column, mouse.row, button, modifiers);
    }
    MouseKind::Release(button) => {
      // A release whose press was never delivered stands in for the whole click: VTE
      // terminals report only the first three presses of a rapid sequence to the app,
      // yet forward every release.
      if !with_store(|store| store.buttons.contains(button)) {
        with_store(|store| store.buttons.insert(button));
        interaction::pointer_press(mouse.column, mouse.row, button, modifiers);
      }

      with_store(|store| store.buttons.remove(button));
      interaction::pointer_release(mouse.column, mouse.row, button, modifiers);
    }
    MouseKind::ScrollDown => {
      interaction::wheel(mouse.column, mouse.row, 0, WHEEL_STEP, modifiers);
    }
    MouseKind::ScrollUp => {
      interaction::wheel(mouse.column, mouse.row, 0, -WHEEL_STEP, modifiers);
    }
    MouseKind::ScrollRight => {
      interaction::wheel(mouse.column, mouse.row, WHEEL_STEP, 0, modifiers);
    }
    MouseKind::ScrollLeft => {
      interaction::wheel(mouse.column, mouse.row, -WHEEL_STEP, 0, modifiers);
    }
  }
}

/// Opens `url` with the system handler, detached and best-effort, the handler reaped
/// once it exits.
fn open_url(url: &str) {
  #[cfg(target_os = "macos")]
  let command = "open";
  #[cfg(all(unix, not(target_os = "macos")))]
  let command = "xdg-open";
  #[cfg(windows)]
  let command = "explorer";

  let child = std::process::Command::new(command)
    .arg(url)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
    .spawn();

  if let Ok(mut child) = child {
    std::thread::spawn(move || child.wait());
  }
}

/// Whether the process runs in a remote session, the environment `variable` names being
/// set: over ssh.
fn is_remote(variable: impl Fn(&str) -> bool) -> bool {
  ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
    .into_iter()
    .any(variable)
}

/// Writes `text` to the clipboard: through the terminal's answering clipboard where it
/// `answers`, in a reply window of its own, and else through OSC 52, which a terminal
/// without it ignores and one gating clipboard writes asks the user about.
fn write_copy(terminal: &mut impl Device, input: &mut Input, text: &str, answers: bool) {
  if answers {
    let _ = escape::write_clipboard(terminal, text);
    let _ = write!(terminal, "{}", escape::REQUEST_PRIMARY_ATTRIBUTES);

    input.open_reply_window();
  }
  else {
    let _ = write!(terminal, "{}", escape::Copy(text));
  }

  let _ = terminal.flush();
}

/// Puts the text measurement on the terminal's grapheme clustering or off it, every
/// entry measured again and the frame laid out afresh; `true` when that changed it.
pub(crate) fn apply_clustering(confirmed: bool) -> bool {
  if !text::set_clustering(confirmed) {
    return false;
  }

  with_store(|store| {
    store.remeasure();
    store.layout_dirty = true;
    store.paint_dirty = true;
  });

  true
}

/// Puts the text measurement on the terminal's text sizing (OSC 66) or off it, every
/// run measured at its size or at one cell and the frame laid out afresh; `true`
/// when that changed it.
pub(crate) fn apply_sizing(confirmed: bool) -> bool {
  if !text::set_sizing(confirmed) {
    return false;
  }

  with_store(|store| {
    store.remeasure();
    store.layout_dirty = true;
    store.paint_dirty = true;
  });

  true
}

/// Puts the graphics on or off: the capability signal moves, so an enhancement on it
/// builds its painted form or its fallback, and the next frame paints whole; `true`
/// when that changed it.
pub(crate) fn apply_graphics(confirmed: bool) -> bool {
  let write = with_store(|store| {
    if store.graphics == confirmed {
      return None;
    }

    store.graphics = confirmed;
    store.paint_dirty = true;

    Some(store.media().set_graphics)
  });
  let Some(write) = write
  else {
    return false;
  };

  skene_reactive::Set::set(&write, confirmed);

  true
}

/// Takes the cell's pixel size from the window's, where the terminal reports it, so a
/// picture renders at the size the terminal shows it.
fn set_cell_pixels(size: &WindowSize) {
  let cell = PixelMouse::cell_of(size).unwrap_or(crate::store::DEFAULT_CELL_PIXELS);

  with_store(|store| {
    if store.cell_pixels != cell {
      store.cell_pixels = cell;
      store.paint_dirty = true;
    }
  });
}

#[cfg(test)]
mod tests {
  use skene_core::Modifiers;

  use super::*;

  /// A session is remote where any of ssh's variables is set.
  #[test]
  fn ssh_makes_a_session_remote() {
    assert!(!is_remote(|_| false));
    assert!(is_remote(|name| name == "SSH_TTY"));
    assert!(is_remote(|name| name == "SSH_CONNECTION"));
  }

  /// A pixel report lands on the cell holding the pixel, the pointer at the pixel's
  /// center within it; without a cell size or the mode confirmed, a report is its
  /// cell at the center.
  #[test]
  fn pixel_reports_locate_within_their_cell() {
    let report = Mouse {
      kind: MouseKind::Move,
      column: 45,
      row: 33,
      modifiers: Modifiers::default(),
    };
    let pixels = PixelMouse {
      cell: Some((10, 16)),
      on: true,
    };
    let (mouse, within) = pixels.locate(report);

    assert_eq!((mouse.column, mouse.row), (4, 2));
    assert!((within.0 - 0.55).abs() < 1e-6 && (within.1 - 0.09375).abs() < 1e-6);

    let cells = PixelMouse {
      cell: Some((10, 16)),
      on: false,
    };

    assert_eq!(cells.locate(report), (report, crate::store::CELL_CENTER));
  }

  /// The cell size is the window's pixels over its cells, and absent without them.
  #[test]
  fn the_cell_size_follows_the_window() {
    let size = WindowSize {
      columns: 80,
      rows: 24,
      pixel_width: Some(800),
      pixel_height: Some(432),
    };

    assert_eq!(PixelMouse::cell_of(&size), Some((10, 18)));
    assert_eq!(
      PixelMouse::cell_of(&WindowSize {
        pixel_width: None,
        ..size
      }),
      None
    );
    assert_eq!(
      PixelMouse::cell_of(&WindowSize {
        pixel_height: Some(0),
        ..size
      }),
      None
    );
  }
}
