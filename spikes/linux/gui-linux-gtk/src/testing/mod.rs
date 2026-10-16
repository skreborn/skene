//! `skene_gui::testing` on GTK: the [`Driver`] a test reads and acts through.
//!
//! - **Reads** go through the accessibility layer, as a screen reader's do: AT-SPI over D-Bus
//!   ([`atspi`]), from the test's own process.
//! - **Actions** are real input: XTest events ([`input`]) from the test process, at the bounds
//!   GTK laid out the widget matching the AT-SPI object (its role, its name where the widget
//!   shows one, and the nearest bounds).
//! - **Settling** syncs with the X server, drains the main context, then waits for a quiet
//!   window in which no frame is painted and nothing is reported; a find looks again for up
//!   to a second.
//!
//! The in-process widget tree and record are used only to match an object to the widget input
//! goes to, never to assert what a reader finds.

pub mod atspi;
pub mod input;

use std::{
  cell::{Cell, OnceCell, RefCell},
  rc::Rc,
  time::{Duration, Instant},
};

use gtk::{gdk, glib, glib::translate::IntoGlib as _, graphene, prelude::*};
use skene_foundation::{
  NodeId, Owner, Scheme,
  layout::Rect,
  seam::{
    access::{Checked, Role},
    activity,
    content::{Graphemes, TextSelection, Unit},
    report::Modifiers,
    testing::{AccessSnapshot, Action, Driver, EnvironmentRequest, Issue, Query, Screenshot},
  },
};

use self::{
  atspi::{Atspi, Item, Object, state},
  input::{Input, keysym},
};
use crate::{app, environment, widget};

/// How long `settle` must see no frame and no report: above two 60 Hz frames.
const QUIET: Duration = Duration::from_millis(30);

/// How long a find looks again before it gives up.
const FIND_TIMEOUT: Duration = Duration::from_secs(1);

/// How long the platform may stay busy before `settle` fails.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);

thread_local! {
  static ATSPI: OnceCell<&'static Atspi> = const { OnceCell::new() };
  static INPUT: OnceCell<&'static Input> = const { OnceCell::new() };
  static WINDOWS: Cell<u32> = const { Cell::new(0) };
  static INITIAL: RefCell<Option<Initial>> = const { RefCell::new(None) };
}

/// The settings a test may change, as the session started them.
struct Initial {
  animations: bool,
  dark: bool,
  theme: Option<glib::GString>,
}

/// The process's accessibility client, connected on first use.
///
/// # Panics
///
/// Where there is no accessibility bus: the display runner gives every test one.
fn atspi() -> &'static Atspi {
  ATSPI.with(|cell| {
    *cell.get_or_init(|| {
      Box::leak(Box::new(Atspi::connect(Duration::from_secs(10)).unwrap_or_else(
        |error| {
          panic!(
            "AT-SPI: {error}; tests run through the display runner (`skene-runner`), which gives each a session and accessibility bus"
          )
        },
      )))
    })
  })
}

/// The process's XTest connection, opened on first use.
///
/// # Panics
///
/// Where the display has no XTest.
fn input() -> &'static Input {
  INPUT.with(|cell| {
    *cell.get_or_init(|| {
      Box::leak(Box::new(
        Input::connect().unwrap_or_else(|error| panic!("XTest: {error}")),
      ))
    })
  })
}

/// Puts what a previous test may have changed back where the session started it: the
/// animations setting, the reader's scheme and theme. What `#[skene_gui::test]` runs first.
pub fn reset() {
  let Some(settings) = gtk::Settings::default() else {
    return;
  };

  INITIAL.with(|initial| {
    let mut initial = initial.borrow_mut();
    let initial = initial.get_or_insert_with(|| Initial {
      animations: settings.is_gtk_enable_animations(),
      dark: settings.is_gtk_application_prefer_dark_theme(),
      theme: settings.gtk_theme_name(),
    });

    if settings.is_gtk_enable_animations() != initial.animations {
      settings.set_gtk_enable_animations(initial.animations);
    }

    if settings.is_gtk_application_prefer_dark_theme() != initial.dark {
      settings.set_gtk_application_prefer_dark_theme(initial.dark);
    }

    if settings.gtk_theme_name() != initial.theme {
      settings.set_gtk_theme_name(initial.theme.as_deref());
    }
  });
}

