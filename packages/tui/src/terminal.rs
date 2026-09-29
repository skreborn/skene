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

use std::{
  collections::HashMap,
  io::{self, Write},
  str::FromStr,
  time::Duration,
};

use skene_core::{
  ClipboardAction,
  Code,
  ColorScheme,
  IntoView,
  Key as PortableKey,
  KeyEvent,
  Modifiers,
  MouseCursor,
  NamedKey,
  PointerButton,
  TextDecorationStyle,
};

use crate::{
  Launch,
  Tui,
  View as _,
  detect::{self, ColorDepth},
  escape::{self, Color, CursorStyle, DecMode, MoveTo, QueryMode, Sgr, Underline},
  frame,
  interaction::{self, WHEEL_STEP},
  paint,
  parse::{
    ClipboardStatus,
    Event,
    Key,
    KeyCode,
    KeyKind,
    ModifierKey,
    Mouse,
    MouseKind,
    WindowSize,
  },
  platform::end_turn,
  text::{self, CellScale, Glyph},
  tty::Tty,
  with_store,
};

/// How long the loop waits for input while nothing moves.
const IDLE_POLL: Duration = Duration::from_millis(100);

/// A DEC private mode set or reset, as one control.
const fn dec_mode(code: u16, on: bool) -> DecMode {
  DecMode { code, on }
}

/// Writes the controls that take the terminal over: the title saved, the alternate
/// screen, mouse reporting when `mouse`, bracketed paste, scheme notifications,
/// grapheme clustering, in-band resize notifications, autowrap off, and the cursor
/// hidden.
fn write_setup(out: &mut (impl Write + ?Sized), mouse: bool) -> io::Result<()> {
  write!(
    out,
    "{}{}",
    escape::PUSH_TITLE,
    dec_mode(escape::ALTERNATE_SCREEN, true)
  )?;

  if mouse {
    write_mouse(out, true)?;
  }

  write!(
    out,
    "{}{}{}{}{}{}",
    dec_mode(escape::BRACKETED_PASTE, true),
    dec_mode(escape::THEME, true),
    dec_mode(escape::GRAPHEME_CLUSTERING, true),
    dec_mode(escape::IN_BAND_RESIZE, true),
    dec_mode(escape::AUTOWRAP, false),
    dec_mode(escape::SHOW_CURSOR, false),
  )
}

/// Writes the modes taking the mouse from the terminal when `on`, and handing it back
/// when not.
fn write_mouse(out: &mut (impl Write + ?Sized), on: bool) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}",
    dec_mode(escape::MOUSE_TRACKING, on),
    dec_mode(escape::BUTTON_EVENT_MOUSE, on),
    dec_mode(escape::ANY_EVENT_MOUSE, on),
    dec_mode(escape::SGR_MOUSE, on),
  )
}

/// Writes the queries whose replies upgrade the launch assumptions: the scheme (a 2031
/// report, the OSC 11 background), the terminal's own colors (the OSC 10 foreground
/// and the OSC 4 base colors), the kitty keyboard flags, synchronized output,
/// grapheme clustering, in-band resize and the answering clipboard (DECRPM reports
/// for modes 2026, 2027, 2048 and 5522), styled underlines (a DECRQSS report after a curly
/// underline), the color depth (a DECRQSS report after a true-color and after a palette
/// background), and text sizing (a cursor position report after a space at scale two from the home
/// cell, which lands on the third column where the terminal sized it), and graphics
/// (the kitty protocol's query, answered `OK` where it is spoken); then the primary
/// device attributes request, whose reply closes the queries' reply window.
fn write_queries(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
    escape::QUERY_THEME,
    QueryMode(escape::SYNCHRONIZED_OUTPUT),
    QueryMode(escape::GRAPHEME_CLUSTERING),
    QueryMode(escape::IN_BAND_RESIZE),
    QueryMode(escape::ANSWERING_CLIPBOARD),
    escape::QUERY_BACKGROUND,
    escape::QUERY_FOREGROUND,
    escape::QUERY_PALETTE,
    escape::QUERY_KEYBOARD_FLAGS,
    Sgr::Background(detect::TRUE_COLOR_PROBE),
    escape::REQUEST_GRAPHIC_RENDITION,
    Sgr::Background(Color::Palette(detect::PALETTE_PROBE)),
    escape::REQUEST_GRAPHIC_RENDITION,
    Sgr::Underline(Underline::Curly),
    escape::REQUEST_GRAPHIC_RENDITION,
    Sgr::Reset,
    move_to(0, 0),
    SIZING_PROBE,
    escape::REQUEST_CURSOR_POSITION,
    escape::GraphicsQuery(GRAPHICS_PROBE),
    escape::REQUEST_PRIMARY_ATTRIBUTES,
  )
}

/// Writes the queries for the terminal's own colors—the OSC 11 background, the OSC 10
/// foreground and the OSC 4 base colors—then the primary device attributes request,
/// whose reply closes their reply window.
fn write_color_queries(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}",
    escape::QUERY_BACKGROUND,
    escape::QUERY_FOREGROUND,
    escape::QUERY_PALETTE,
    escape::REQUEST_PRIMARY_ATTRIBUTES,
  )
}

/// The image id the graphics probe asks under; a painted box's ids are its record's.
const GRAPHICS_PROBE: u32 = 0x7fff_ffff;

/// A space at scale two: the text sizing probe, two cells wide where the terminal
/// sizes text and nothing elsewhere.
const SIZING_PROBE: &str = "\x1b]66;s=2; \x1b\\";

/// The column the probe's cursor position report names where the terminal sized it.
const SIZED_COLUMN: u16 = 2;

/// Writes the controls that hand the terminal back: the graphic rendition reset, the
/// legacy key encoding, mouse reporting, bracketed paste, scheme and resize
/// notifications and grapheme clustering off, autowrap on, the cursor shown in its
/// default shape, the main screen, and the title restored. Idempotent but for the
/// title, which pops what setup saved.
fn write_restore(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(
    out,
    "{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}",
    Sgr::Reset,
    escape::PointerShape("default"),
    escape::KeyboardFlags(0),
    dec_mode(escape::THEME, false),
    dec_mode(escape::IN_BAND_RESIZE, false),
    dec_mode(escape::GRAPHEME_CLUSTERING, false),
    dec_mode(escape::BRACKETED_PASTE, false),
    dec_mode(escape::SGR_PIXELS_MOUSE, false),
    dec_mode(escape::SGR_MOUSE, false),
    dec_mode(escape::ANY_EVENT_MOUSE, false),
    dec_mode(escape::BUTTON_EVENT_MOUSE, false),
    dec_mode(escape::MOUSE_TRACKING, false),
    dec_mode(escape::AUTOWRAP, true),
    CursorStyle::Default,
    dec_mode(escape::SHOW_CURSOR, true),
    dec_mode(escape::ALTERNATE_SCREEN, false),
    escape::POP_TITLE,
  )
}

/// Writes the controls that free every picture and hand the terminal back.
fn write_hand_back(out: &mut (impl Write + ?Sized)) -> io::Result<()> {
  write!(out, "{}", escape::FREE_ALL)?;
  write_restore(out)
}