/// Runs the main context for `duration`, dispatching whatever becomes ready.
pub fn wait(duration: Duration) {
  let context = glib::MainContext::default();
  let done = Rc::new(Cell::new(false));
  let _timer = glib::timeout_add_local_once(duration, {
    let done = Rc::clone(&done);

    move || done.set(true)
  });

  while !done.get() {
    context.iteration(true);
  }
}

/// Runs the platform until it is idle: the input sent delivered (the X server synced with,
/// GTK's own connection too), everything ready dispatched, then one quiet window in which no
/// frame was painted and nothing reported.
///
/// # Panics
///
/// Where frames or reports keep coming for five seconds (an endless animation).
pub fn settle() {
  let started = Instant::now();

  if INPUT.with(|cell| cell.get().is_some()) {
    let _ = input().sync();
  }

  if let Some(display) = gdk::Display::default() {
    display.sync();
  }

  loop {
    app::drain();

    let before = activity::now();

    wait(QUIET);

    if activity::now() == before && !glib::MainContext::default().pending() {
      return;
    }

    assert!(
      started.elapsed() < SETTLE_TIMEOUT,
      "the window did not settle within {SETTLE_TIMEOUT:?}: frames or reports kept coming ({:?} then {:?})",
      before,
      activity::now()
    );
  }
}

/// A node a reader found: its object on the accessibility bus, and what was read of it then.
#[derive(Clone, PartialEq, Debug)]
pub struct Element {
  /// The object on the accessibility bus.
  pub object: Object,
  /// Its role, mapped back to Skene's.
  pub role: Role,
  /// AT-SPI's name for its role.
  pub platform_role: String,
  /// Its accessible name when it was found.
  pub name: String,
}

/// Skene's role for AT-SPI's role name, as GTK 4.14 maps its accessible roles to AT-SPI's.
#[must_use]
pub fn role_of(platform_role: &str, states: u64) -> Role {
  match platform_role {
    "push button" | "button" | "toggle button" => Role::Button,
    "switch" => Role::Switch,
    "check box" | "checkbox" => Role::Checkbox,
    "radio button" => Role::Radio,
    "slider" => Role::Slider,
    "spin button" => Role::SpinButton,
    "text" | "entry" | "password text" | "text box" => {
      if states & (1 << state::MULTI_LINE) != 0 {
        Role::TextEditor
      } else {
        Role::TextField
      }
    }
    "combo box" => Role::ComboBox,
    "list box" => Role::ListBox,
    "list item" | "option" => Role::Option,
    "label" | "static" => Role::Text,
    "heading" => Role::Heading(0),
    "image" => Role::Image,
    "link" => Role::Link,
    "progress bar" => Role::ProgressBar,
    "landmark" | "section" | "region" => Role::Region,
    "group" | "panel" | "grouping" => Role::Group,
    "radio menu item" | "menu item" => Role::MenuItem,
    "menu" => Role::Menu,
    "dialog" => Role::Dialog,
    "status bar" => Role::Status,
    "notification" | "alert" => Role::Alert,
    "tool tip" => Role::Tooltip,
    "scroll pane" => Role::ScrollArea,
    _ => Role::Generic,
  }
}

/// Whether two roles are the same to a reader on GTK: a text box is one role there, and a
/// heading's level is not part of the role.
fn same_role(found: Role, widget: Role) -> bool {
  match (found, widget) {
    (Role::TextField | Role::TextEditor, Role::TextField | Role::TextEditor) => true,
    (Role::Heading(_), Role::Heading(_)) => true,
    (Role::ListBox, Role::ListBox) | (Role::Option, Role::Option) => true,
    (found, widget) => found == widget,
  }
}