/// Takes the terminal over: raw mode, the setup controls with the mouse when `mouse`,
/// and the queries, whose reply window opens.
fn take_over(terminal: &mut Tty, mouse: bool) -> io::Result<()> {
  terminal.enter_raw_mode()?;
  write_setup(terminal, mouse)?;
  write_queries(terminal)?;
  terminal.flush()?;
  terminal.open_reply_window();

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

/// Mounts and runs a skene application in the terminal under `launch`; see
/// [`Launch::run`].
#[expect(
  clippy::too_many_lines,
  reason = "the loop body in one place; length is the event count, not complexity"
)]
pub(crate) fn run<V, F>(launch: Launch, app: F)
where
  F: FnOnce() -> V + 'static,
  V: IntoView<Tui>,
{
  launch.install();

  let mouse = with_store(|store| {
    store.clock = crate::store::Clock::Real(std::time::Instant::now());

    store.mouse
  });

  let control = crate::dev::install();

  let _ = crate::mount(|| app().into_view().build());

  let mut terminal = Tty::open().expect("open the terminal");

  skene_core::executor::set_notifier(crate::tty::wake);

  if let Some(fd) = control {
    terminal.set_control(fd);
  }

  terminal.set_panic_hook(|out| {
    let _ = write_hand_back(out);
  });

  {
    crate::dev::adopt_restoration();

    if let Some(notice) = crate::dev::notice_signal() {
      let _ = crate::mount(|| crate::dev::notice_view(notice).into_view().build());
    }
  }

  take_over(&mut terminal, mouse).expect("take the terminal over");

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
      write_copy(&mut terminal, &text, clipboard_answers);
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

      terminal.open_reply_window();
    }

    // Over a remote session the system's link handler is the remote machine's, so a
    // followed link goes to the clipboard, where copying is offered.
    if let Some(url) = open {
      if !is_remote(|name| std::env::var_os(name).is_some()) {
        open_url(&url);
      }
      else if with_store(|store| store.offers(ClipboardAction::Copy)) {
        write_copy(&mut terminal, &url, clipboard_answers);
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
    let Ok(events) = terminal.wait(wait)
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

            terminal.set_unambiguous_escapes(false);
            terminal.stop();
          }

          let _ = take_over(&mut terminal, mouse_taken);

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

          terminal.open_reply_window();
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

            terminal.set_unambiguous_escapes(true);
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
  terminal.close();
}

/// Runs every scheduled action whose reading has come, earliest due first, as one
/// turn.
fn run_due_timers() {
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
fn finish(terminal: &mut Tty) {
  let _ = write_hand_back(terminal);
  let _ = terminal.flush();
}

/// Records the terminal's own background, which a paint mixes against, and the scheme
/// its luminance implies; the frame paints afresh.
fn set_detected_background(background: (u8, u8, u8)) {
  with_store(|store| {
    store.detected_background = Some(background);
    store.paint_dirty = true;
  });
  set_detected_scheme(detect::is_dark(background));
}

/// Records the detected background, moving the scheme in force where it changes.
fn set_detected_scheme(dark: bool) {
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
    MouseKind::ScrollDown => interaction::wheel(mouse.column, mouse.row, 0, WHEEL_STEP),
    MouseKind::ScrollUp => interaction::wheel(mouse.column, mouse.row, 0, -WHEEL_STEP),
    MouseKind::ScrollRight => interaction::wheel(mouse.column, mouse.row, WHEEL_STEP, 0),
    MouseKind::ScrollLeft => interaction::wheel(mouse.column, mouse.row, -WHEEL_STEP, 0),
  }
}

/// Emits the terminal title via OSC 2; terminals without support ignore it.
fn emit_title(out: &mut impl Write, title: &str) {
  let _ = write!(out, "{}", escape::Title(title));
  let _ = out.flush();
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
fn write_copy(terminal: &mut Tty, text: &str, answers: bool) {
  if answers {
    let _ = escape::write_clipboard(terminal, text);
    let _ = write!(terminal, "{}", escape::REQUEST_PRIMARY_ATTRIBUTES);

    terminal.open_reply_window();
  }
  else {
    let _ = write!(terminal, "{}", escape::Copy(text));
  }

  let _ = terminal.flush();
}

/// Simulates the terminal's report that it reports key releases (the kitty keyboard
/// protocol's event types), or that it does not, for a test driving the loop headlessly.
///
/// Until it does, a release follows every press at once.
pub fn trigger_key_releases(confirmed: bool) {
  trigger(|| with_store(|store| store.key_releases = confirmed));
}

/// Simulates the terminal's report that its clipboard answers (mode 5522), or that it
/// does not, for a test driving the loop headlessly.
///
/// An answering clipboard offers every operation, whatever the launch chose.
pub fn trigger_answering_clipboard(confirmed: bool) {
  trigger(|| with_store(|store| store.clipboard_answers = confirmed));
}

/// The OSC 22 name of a pointer shape; the terminal's own shape where none is set.
const fn pointer_shape(cursor: Option<MouseCursor>) -> &'static str {
  match cursor {
    Some(MouseCursor::Click) => "pointer",
    Some(MouseCursor::Text) => "text",
    Some(MouseCursor::Grab) => "grab",
    Some(MouseCursor::Grabbing) => "grabbing",
    Some(MouseCursor::Default) | None => "default",
  }
}

/// Writes the pointer shape the current hover resolves to when it differs from
/// `shape`, the one last written, which follows.
fn sync_pointer_shape(out: &mut impl Write, shape: &mut &'static str) {
  let current = pointer_shape(with_store(|store| store.hover_cursor()));

  if current != *shape {
    *shape = current;

    let _ = write!(out, "{}", escape::PointerShape(current));
    let _ = out.flush();
  }
}

/// The control moving the cursor to the cell (`column`, `row`).
const fn move_to(column: u16, row: u16) -> MoveTo {
  MoveTo { column, row }
}

/// What the terminal renders of a cell's attributes: the color depth painted RGB
/// quantizes to, the base colors it shows, and whether an underline keeps its style
/// and color.
#[derive(Clone, Copy)]
struct Rendition {
  depth: ColorDepth,
  palette: detect::Palette,
  styled_underlines: bool,
}

impl Rendition {
  /// The underline control for `decoration`: the style and color where the terminal
  /// keeps them, a plain single underline elsewhere.
  const fn underline(self, decoration: paint::CellDecoration) -> Underline {
    match decoration.underline {
      None => Underline::None,
      Some(_) if !self.styled_underlines => Underline::Single,
      Some(TextDecorationStyle::Solid) => Underline::Single,
      Some(TextDecorationStyle::Double) => Underline::Double,
      Some(TextDecorationStyle::Dotted) => Underline::Dotted,
      Some(TextDecorationStyle::Dashed) => Underline::Dashed,
      Some(TextDecorationStyle::Wavy) => Underline::Curly,
    }
  }