/// Whether `query` finds `item`.
fn matches(query: &Query, item: &Item) -> bool {
  let role = role_of(&item.role, item.states);

  match query {
    Query::Role(wanted, name) => {
      let role_matches = match wanted {
        Role::Heading(0) => matches!(role, Role::Heading(_)),
        wanted => same_role(*wanted, role) && (*wanted != Role::TextEditor || role == *wanted),
      };

      role_matches && item.name == **name
    }
    Query::Text(text) => role == Role::Text && item.name == **text,
    Query::Focused => item.states & (1 << state::FOCUSED) != 0,
  }
}

/// A window a test opened, and the owner of everything built into it.
struct Shown {
  window: gtk::Window,
  title: String,
  owner: Option<Owner>,
}

/// The GTK driver: one window at a time, read through AT-SPI and acted on through XTest.
#[derive(Default)]
pub struct Gtk {
  shown: Option<Shown>,
}

impl Drop for Gtk {
  fn drop(&mut self) {
    self.close();
  }
}

impl Gtk {
  /// A driver with no window open.
  #[must_use]
  pub const fn new() -> Self {
    Self { shown: None }
  }

  /// Shows the node `root` (built under `owner`) in a new window 480 by 640 pixels, the
  /// previous one closed, and settles once the window is drawn and a reader can see it.
  ///
  /// # Panics
  ///
  /// Where the window never draws or never reaches the accessibility bus.
  pub fn show(&mut self, owner: Owner, root: NodeId) {
    self.close();

    let number = WINDOWS.with(|count| {
      count.set(count.get() + 1);
      count.get()
    });
    let title = format!("skene-test-{number}");
    let window = gtk::Window::new();
    let painted = Rc::new(Cell::new(false));

    window.set_title(Some(&title));
    window.set_default_size(480, 640);
    environment::attach_settings();
    environment::attach_window(&window);
    app::count_frames(&window);
    window.connect_realize({
      let painted = Rc::clone(&painted);

      move |window| {
        if let Some(clock) = window.frame_clock() {
          let painted = Rc::clone(&painted);

          clock.connect_after_paint(move |_| painted.set(true));
        }
      }
    });

    if let Some(child) = widget(root) {
      window.set_child(Some(&child));
    }

    window.present();

    let started = Instant::now();
    let context = glib::MainContext::default();

    while !painted.get() {
      context.iteration(false);
      std::thread::sleep(Duration::from_millis(1));

      assert!(
        started.elapsed() < Duration::from_secs(10),
        "the window never drew"
      );
    }

    self.shown = Some(Shown {
      window,
      title,
      owner: Some(owner),
    });

    settle();

    while self.frame().is_none() {
      assert!(
        started.elapsed() < Duration::from_secs(10),
        "the window never reached the accessibility bus"
      );

      wait(Duration::from_millis(10));
    }
  }

  /// The window open now.
  #[must_use]
  pub fn window(&self) -> Option<&gtk::Window> {
    self.shown.as_ref().map(|shown| &shown.window)
  }

  /// Closes the window and disposes what was built into it.
  pub fn close(&mut self) {
    if let Some(mut shown) = self.shown.take() {
      shown.window.destroy();

      if let Some(owner) = shown.owner.take() {
        owner.dispose();
      }

      app::drain();
    }
  }

  /// The window's object on the accessibility bus.
  fn frame(&self) -> Option<Object> {
    let shown = self.shown.as_ref()?;

    atspi().toplevel(&shown.title).ok().flatten()
  }

  /// What a reader finds in the window now, depth first.
  fn items(&self) -> Vec<Item> {
    let frame = self
      .frame()
      .unwrap_or_else(|| panic!("no window is open, or a reader cannot see it"));

    atspi()
      .tree(&frame)
      .unwrap_or_else(|error| panic!("AT-SPI: {error}"))
  }

  /// The widget the reader's `element` stands for: of the same role, showing the same text
  /// where it shows text, nearest the bounds the reader reads.
  ///
  /// # Panics
  ///
  /// Where no shown widget has the element's role.
  fn widget_for(&self, element: &Element) -> gtk::Widget {
    self.nearest_widget(element).unwrap_or_else(|| {
      panic!(
        "a reader finds {:?} {:?}, but no widget of its role is shown",
        element.role, element.name
      )
    })
  }