  /// Writes the attributes taking a cell's decoration from `current` to `next`.
  fn write_decoration(
    self,
    out: &mut impl Write,
    current: paint::CellDecoration,
    next: paint::CellDecoration,
  ) {
    let underline = self.underline(next);

    if underline != self.underline(current) {
      let _ = write!(out, "{underline}", underline = Sgr::Underline(underline));
    }

    if self.styled_underlines && next.underline_color != current.underline_color {
      let color = next.underline_color.map_or(Color::Reset, |rgb| {
        self.depth.terminal_color(rgb, &self.palette)
      });

      let _ = write!(out, "{}", Sgr::UnderlineColor(color));
    }

    if next.overline != current.overline {
      let _ = write!(out, "{}", Sgr::Overline(next.overline));
    }

    if next.line_through != current.line_through {
      let _ = write!(out, "{}", Sgr::StrikeThrough(next.line_through));
    }
  }
}

/// What the terminal holds of the frame's pictures: the generation each image was
/// transmitted at, and the cells each is placed over.
#[derive(Default)]
struct Placed {
  transmitted: HashMap<u32, u64>,
  placements: HashMap<u32, paint::PlacedCells>,
}

/// Writes a painted frame to the terminal under `rendition`; rows unchanged since the
/// previous frame are skipped, the pictures are transmitted where their image changed
/// and placed where their cells did (every one again without a previous frame), an
/// image no picture shows any more is freed, and the whole frame sits between the
/// synchronized-output brackets (mode 2026) when `synchronized`.
fn draw(
  out: &mut impl Write,
  buffer: &paint::Buffer,
  rendition: Rendition,
  previous: Option<&paint::Buffer>,
  synchronized: bool,
  placed: &mut Placed,
) {
  if synchronized {
    let _ = write!(out, "{}", dec_mode(escape::SYNCHRONIZED_OUTPUT, true));
  }

  // The cursor hides while the cells are written, so it never shows mid-frame.
  let _ = write!(
    out,
    "{}{}{}",
    dec_mode(escape::SHOW_CURSOR, false),
    move_to(0, 0),
    Sgr::Reset
  );

  for row in 0..buffer.height {
    if !paint::row_changed(previous, buffer, row) {
      continue;
    }

    draw_row(out, buffer, row, rendition);
  }

  draw_pictures(out, buffer, placed, previous.is_none());

  // The focused entry's caret is the terminal's own bar cursor, blinking as the
  // terminal does.
  if let Some((column, row)) = buffer.caret {
    let _ = write!(
      out,
      "{}{}{}",
      move_to(column, row),
      CursorStyle::BlinkingBar,
      dec_mode(escape::SHOW_CURSOR, true)
    );
  }

  if synchronized {
    let _ = write!(out, "{}", dec_mode(escape::SYNCHRONIZED_OUTPUT, false));
  }

  let _ = out.flush();
}

/// Writes the frame's pictures: an image transmitted where its generation moved on,
/// placed over its cells where those moved or the frame is written `whole`, and freed
/// where no picture shows it any more.
fn draw_pictures(out: &mut impl Write, buffer: &paint::Buffer, placed: &mut Placed, whole: bool) {
  let mut shown = Vec::new();

  for picture in &buffer.pictures {
    shown.push(picture.id);

    if placed.transmitted.get(&picture.id) != Some(&picture.image.generation) {
      let _ = escape::write_transmit(
        out,
        picture.id,
        picture.image.width,
        picture.image.height,
        &picture.image.pixels,
      );

      placed
        .transmitted
        .insert(picture.id, picture.image.generation);
      placed.placements.remove(&picture.id);
    }

    let cells = picture.cells();

    if !whole && placed.placements.get(&picture.id) == Some(&cells) {
      continue;
    }

    if placed.placements.contains_key(&picture.id) {
      let _ = write!(out, "{}", escape::Unplace(picture.id));
    }

    let _ = write!(
      out,
      "{}{}",
      move_to(picture.column, picture.row),
      escape::Place {
        id: picture.id,
        columns: picture.columns,
        rows: picture.rows,
        crop: picture.crop,
        layer: picture.layer,
      }
    );
    placed.placements.insert(picture.id, cells);
  }

  let gone: Vec<u32> = placed
    .transmitted
    .keys()
    .filter(|id| !shown.contains(id))
    .copied()
    .collect();

  for id in gone {
    let _ = write!(out, "{}", escape::Free(id));

    placed.transmitted.remove(&id);
    placed.placements.remove(&id);
  }
}

/// Writes the row `row` of a painted frame under `rendition`: the cursor at its start,
/// the attributes reset, each glyph after the attributes that change for it, a sized
/// glyph as a sized run, the cells a glyph's advance covers left to the terminal, and
/// a block's lower rows stepped over before the next glyph.
fn draw_row(out: &mut impl Write, buffer: &paint::Buffer, row: usize, rendition: Rendition) {
  let depth = rendition.depth;

  let _ = write!(
    out,
    "{}{}",
    move_to(0, u16::try_from(row).unwrap_or(u16::MAX)),
    Sgr::Reset
  );

  let mut current = paint::TerminalCell::EMPTY;
  // The column the terminal's cursor is at: a glyph advances it by its block's
  // columns, and a block's lower rows are stepped over before the next glyph.
  let mut cursor = 0;

  for (column, cell) in buffer.cells[row * buffer.width..(row + 1) * buffer.width]
    .iter()
    .enumerate()
  {
    if cell.glyph == Glyph::Continuation {
      continue;
    }

    if cursor != column {
      let _ = write!(
        out,
        "{}",
        move_to(
          u16::try_from(column).unwrap_or(u16::MAX),
          u16::try_from(row).unwrap_or(u16::MAX)
        )
      );
    }

    cursor = column + cell.glyph.cells() * cell.scale.rows();

    if cell.foreground != current.foreground {
      let color = cell.foreground.map_or(Color::Reset, |rgb| {
        depth.terminal_color(rgb, &rendition.palette)
      });

      let _ = write!(out, "{}", Sgr::Foreground(color));
    }

    if cell.background != current.background {
      let color = cell.background.map_or(Color::Reset, |rgb| {
        depth.terminal_color(rgb, &rendition.palette)
      });

      let _ = write!(out, "{}", Sgr::Background(color));
    }

    if cell.bold != current.bold {
      let _ = write!(out, "{}", Sgr::Bold(cell.bold));
    }

    if cell.decoration != current.decoration {
      rendition.write_decoration(out, current.decoration, cell.decoration);
    }

    if cell.reverse != current.reverse {
      let _ = write!(out, "{}", Sgr::Reverse(cell.reverse));
    }

    // OSC 8 anchors: close the running link on any change, open the next.
    if cell.link != current.link {
      if current.link.is_some() {
        let _ = write!(out, "{}", escape::Hyperlink(""));
      }

      if let Some(url) = cell
        .link
        .and_then(|index| buffer.links.get(usize::from(index)))
      {
        let _ = write!(out, "{}", escape::Hyperlink(url));
      }
    }

    let _ = match cell.scale {
      CellScale::ONE => write!(out, "{}", cell.glyph),
      CellScale::Whole(scale) => write!(out, "\x1b]66;s={scale};{}\x1b\\", cell.glyph),
      CellScale::Fraction(numerator, denominator) => write!(
        out,
        "\x1b]66;n={numerator}:d={denominator}:v=1;{}\x1b\\",
        cell.glyph
      ),
    };

    current = *cell;
  }

  if current.link.is_some() {
    let _ = write!(out, "{}", escape::Hyperlink(""));
  }
}