  /// The widget the reader's `element` stands for, where a shown widget has its role.
  fn nearest_widget(&self, element: &Element) -> Option<gtk::Widget> {
    let window = self.window()?;
    let extents = atspi()
      .extents(&element.object)
      .unwrap_or_else(|error| panic!("AT-SPI extents of {:?}: {error}", element.name));
    let center = (
      extents.0 as f32 + extents.2 as f32 / 2.,
      extents.1 as f32 + extents.3 as f32 / 2.,
    );
    let mut best: Option<(f32, gtk::Widget)> = None;

    walk(window.upcast_ref(), &mut |widget| {
      if !widget.is_mapped() || !same_role(element.role, crate::role_of(widget)) {
        return;
      }

      if let Some(label) = widget.downcast_ref::<gtk::Label>()
        && label.label() != element.name
      {
        return;
      }

      let Some(native) = widget.native() else {
        return;
      };
      let Some(bounds) = widget.compute_bounds(native.upcast_ref::<gtk::Widget>()) else {
        return;
      };
      let (dx, dy) = native.surface_transform();
      let distance = [(0., 0.), (dx as f32, dy as f32)]
        .into_iter()
        .map(|(dx, dy)| {
          (bounds.x() + dx + bounds.width() / 2. - center.0).abs()
            + (bounds.y() + dy + bounds.height() / 2. - center.1).abs()
        })
        .fold(f32::MAX, f32::min);

      if best.as_ref().is_none_or(|(nearest, _)| distance < *nearest) {
        best = Some((distance, widget.clone()));
      }
    });

    best.map(|(_, widget)| widget)
  }

  /// The border box GTK laid the widget `element` stands for out at, in the window's
  /// coordinates, where the element has a role of its own and a shown widget has it.
  fn laid_out(&self, element: &Element) -> Option<Rect> {
    if element.role == Role::Generic {
      return None;
    }

    let window = self.window()?;
    let bounds = self
      .nearest_widget(element)?
      .compute_bounds(window.upcast_ref::<gtk::Widget>())?;

    Some(Rect {
      x: bounds.x(),
      y: bounds.y(),
      width: bounds.width(),
      height: bounds.height(),
    })
  }

  /// The root coordinates of the point `(x, y)` of `widget`'s bounds, as fractions of its size.
  fn point(widget: &gtk::Widget, x: f32, y: f32) -> (i32, i32) {
    let native = widget.native().expect("the widget is shown");
    let bounds = widget
      .compute_bounds(native.upcast_ref::<gtk::Widget>())
      .unwrap_or_else(|| graphene::Rect::new(0., 0., 0., 0.));
    let (dx, dy) = native.surface_transform();
    let surface = native.surface().expect("the widget's window is shown");
    let xid = surface
      .downcast_ref::<gdk4_x11::X11Surface>()
      .expect("tests run on X11 (the display runner's Xvfb)")
      .xid();
    let (left, top) = input()
      .origin(u32::try_from(xid).unwrap_or_default())
      .unwrap_or_else(|error| panic!("X: {error}"));
    let scale = surface.scale();

    (
      left + ((dx + f64::from(bounds.x() + bounds.width() * x)) * scale) as i32,
      top + ((dy + f64::from(bounds.y() + bounds.height() * y)) * scale) as i32,
    )
  }

  /// Presses the key `name` with `modifiers` held, focus where it is.
  fn press(name: &str, modifiers: Modifiers) {
    let name = match name {
      "Enter" => "Return",
      "Esc" => "Escape",
      "Space" => "space",
      "Backspace" => "BackSpace",
      "PageUp" => "Page_Up",
      "PageDown" => "Page_Down",
      name => name,
    };
    let sym = match gdk::Key::from_name(name) {
      Some(key) => key.into_glib(),
      None => {
        let mut characters = name.chars();

        match (characters.next(), characters.next()) {
          (Some(character), None) => gdk::unicode_to_keyval(u32::from(character)),
          _ => panic!("no key is named {name:?}"),
        }
      }
    };
    let held: Vec<u32> = [
      (modifiers.control, keysym::CONTROL),
      (modifiers.shift, keysym::SHIFT),
      (modifiers.alt, keysym::ALT),
      (modifiers.meta, keysym::SUPER),
    ]
    .into_iter()
    .filter_map(|(held, sym)| held.then_some(sym))
    .collect();

    input()
      .key(sym, &held)
      .unwrap_or_else(|error| panic!("X: {error}"));
  }