/// The kitty keyboard flags asked of a terminal that answers the flags query: the
/// disambiguated escape codes, the event types (a release, a repeat), the alternate
/// keys (the shifted key under `Shift`), every key as an escape code (a modifier
/// alone), and the text a key typed (through `AltGr` or `Option` too). A terminal keeps
/// the flags it has.
const KEYBOARD_FLAGS: u8 = escape::DISAMBIGUATE_ESCAPE_CODES
  | escape::REPORT_EVENT_TYPES
  | escape::REPORT_ALTERNATE_KEYS
  | escape::REPORT_ALL_KEYS_AS_ESCAPE_CODES
  | escape::REPORT_ASSOCIATED_TEXT;

/// Dispatches one keyboard press or repeat; see [`press`].
fn dispatch_key(key: &Key) {
  press(&portable_key(key));
}

/// Dispatches one key press or repeat through core's dispatch, then the terminal's own
/// response unless a handler prevented it; `Ctrl-Q` quits unconditionally.
///
/// Where the terminal does not report releases, a release follows every press at once.
fn press(event: &KeyEvent) {
  if event.modifiers.ctrl && event.shortcut_key() == "q" {
    with_store(|store| store.quit = true);

    return;
  }

  interaction::key(event);

  if !with_store(|store| store.key_releases) {
    interaction::key_up(&KeyEvent {
      repeat: false,
      ..event.clone()
    });
  }
}

/// Maps a terminal key to the portable W3C key value and code: a key carries `Shift`
/// when it arrives with its shifted alternate applied, or uppercase, and Caps Lock
/// flips a letter's case, so a shortcut reads the modifiers and a text entry the case
/// as a browser's `key` gives them; the code is the key's position on the standard
/// layout, from the base-layout alternate where the terminal reports one.
///
/// Where the terminal reports the text a key typed, the text is the key, as a
/// browser's `key` is, and `Ctrl` and `Alt` are spent in typing it (`AltGr`, `Option`), so
/// a text entry inserts it.
fn portable_key(key: &Key) -> KeyEvent {
  let mut modifiers = key.modifiers;
  let keypad = key.keypad;
  let key_value = match (key.text, key.code) {
    (Some(text), _) => {
      modifiers.ctrl = false;
      modifiers.alt = false;

      PortableKey::Character(text.to_string())
    }
    (None, code) => portable_key_value(key, code, &mut modifiers),
  };
  let code = match key.code {
    KeyCode::Char(character) if keypad => keypad_code(character),
    KeyCode::Char(character) => KeyEvent::code_of(&PortableKey::Character(
      key.base_layout.unwrap_or(character).to_string(),
    )),
    KeyCode::Enter if keypad => Code::NumpadEnter,
    KeyCode::KeypadBegin => Code::Numpad5,
    KeyCode::Modifier(modifier) => modifier_code(modifier),
    _ => KeyEvent::code_of(&key_value),
  };

  if key.code == KeyCode::BackTab {
    modifiers.shift = true;
  }

  KeyEvent {
    key: key_value,
    code,
    modifiers,
    repeat: key.kind == KeyKind::Repeat,
    composing: false,
  }
}

/// The W3C key value of `code`, a key reporting no text, marking `Shift` in `modifiers`
/// where the key arrived shifted.
#[expect(
  deprecated,
  reason = "the terminal reports a Hyper key, which the W3C names by its legacy value"
)]
fn portable_key_value(key: &Key, code: KeyCode, modifiers: &mut Modifiers) -> PortableKey {
  let named = match code {
    KeyCode::Char(character) => {
      modifiers.shift |= key.shifted.is_some() || character.is_uppercase();

      return PortableKey::Character(if key.caps_lock && character.is_alphabetic() {
        if character.is_uppercase() {
          character.to_lowercase().collect()
        }
        else {
          character.to_uppercase().collect()
        }
      }
      else {
        character.to_string()
      });
    }
    KeyCode::Modifier(modifier) => match modifier {
      ModifierKey::LeftShift | ModifierKey::RightShift => NamedKey::Shift,
      ModifierKey::LeftControl | ModifierKey::RightControl => NamedKey::Control,
      ModifierKey::LeftAlt | ModifierKey::RightAlt => NamedKey::Alt,
      ModifierKey::LeftSuper
      | ModifierKey::RightSuper
      | ModifierKey::LeftMeta
      | ModifierKey::RightMeta => NamedKey::Meta,
      ModifierKey::LeftHyper | ModifierKey::RightHyper => NamedKey::Hyper,
      ModifierKey::IsoLevel3Shift => NamedKey::AltGraph,
      ModifierKey::IsoLevel5Shift => NamedKey::Unidentified,
    },
    KeyCode::Enter => NamedKey::Enter,
    KeyCode::Backspace => NamedKey::Backspace,
    KeyCode::Tab | KeyCode::BackTab => NamedKey::Tab,
    KeyCode::Escape => NamedKey::Escape,
    KeyCode::Delete => NamedKey::Delete,
    KeyCode::Left => NamedKey::ArrowLeft,
    KeyCode::Right => NamedKey::ArrowRight,
    KeyCode::Up => NamedKey::ArrowUp,
    KeyCode::Down => NamedKey::ArrowDown,
    KeyCode::Home => NamedKey::Home,
    KeyCode::End => NamedKey::End,
    KeyCode::PageUp => NamedKey::PageUp,
    KeyCode::PageDown => NamedKey::PageDown,
    KeyCode::Insert => NamedKey::Insert,
    KeyCode::Function(number) => {
      NamedKey::from_str(&format!("F{number}")).unwrap_or(NamedKey::Unidentified)
    }
    _ => NamedKey::Unidentified,
  };

  PortableKey::Named(named)
}

/// The W3C code of the keypad key producing `character`.
fn keypad_code(character: char) -> Code {
  match character {
    '0'..='9' => Code::from_str(&format!("Numpad{character}")).unwrap_or(Code::Unidentified),
    '.' => Code::NumpadDecimal,
    '/' => Code::NumpadDivide,
    '*' => Code::NumpadMultiply,
    '-' => Code::NumpadSubtract,
    '+' => Code::NumpadAdd,
    '=' => Code::NumpadEqual,
    ',' => Code::NumpadComma,
    _ => Code::Unidentified,
  }
}

/// The W3C code of the modifier key `modifier`, by its side.
const fn modifier_code(modifier: ModifierKey) -> Code {
  match modifier {
    ModifierKey::LeftShift => Code::ShiftLeft,
    ModifierKey::RightShift => Code::ShiftRight,
    ModifierKey::LeftControl => Code::ControlLeft,
    ModifierKey::RightControl => Code::ControlRight,
    ModifierKey::LeftAlt => Code::AltLeft,
    ModifierKey::RightAlt | ModifierKey::IsoLevel3Shift => Code::AltRight,
    ModifierKey::LeftSuper | ModifierKey::LeftMeta => Code::MetaLeft,
    ModifierKey::RightSuper | ModifierKey::RightMeta => Code::MetaRight,
    ModifierKey::LeftHyper | ModifierKey::RightHyper | ModifierKey::IsoLevel5Shift => {
      Code::Unidentified
    }
  }
}

/// The key event `spec` names: a portable W3C key value (`"a"`, `"Enter"`, `"Tab"`,
/// `"Backspace"`, …) after the `"Ctrl+"`, `"Alt+"` and `"Shift+"` prefixes adding
/// its modifiers (`"Ctrl+ArrowLeft"`, `"Shift+Tab"`).
pub(crate) fn key_event(spec: &str) -> KeyEvent {
  let mut modifiers = Modifiers::default();
  let mut key = spec;

  while let Some((modifier, rest)) = key.split_once('+') {
    match modifier {
      "Ctrl" => modifiers.ctrl = true,
      "Alt" => modifiers.alt = true,
      "Shift" => modifiers.shift = true,
      _ => break,
    }

    key = rest;
  }

  KeyEvent {
    modifiers,
    ..KeyEvent::new(key)
  }
}

/// Puts the text measurement on the terminal's grapheme clustering or off it, every
/// entry measured again and the frame laid out afresh; `true` when that changed it.
fn apply_clustering(confirmed: bool) -> bool {
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

/// Simulates the terminal's report that grapheme clustering (mode 2027) is set, or
/// that it is not, for a test driving the loop headlessly.
pub fn trigger_grapheme_clustering(confirmed: bool) {
  trigger(|| {
    apply_clustering(confirmed);
  });
}

/// Puts the text measurement on the terminal's text sizing (OSC 66) or off it, every
/// run measured at its size or at one cell and the frame laid out afresh; `true`
/// when that changed it.
fn apply_sizing(confirmed: bool) -> bool {
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

/// Simulates the terminal's reply that proves text sizing (OSC 66), or one that does
/// not, for a test driving the loop headlessly.
pub fn trigger_text_sizing(confirmed: bool) {
  trigger(|| {
    apply_sizing(confirmed);
  });
}

/// Puts the graphics on or off: the capability signal moves, so an enhancement on it
/// builds its painted form or its fallback, and the next frame paints whole; `true`
/// when that changed it.
fn apply_graphics(confirmed: bool) -> bool {
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

/// Simulates the terminal's reply that proves the graphics protocol, or its absence,
/// for a test driving the loop headlessly.
pub fn trigger_graphics(confirmed: bool) {
  trigger(|| {
    apply_graphics(confirmed);
  });
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

/// Runs `event` as one turn of the loop, for a test driving it headlessly: what earlier
/// writes scheduled takes effect first, and what the event's own writes schedule once
/// it is handled.
fn trigger(event: impl FnOnce()) {
  end_turn();
  event();
  end_turn();
}

/// Simulates one key press by its portable W3C key value, for a test driving the loop
/// headlessly.
///
/// The value is a key's name (`"a"`, `"Enter"`, `"Tab"`, `"Backspace"`, …), and
/// `"Ctrl+"`, `"Alt+"`, and `"Shift+"` prefixes add the modifier (`"Ctrl+ArrowLeft"`,
/// `"Shift+Tab"`). Until [`trigger_key_releases`] confirms releases, the key's release
/// follows at once.
pub fn trigger_key(key: &str) {
  trigger(|| press(&key_event(key)));
}

/// Simulates one key press as `event` describes it—its key value, its physical code
/// and its modifiers—for a test driving a layout the key value alone does not place.
pub fn trigger_key_event(event: &KeyEvent) {
  trigger(|| press(event));
}

/// Whether a key asked the app to quit, for a headless test.
#[must_use]
pub fn quit_requested() -> bool {
  end_turn();
  with_store(|store| store.quit)
}

/// Simulates one key release by its portable W3C key value, with the prefixes
/// [`trigger_key`] reads, for a test driving the loop headlessly.
pub fn trigger_key_up(key: &str) {
  trigger(|| interaction::key_up(&key_event(key)));
}

/// The node holding focus, for a test asserting focus movement.
#[must_use]
pub fn focused_node() -> Option<skene_core::NodeId> {
  end_turn();
  with_store(|store| {
    store
      .focused()
      .and_then(|focused| store.record(focused).and_then(|record| record.node))
  })
}

/// Simulates the mouse moving to the cell (`x`, `y`), for a test driving hover
/// headlessly. Hit-testing resolves against the last render's layout.
pub fn trigger_mouse_move(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| store.pointer_within = crate::store::CELL_CENTER);
    interaction::pointer_move(x, y, None, Modifiers::default());
  });
}

/// Simulates a left click at the cell (`x`, `y`), for a test driving mouse interaction
/// headlessly. Hit-testing resolves against the last render's layout.
pub fn trigger_mouse_click(x: u16, y: u16) {
  trigger_mouse_press(x, y);
  trigger_mouse_release(x, y);
}

/// Simulates pressing the left button at (`x`, `y`) without releasing—the start of a
/// drag—for a test driving drags headlessly.
pub fn trigger_mouse_press(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| {
      store.pointer_within = crate::store::CELL_CENTER;
      store.buttons.insert(PointerButton::Primary);
    });
    interaction::pointer_press(x, y, PointerButton::Primary, Modifiers::default());
  });
}

/// Simulates dragging to the center of the cell (`x`, `y`) with the left button held,
/// after [`trigger_mouse_press`].
pub fn trigger_mouse_drag(x: u16, y: u16) {
  trigger_mouse_drag_within(x, y, crate::store::CELL_CENTER);
}

/// Simulates dragging to the cell (`x`, `y`) with the left button held, the pointer
/// `within` the cell—`0..1` along each axis, as a terminal reporting pixels places
/// it—after [`trigger_mouse_press`].
pub fn trigger_mouse_drag_within(x: u16, y: u16, within: (f32, f32)) {
  trigger(|| {
    with_store(|store| store.pointer_within = within);
    interaction::pointer_move(x, y, Some(PointerButton::Primary), Modifiers::default());
  });
}

/// Simulates releasing the left button at (`x`, `y`), ending a drag. A release with no
/// press delivered stands in for the whole click, as the interactive loop's does.
pub fn trigger_mouse_release(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| store.pointer_within = crate::store::CELL_CENTER);

    if !with_store(|store| store.buttons.contains(PointerButton::Primary)) {
      with_store(|store| store.buttons.insert(PointerButton::Primary));
      interaction::pointer_press(x, y, PointerButton::Primary, Modifiers::default());
    }

    with_store(|store| store.buttons.remove(PointerButton::Primary));
    interaction::pointer_release(x, y, PointerButton::Primary, Modifiers::default());
  });
}