  /// Types `text`, a key per character, focus where it is.
  fn type_text(text: &str) {
    for character in text.chars() {
      let sym = match character {
        '\n' => gdk::Key::Return.into_glib(),
        '\t' => gdk::Key::Tab.into_glib(),
        character => gdk::unicode_to_keyval(u32::from(character)),
      };

      if !input().types(sym) {
        panic!("no key types {character:?} on the test keyboard: compose it (`Action::Compose`)");
      }

      input()
        .key(sym, &[])
        .unwrap_or_else(|error| panic!("X: {error}"));
    }
  }

  /// Starts composing `character` through GTK's own input method (`Ctrl+Shift+U` and its code
  /// point), leaving the preedit open.
  fn compose(character: char) {
    Self::press(
      "u",
      Modifiers {
        control: true,
        shift: true,
        ..Modifiers::default()
      },
    );
    Self::type_text(&format!("{:x}", u32::from(character)));
  }
}

/// Visits `widget` and its descendants, depth first.
fn walk(widget: &gtk::Widget, visit: &mut impl FnMut(&gtk::Widget)) {
  visit(widget);

  let mut child = widget.first_child();

  while let Some(current) = child {
    walk(&current, visit);
    child = current.next_sibling();
  }
}

impl Driver for Gtk {
  type Found = Element;

  fn open(&mut self, page: &str) {
    let owner = Owner::new();
    let root = owner
      .run(|| skene_foundation::seam::testing::build(page))
      .unwrap_or_else(|| {
        panic!(
          "no page is registered as {page:?}; the pages are {:?}",
          skene_foundation::seam::testing::pages()
        )
      });

    self.show(owner, root);
  }

  fn settle(&mut self) {
    settle();
  }

  fn find_now(&mut self, query: &Query) -> Vec<Element> {
    self
      .items()
      .into_iter()
      .filter(|item| matches(query, item))
      .map(|item| Element {
        role: role_of(&item.role, item.states),
        platform_role: item.role,
        name: item.name,
        object: item.object,
      })
      .collect()
  }

  fn read(&mut self, found: &Element) -> AccessSnapshot {
    let atspi = atspi();
    let object = &found.object;
    let fail = |error: String| -> ! { panic!("AT-SPI: {error}") };
    let states = atspi.states(object).unwrap_or_else(|error| fail(error));
    let role_name = atspi.role(object).unwrap_or_else(|error| fail(error));
    let role = role_of(&role_name, states);
    let interfaces = atspi.interfaces(object).unwrap_or_default();
    let has = |interface: &str| interfaces.iter().any(|name| name == interface);
    let bit = |bit: u32| states & (1 << bit) != 0;
    let text = (has("org.a11y.atspi.Text") && role != Role::Text)
      .then(|| atspi.text(object).ok())
      .flatten();
    let selection = text.as_ref().and_then(|text| {
      let (caret, start, end) = atspi.caret_and_selection(object).ok()?;
      let cluster = |offset: i32| {
        Graphemes::cluster(text, Unit::CodePoint, usize::try_from(offset).unwrap_or(0))
      };
      let (start, end) = (cluster(start), cluster(end));
      let caret = cluster(caret);

      Some(if start != end && caret == start {
        TextSelection {
          start: end,
          end: start,
        }
      } else {
        TextSelection { start, end }
      })
    });
    let checkable = matches!(role, Role::Switch | Role::Checkbox | Role::Radio);
    let extents = atspi.extents(object).unwrap_or_default();
    let description = atspi.description(object).unwrap_or_default();

    AccessSnapshot {
      role,
      platform_role: role_name.into(),
      name: atspi
        .name(object)
        .unwrap_or_else(|error| fail(error))
        .into(),
      description: (!description.is_empty()).then(|| description.into()),
      checked: checkable.then(|| {
        if bit(state::INDETERMINATE) {
          Checked::Mixed
        } else if bit(state::CHECKED) {
          Checked::On
        } else {
          Checked::Off
        }
      }),
      enabled: bit(state::SENSITIVE),
      focused: bit(state::FOCUSED),
      expanded: (role == Role::ComboBox).then(|| bit(state::EXPANDED)),
      value: has("org.a11y.atspi.Value")
        .then(|| atspi.value(object).ok())
        .flatten(),
      text: text.map(Rc::from),
      selection,
      bounds: self.laid_out(found).unwrap_or(Rect {
        x: extents.0 as f32,
        y: extents.1 as f32,
        width: extents.2 as f32,
        height: extents.3 as f32,
      }),
    }
  }