/// Simulates a right click at the cell (`x`, `y`)—the copy-menu gesture—for a test
/// driving selection headlessly.
pub fn trigger_mouse_right_click(x: u16, y: u16) {
  trigger(|| {
    with_store(|store| {
      store.pointer_within = crate::store::CELL_CENTER;
      store.buttons.insert(PointerButton::Secondary);
    });
    interaction::pointer_press(x, y, PointerButton::Secondary, Modifiers::default());
    with_store(|store| store.buttons.remove(PointerButton::Secondary));
    interaction::pointer_release(x, y, PointerButton::Secondary, Modifiers::default());
  });
}

/// Simulates the mouse wheel at the cell (`x`, `y`), for a headless test driving
/// scrolling.
///
/// Positive deltas scroll right and down, one cell per unit; hit-testing resolves
/// against the last render's layout.
pub fn trigger_mouse_scroll(x: u16, y: u16, delta_columns: i64, delta_rows: i64) {
  trigger(|| interaction::wheel(x, y, delta_columns, delta_rows));
}

/// Takes the text the last `Copy` staged for the clipboard, for a headless test.
#[must_use]
pub fn take_copied() -> Option<String> {
  end_turn();
  with_store(|store| store.pending_copy.take())
}

/// Takes whether the last `Paste` staged a clipboard read, for a headless test.
#[must_use]
pub fn take_paste_requested() -> bool {
  end_turn();
  with_store(|store| std::mem::take(&mut store.pending_paste))
}

/// Simulates the terminal pasting `text` by its own key—a bracketed paste—into the
/// focused entry, for a headless test.
pub fn trigger_paste(text: &str) {
  trigger(|| interaction::paste(text));
}

/// Simulates the terminal answering the last clipboard read with `text`, for a headless
/// test: it lands in the entry that asked, wherever focus has moved since.
pub fn trigger_clipboard_reply(text: &str) {
  trigger(|| interaction::paste_reply(text));
}

/// Takes the link the last click staged for the system handler, for a headless test.
#[must_use]
pub fn take_opened() -> Option<String> {
  end_turn();
  with_store(|store| store.pending_open.take())
}

/// The current terminal title, for a headless test.
#[must_use]
pub fn current_title() -> Option<String> {
  end_turn();
  with_store(|store| store.title.clone())
}

/// Simulates the terminal reporting `scheme` as its background, for a headless test
/// driving the scheme conditions; the app's own choice still wins.
pub fn trigger_color_scheme(scheme: ColorScheme) {
  trigger(|| set_detected_scheme(scheme == ColorScheme::Dark));
}

/// Simulates the terminal reporting its own `foreground` and `background` (OSC 10 and
/// 11), for a headless test asserting what a paint mixes against; the scheme follows
/// the background's luminance.
pub fn trigger_terminal_colors(foreground: (u8, u8, u8), background: (u8, u8, u8)) {
  trigger(|| {
    with_store(|store| store.detected_foreground = Some(foreground));
    set_detected_background(background);
  });
}

/// Steps the motion clock forward by `elapsed`, for a headless test driving transitions,
/// animations and a caret drag's autoscroll frame by frame; the next render samples
/// them at the new reading.
///
/// A headless platform's clock stands still otherwise, so a render shows a transition
/// at the point the last step left it.
pub fn trigger_frame(elapsed: Duration) {
  trigger(|| {
    with_store(|store| {
      store.clock.advance(elapsed.as_secs_f64());
      store.motion_active = true;
    });
    run_due_timers();
  });
}

/// Simulates the reader's reduced-motion preference, for a headless test driving the
/// reduced-motion condition and the motion that respects it.
pub fn trigger_reduced_motion(reduced: bool) {
  trigger(|| {
    let write = with_store(|store| {
      if store.reduced_motion == reduced {
        return None;
      }

      store.reduced_motion = reduced;
      store.invalidate_styles();

      Some(store.media().set_reduced_motion)
    });

    if let Some(write) = write {
      skene_reactive::Set::set(&write, reduced);
    }
  });
}

/// Simulates the terminal resizing to `columns` x `rows`, for a headless test driving
/// the size conditions; the next render lays out at the size it is given.
pub fn trigger_resize(columns: u16, rows: u16) {
  trigger(|| frame::resize(columns, rows));
}

/// The pointer shape the current hover resolves to, for a headless test.
#[must_use]
pub fn current_cursor() -> Option<MouseCursor> {
  end_turn();
  with_store(|store| store.hover_cursor())
}

/// Whether the loop would draw a frame now, something having changed since the last
/// render, for a test asserting that a change schedules one.
#[must_use]
pub fn frame_pending() -> bool {
  end_turn();
  with_store(|store| store.needs_frame())
}

/// How many layouts have been computed since the platform was installed, for a test
/// asserting that a change repainted without a relayout.
#[must_use]
pub fn layout_generation() -> u64 {
  end_turn();
  with_store(|store| store.layout_generation)
}

/// How many taffy styles have been written since the platform was installed, for a test
/// counting the restyles a change caused.
#[must_use]
pub fn set_style_calls() -> u64 {
  end_turn();
  with_store(|store| store.set_style_calls)
}

/// How many node styles have been resolved from their declarations since the platform
/// was installed, for a test counting what a frame or a change resolved.
#[must_use]
pub fn style_resolutions() -> u64 {
  end_turn();
  with_store(|store| store.style_resolutions)
}

/// The rows the last render rewrote, for a test asserting that a change repainted only
/// the rows it touched.
#[must_use]
pub fn repainted_rows() -> Vec<usize> {
  end_turn();
  with_store(|store| store.repainted.clone())
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Sixteen colors and plain underlines.
  const PLAIN: Rendition = Rendition {
    depth: ColorDepth::Ansi16,
    palette: detect::Palette::DEFAULT,
    styled_underlines: false,
  };

  /// A styled, colored underline is written as its style and color where the terminal
  /// kept the probe, and as a plain underline elsewhere; an overline and a line through
  /// are written as their own attributes either way.
  #[test]
  fn decorations_are_written_as_the_terminal_keeps_them() {
    let mut buffer = paint::Buffer::new(3, 1);

    buffer.cells[0].decoration = paint::CellDecoration {
      underline: Some(TextDecorationStyle::Wavy),
      underline_color: Some((255, 0, 0)),
      ..paint::CellDecoration::NONE
    };
    buffer.cells[1].decoration = paint::CellDecoration {
      overline: true,
      ..paint::CellDecoration::NONE
    };
    buffer.cells[2].decoration = paint::CellDecoration {
      line_through: true,
      ..paint::CellDecoration::NONE
    };

    let mut styled = Vec::new();
    let mut plain = Vec::new();

    draw(
      &mut styled,
      &buffer,
      Rendition {
        depth: ColorDepth::TrueColor,
        palette: detect::Palette::DEFAULT,
        styled_underlines: true,
      },
      None,
      false,
      &mut Placed::default(),
    );
    draw(
      &mut plain,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    let styled = String::from_utf8(styled).unwrap();
    let plain = String::from_utf8(plain).unwrap();

    assert!(styled.contains("\x1b[4:3m"), "{styled:?}");
    assert!(styled.contains("\x1b[58:2::255:0:0m"), "{styled:?}");
    assert!(
      styled.contains("\x1b[53m") && styled.contains("\x1b[55m"),
      "{styled:?}"
    );
    assert!(styled.contains("\x1b[9m"), "{styled:?}");
    assert!(
      plain.contains("\x1b[4m") && !plain.contains("4:3"),
      "{plain:?}"
    );
    assert!(!plain.contains("58:"), "{plain:?}");
    assert!(
      plain.contains("\x1b[53m") && plain.contains("\x1b[9m"),
      "{plain:?}"
    );
  }

  /// A modifier pressed alone maps to its W3C key value, and a repeat carries the
  /// flag.
  #[test]
  fn modifier_keys_and_repeats_map_to_the_portable_vocabulary() {
    let shift = portable_key(&Key::new(KeyCode::Modifier(ModifierKey::LeftShift)).with(
      Modifiers {
        shift: true,
        ..Modifiers::default()
      },
    ));
    let meta = portable_key(&Key::new(KeyCode::Modifier(ModifierKey::RightSuper)));
    let mut held = Key::new(KeyCode::Char('a'));

    held.kind = KeyKind::Repeat;

    let held = portable_key(&held);

    assert!(shift.is(NamedKey::Shift));
    assert!(shift.is_modifier());
    assert!(meta.is(NamedKey::Meta));
    assert!(held.repeat && !held.is_modifier());
  }

  /// An uppercase letter carries `Shift` even where the terminal dropped it for the
  /// shifted key, and Caps Lock flips a letter's case without touching the modifiers.
  #[test]
  fn letter_case_follows_shift_and_caps_lock() {
    let shifted = portable_key(&Key::new(KeyCode::Char('Z')).with(Modifiers {
      ctrl: true,
      ..Modifiers::default()
    }));
    let mut locked = Key::new(KeyCode::Char('z')).with(Modifiers {
      ctrl: true,
      ..Modifiers::default()
    });

    locked.caps_lock = true;

    let mut locked_shifted = locked;

    locked_shifted.code = KeyCode::Char('Z');

    let locked_shifted = portable_key(&locked_shifted);
    let locked = portable_key(&locked);

    assert!(shifted.types("Z"));
    assert!(shifted.modifiers.shift && shifted.modifiers.ctrl);
    assert!(locked.types("Z"));
    assert!(!locked.modifiers.shift && locked.modifiers.ctrl);
    assert!(locked_shifted.types("z"));
    assert!(locked_shifted.modifiers.shift);
  }

  /// A shifted alternate the terminal applied restores `Shift` for any key, and the
  /// code is the base-layout alternate's position where one is reported, else the
  /// key's own on the standard layout, the keypad's keys and the modifiers' sides by
  /// their own names.
  #[test]
  fn alternate_keys_restore_shift_and_name_the_physical_key() {
    let mut bang = Key::new(KeyCode::Char('!'));

    bang.shifted = Some('!');

    let bang = portable_key(&bang);

    assert!(bang.types("!"));
    assert_eq!(bang.code, Code::Digit1);
    assert!(bang.modifiers.shift);

    let mut cyrillic = Key::new(KeyCode::Char('\u{444}')).with(Modifiers {
      ctrl: true,
      ..Modifiers::default()
    });

    cyrillic.base_layout = Some('a');

    let cyrillic = portable_key(&cyrillic);

    assert!(cyrillic.types("\u{444}"));
    assert_eq!(cyrillic.code, Code::KeyA);
    assert!(cyrillic.modifiers.ctrl && !cyrillic.modifiers.shift);

    let mut keypad = Key::new(KeyCode::Char('7'));

    keypad.keypad = true;

    assert_eq!(portable_key(&keypad).code, Code::Numpad7);
    assert_eq!(
      portable_key(&Key::new(KeyCode::Modifier(ModifierKey::RightShift))).code,
      Code::ShiftRight
    );
    assert_eq!(portable_key(&Key::new(KeyCode::Left)).code, Code::ArrowLeft);
    assert_eq!(key_event("Ctrl+z").code, Code::KeyZ);
  }

  /// A key reporting the text it typed is that text, the `Ctrl` and `Alt` spent in
  /// typing it cleared, so an entry inserts what `AltGr` or `Option` typed.
  #[test]
  fn a_typed_text_is_the_key() {
    let mut option = Key::new(KeyCode::Char('2')).with(Modifiers {
      alt: true,
      ..Modifiers::default()
    });

    option.text = Some('\u{2122}');

    let option = portable_key(&option);

    assert!(option.types("\u{2122}"));
    assert_eq!(option.code, Code::Digit2);
    assert!(!option.modifiers.alt && !option.modifiers.ctrl);
  }

  /// In-band resize notifications are asked for, queried and handed back with the
  /// other modes.
  #[test]
  fn in_band_resize_is_set_queried_and_reset() {
    let mut setup = Vec::new();
    let mut queries = Vec::new();
    let mut restore = Vec::new();

    write_setup(&mut setup, false).unwrap();
    write_queries(&mut queries).unwrap();
    write_restore(&mut restore).unwrap();

    assert!(String::from_utf8(setup).unwrap().contains("\x1b[?2048h"));
    let queries = String::from_utf8(queries).unwrap();

    assert!(queries.contains("\x1b[?2048$p"));
    assert!(queries.contains("\x1b[?5522$p"));
    assert!(String::from_utf8(restore).unwrap().contains("\x1b[?2048l"));
  }

  /// The mouse modes are set together and reset together.
  #[test]
  fn the_mouse_modes_are_written_as_one() {
    let mut on = Vec::new();
    let mut off = Vec::new();

    write_mouse(&mut on, true).unwrap();
    write_mouse(&mut off, false).unwrap();

    assert_eq!(
      String::from_utf8(on).unwrap(),
      "\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h"
    );
    assert_eq!(
      String::from_utf8(off).unwrap(),
      "\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l"
    );
  }

  /// Every pointer shape the GUI's `cursor` takes has an OSC 22 name, and the
  /// restoring controls hand the shape back to the terminal.
  #[test]
  fn pointer_shapes_are_named_and_restored() {
    assert_eq!(pointer_shape(Some(MouseCursor::Click)), "pointer");
    assert_eq!(pointer_shape(Some(MouseCursor::Text)), "text");
    assert_eq!(pointer_shape(Some(MouseCursor::Grab)), "grab");
    assert_eq!(pointer_shape(Some(MouseCursor::Grabbing)), "grabbing");
    assert_eq!(pointer_shape(Some(MouseCursor::Default)), "default");
    assert_eq!(pointer_shape(None), "default");

    let mut restore = Vec::new();

    write_restore(&mut restore).unwrap();

    assert!(
      String::from_utf8(restore)
        .unwrap()
        .contains("\x1b]22;default\x1b\\")
    );
  }

  /// A scaled cell is written as a sized run, the cells its block covers on its row
  /// are left to the terminal's advance, and a block's lower rows are stepped over
  /// before the next glyph; a fraction names its parts and sits at the cell's foot.
  #[test]
  fn sized_cells_are_written_as_sized_runs_and_their_blocks_stepped_over() {
    let mut buffer = paint::Buffer::new(4, 2);

    buffer.put_sized(0, 0, "H", 1, CellScale::Whole(2), |_| {});
    buffer.put_sized(2, 0, "i", 1, CellScale::Fraction(1, 2), |_| {});
    buffer.put_cluster(2, 1, "x", 1, |_| {});

    let mut out = Vec::new();

    draw(
      &mut out,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    let out = String::from_utf8(out).unwrap();

    assert!(
      out.contains("\x1b]66;s=2;H\x1b\\\x1b]66;n=1:d=2:v=1;i\x1b\\ "),
      "{out:?}"
    );
    assert!(
      out.contains("\x1b[2;1H\x1b[m\x1b[2;3Hx "),
      "the second row steps past the block: {out:?}"
    );
  }

  /// At sixteen colors a painted RGB is written as the nearest base color the
  /// terminal reported, not the nearest default.
  #[test]
  fn sixteen_colors_pick_from_the_reported_palette() {
    let mut buffer = paint::Buffer::new(1, 1);

    buffer.cells[0].foreground = Some((40, 44, 52));

    let mut palette = detect::Palette::DEFAULT;

    palette.set(4, (40, 44, 52));

    let mut reported = Vec::new();
    let mut plain = Vec::new();

    draw(
      &mut reported,
      &buffer,
      Rendition { palette, ..PLAIN },
      None,
      false,
      &mut Placed::default(),
    );
    draw(
      &mut plain,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    assert!(String::from_utf8(reported).unwrap().contains("\x1b[34m"));
    assert!(String::from_utf8(plain).unwrap().contains("\x1b[30m"));
  }

  /// A picture is transmitted once per image generation and placed over its cells,
  /// placed again without transmission where its cells move, and freed once it is
  /// gone from the frame.
  #[test]
  fn pictures_are_transmitted_once_placed_on_change_and_freed_when_gone() {
    let image = std::rc::Rc::new(paint::Image {
      generation: 1,
      width: 2,
      height: 1,
      pixels: vec![255, 0, 0, 255, 0, 0, 255, 255],
    });
    let placement = |column| paint::Placement {
      id: 3,
      column,
      row: 1,
      columns: 2,
      rows: 1,
      crop: (0, 0, 2, 1),
      layer: 0,
      image: std::rc::Rc::clone(&image),
    };
    let mut buffer = paint::Buffer::new(6, 2);

    buffer.pictures.push(placement(0));

    let mut placed = Placed::default();
    let mut first = Vec::new();

    draw(&mut first, &buffer, PLAIN, None, false, &mut placed);

    let first = String::from_utf8(first).unwrap();

    assert!(
      first.contains("\x1b_Ga=t,f=32,s=2,v=1,i=3,q=2,m=0;/wAA/wAA//8=\x1b\\"),
      "{first:?}"
    );
    assert!(
      first.contains("\x1b[2;1H\x1b_Ga=p,i=3,p=1,c=2,r=1,x=0,y=0,w=2,h=1,z=0,C=1,q=2\x1b\\"),
      "{first:?}"
    );

    let mut same = Vec::new();

    draw(&mut same, &buffer, PLAIN, Some(&buffer), false, &mut placed);

    assert!(!String::from_utf8(same).unwrap().contains("\x1b_G"));

    let mut moved_frame = paint::Buffer::new(6, 2);

    moved_frame.pictures.push(placement(2));

    let mut moved = Vec::new();

    draw(
      &mut moved,
      &moved_frame,
      PLAIN,
      Some(&buffer),
      false,
      &mut placed,
    );

    let moved = String::from_utf8(moved).unwrap();

    assert!(!moved.contains("a=t,"), "{moved:?}");
    assert!(
      moved.contains("\x1b_Ga=d,d=i,i=3,q=2\x1b\\\x1b[2;3H\x1b_Ga=p,i=3"),
      "{moved:?}"
    );

    let mut gone = Vec::new();

    draw(
      &mut gone,
      &paint::Buffer::new(6, 2),
      PLAIN,
      Some(&moved_frame),
      false,
      &mut placed,
    );

    assert!(
      String::from_utf8(gone)
        .unwrap()
        .contains("\x1b_Ga=d,d=I,i=3,q=2\x1b\\")
    );
    assert!(placed.transmitted.is_empty());
  }

  /// Launch probes text sizing from the home cell with a space at scale two and asks
  /// where the cursor landed, and fences its queries with the primary device
  /// attributes request.
  #[test]
  fn text_sizing_is_probed_from_the_home_cell() {
    let mut out = Vec::new();

    write_queries(&mut out).unwrap();

    let out = String::from_utf8(out).unwrap();

    assert!(
      out.ends_with(
        "\x1b[1;1H\x1b]66;s=2; \x1b\\\x1b[6n\x1b_Gi=2147483647,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\
         \x1b[c"
      ),
      "{out:?}"
    );
  }

  /// A session is remote where any of ssh's variables is set.
  #[test]
  fn ssh_makes_a_session_remote() {
    assert!(!is_remote(|_| false));
    assert!(is_remote(|name| name == "SSH_TTY"));
    assert!(is_remote(|name| name == "SSH_CONNECTION"));
  }

  /// A theme change asks for the terminal's own colors afresh, fenced by the primary
  /// device attributes request.
  #[test]
  fn the_colors_are_asked_for_in_a_window_of_their_own() {
    let mut out = Vec::new();

    write_color_queries(&mut out).unwrap();

    let out = String::from_utf8(out).unwrap();

    assert!(out.starts_with("\x1b]11;?"), "{out:?}");
    assert!(out.contains("\x1b]10;?"), "{out:?}");
    assert!(out.ends_with("\x1b[c"), "{out:?}");
  }

  /// A frame is bracketed by mode 2026 when the terminal confirmed it can toggle the
  /// mode, and written plain otherwise; either way the cursor hides while it is
  /// written.
  #[test]
  fn a_frame_is_bracketed_by_synchronized_output_when_confirmed() {
    let buffer = paint::Buffer::new(2, 1);
    let mut synchronized = Vec::new();
    let mut plain = Vec::new();

    draw(
      &mut synchronized,
      &buffer,
      PLAIN,
      None,
      true,
      &mut Placed::default(),
    );
    draw(
      &mut plain,
      &buffer,
      PLAIN,
      None,
      false,
      &mut Placed::default(),
    );

    let synchronized = String::from_utf8(synchronized).unwrap();
    let plain = String::from_utf8(plain).unwrap();

    assert!(synchronized.starts_with("\x1b[?2026h"), "{synchronized:?}");
    assert!(synchronized.ends_with("\x1b[?2026l"), "{synchronized:?}");
    assert_eq!(
      &synchronized["\x1b[?2026h".len()..synchronized.len() - "\x1b[?2026l".len()],
      plain
    );
    assert!(!plain.contains("2026"), "{plain:?}");
    assert!(plain.starts_with("\x1b[?25l"), "{plain:?}");
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