  fn act(&mut self, found: &Element, action: &Action) {
    match action {
      Action::Click => {
        let (x, y) = Self::point(&self.widget_for(found), 0.5, 0.5);

        input()
          .click(x, y, 1)
          .unwrap_or_else(|error| panic!("X: {error}"));
      }
      Action::ContextClick => {
        let (x, y) = Self::point(&self.widget_for(found), 0.5, 0.5);

        input()
          .click(x, y, 3)
          .unwrap_or_else(|error| panic!("X: {error}"));
      }
      Action::Hover => {
        let (x, y) = Self::point(&self.widget_for(found), 0.5, 0.5);

        input()
          .move_to(x, y)
          .unwrap_or_else(|error| panic!("X: {error}"));
      }
      Action::Drag(right, down) => {
        let widget = self.widget_for(found);
        let (x, y) = Self::point(&widget, 0.5, 0.5);
        let scale = widget
          .native()
          .and_then(|native| native.surface())
          .map_or(1., |surface| surface.scale());
        let to = (
          x + (f64::from(*right) * scale) as i32,
          y + (f64::from(*down) * scale) as i32,
        );

        input()
          .drag((x, y), to)
          .unwrap_or_else(|error| panic!("X: {error}"));
      }
      Action::Press(..) | Action::Type(_) | Action::Compose(_) | Action::Commit => {
        return self.act_at_focus(action);
      }
      Action::Scroll(right, down) => {
        let (x, y) = Self::point(&self.widget_for(found), 0.5, 0.5);
        let steps = [(*down, 5, 4), (*right, 7, 6)];

        for (count, forward, back) in steps {
          let button = if count >= 0 { forward } else { back };

          for _ in 0..count.unsigned_abs() {
            input()
              .click(x, y, button)
              .unwrap_or_else(|error| panic!("X: {error}"));
          }
        }
      }
    }

    settle();
  }

  fn probe(&mut self, name: &str) -> Option<Rc<str>> {
    skene_foundation::seam::testing::probe(name).map(Rc::from)
  }

  fn set_environment(&mut self, request: EnvironmentRequest) {
    match request {
      EnvironmentRequest::Scheme(scheme) => environment::set_scheme(scheme),
      EnvironmentRequest::Size(width, height) => {
        if let Some(window) = self.window() {
          window.set_default_size(width as i32, height as i32);
        }
      }
      EnvironmentRequest::ReducedMotion(reduced) => {
        if let Some(settings) = gtk::Settings::default() {
          settings.set_gtk_enable_animations(!reduced);
        }
      }
      EnvironmentRequest::ReaderScheme(scheme) => {
        if let Some(settings) = gtk::Settings::default() {
          settings.set_gtk_application_prefer_dark_theme(scheme == Scheme::Dark);
        }
      }
      EnvironmentRequest::HighContrast(high) => {
        if let Some(settings) = gtk::Settings::default() {
          let theme = INITIAL.with(|initial| {
            initial
              .borrow()
              .as_ref()
              .and_then(|initial| initial.theme.clone())
          });

          settings.set_gtk_theme_name(if high {
            Some("Default-hc")
          } else {
            theme.as_deref()
          });
        }
      }
    }

    settle();
  }

  fn screenshot(&mut self) -> Screenshot {
    settle();

    let window = self.window().expect("a window is open");
    let texture = app::render(window.upcast_ref());
    let mut downloader = gdk::TextureDownloader::new(&texture);

    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);

    let (bytes, stride) = downloader.download_bytes();
    let (width, height) = (texture.width() as usize, texture.height() as usize);
    let mut rgba = Vec::with_capacity(width * height * 4);

    for row in bytes.chunks(stride).take(height) {
      rgba.extend_from_slice(&row[..width * 4]);
    }

    Screenshot {
      width: width as u32,
      height: height as u32,
      rgba,
    }
  }

  fn outline(&mut self) -> String {
    self
      .items()
      .iter()
      .map(|item| {
        let states = state::names(item.states);

        format!(
          "{}{} {:?}{}",
          "  ".repeat(item.depth),
          item.role,
          item.name,
          if states.is_empty() {
            String::new()
          } else {
            format!(" [{}]", states.join(", "))
          }
        )
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn audit(&mut self) -> Vec<Issue> {
    let items = self.items();
    let mut issues = Vec::new();

    for item in items {
      let role = role_of(&item.role, item.states);
      let control = matches!(
        role,
        Role::Button
          | Role::Switch
          | Role::Checkbox
          | Role::Radio
          | Role::Slider
          | Role::SpinButton
          | Role::TextField
          | Role::TextEditor
          | Role::ComboBox
          | Role::Link
      );
      let problem = if control && item.name.trim().is_empty() {
        Some("a control without a name")
      } else if role == Role::Image && item.name.trim().is_empty() {
        Some("a picture without a description")
      } else if item.states & (1 << state::SENSITIVE) == 0
        && item.states & (1 << state::FOCUSED) != 0
      {
        Some("a disabled control that has focus")
      } else {
        None
      };

      if let Some(problem) = problem {
        let element = Element {
          role,
          platform_role: item.role.clone(),
          name: item.name.clone(),
          object: item.object.clone(),
        };

        issues.push(Issue {
          snapshot: self.read(&element),
          problem: problem.into(),
        });
      }
    }

    issues
  }

  fn find(&mut self, query: &Query, timeout: Duration) -> Vec<Element> {
    let started = Instant::now();

    loop {
      let found = self.find_now(query);

      if !found.is_empty() || started.elapsed() >= timeout {
        return found;
      }

      wait(Duration::from_millis(10));
      settle();
    }
  }
}

impl Gtk {
  /// What a reader finds for `query`, looking again for up to a second.
  pub fn find_soon(&mut self, query: &Query) -> Vec<Element> {
    self.find(query, FIND_TIMEOUT)
  }

  /// Focuses the widget `found` stands for, as moving focus to it does, and settles.
  pub fn focus(&mut self, found: &Element) {
    self.widget_for(found).grab_focus();
    settle();
  }

  /// Acts where focus is: a key pressed, text typed, a composition started or committed; then
  /// settles.
  ///
  /// # Panics
  ///
  /// Where the action goes to a point (a click, a hover, a scroll): those act on an element.
  pub fn act_at_focus(&mut self, action: &Action) {
    match action {
      Action::Press(name, modifiers) => Self::press(name, *modifiers),
      Action::Type(text) => Self::type_text(text),
      Action::Compose(character) => Self::compose(*character),
      Action::Commit => Self::press("space", Modifiers::default()),
      Action::Click
      | Action::ContextClick
      | Action::Hover
      | Action::Drag(..)
      | Action::Scroll(..) => {
        panic!("{action:?} acts on an element, not at the focus")
      }
    }

    settle();
  }

  /// The app's scheme choice, through the platform's own path.
  pub fn set_scheme(&mut self, scheme: Option<Scheme>) {
    self.set_environment(EnvironmentRequest::Scheme(scheme));
  }
}
