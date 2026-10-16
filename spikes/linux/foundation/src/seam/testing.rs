//! Testing on the real platform: the pages an app registers, the probes they publish, the
//! gate every realization passes, and the driver each realization implements.
//!
//! A test never runs in the app's scene directly: it opens a page by name and reads what a
//! reader reads, so the same test drives an app in its own process (Linux, Android
//! instrumentation, Windows) or in another (the web over WebDriver, Apple over XCUITest).

use std::{
  cell::RefCell,
  rc::Rc,
  time::{Duration, Instant},
};

use crate::{
  NodeId, Scheme,
  layout::Rect,
  seam::{
    Realization,
    access::{Checked, Role},
    content::{Choice, Content, Edit, PropValue, Range, Run, TextSelection, writing},
    node::{NodeKind, NodeSpec},
    picture::{
      Paint, PaintOp, Picture,
      kurbo::{self, Shape as _},
    },
    report::Modifiers,
    style::ScopeChain,
    tree,
  },
};

type Page = (&'static str, Rc<dyn Fn() -> NodeId>);
type Probe = (&'static str, Rc<dyn Fn() -> String>);

thread_local! {
  static PAGES: RefCell<Vec<Page>> = const { RefCell::new(Vec::new()) };
  static PROBES: RefCell<Vec<Probe>> = const { RefCell::new(Vec::new()) };
}

/// Registers a page of the app under `name`: a view a test opens by name, `build` building
/// its root node. A later registration under the same name replaces it.
pub fn register(name: &'static str, build: impl Fn() -> NodeId + 'static) {
  PAGES.with(|pages| {
    let mut pages = pages.borrow_mut();

    pages.retain(|(page, _)| *page != name);
    pages.push((name, Rc::new(build)));
  });
}

/// The pages registered, in order.
#[must_use]
pub fn pages() -> Vec<&'static str> {
  PAGES.with(|pages| pages.borrow().iter().map(|(name, _)| *name).collect())
}

/// Builds the page `name` in the app, its previous page's probes dropped: what a driver
/// asks the app for, in process or through the platform's channel.
pub fn build(name: &str) -> Option<NodeId> {
  let build = PAGES.with(|pages| {
    pages
      .borrow()
      .iter()
      .find(|(page, _)| *page == name)
      .map(|(_, build)| Rc::clone(build))
  })?;

  PROBES.with(|probes| probes.borrow_mut().clear());

  Some(build())
}

/// Publishes a probe of the page being built: a value a test reads by name (what a signal
/// the page binds holds), the only state a test reads that a reader does not.
pub fn publish(name: &'static str, probe: impl Fn() -> String + 'static) {
  PROBES.with(|probes| probes.borrow_mut().push((name, Rc::new(probe))));
}

/// The probe `name` of the open page now.
#[must_use]
pub fn probe(name: &str) -> Option<String> {
  let probe = PROBES.with(|probes| {
    probes
      .borrow()
      .iter()
      .find(|(probe, _)| *probe == name)
      .map(|(_, probe)| Rc::clone(probe))
  })?;

  Some(probe())
}

/// How a content fared in [`contents_gate`].
#[derive(Clone, PartialEq, Debug)]
pub enum GateOutcome {
  /// Written, and read back from the toolkit equal.
  Shown,
  /// Written; the content is shown only ([`Content::readable`]), so a reader's view of it is
  /// checked through the accessibility layer instead.
  Written,
  /// Read back as something else, or not at all: the realization drops it.
  Dropped(Option<PropValue>),
}

/// One row of [`contents_gate`].
#[derive(Clone, PartialEq, Debug)]
pub struct GateRow {
  /// The node's kind.
  pub kind: NodeKind,
  /// The content.
  pub content: Content,
  /// What happened.
  pub outcome: GateOutcome,
}

/// A sample of each content, as the gate writes it.
#[must_use]
pub fn sample(content: Content) -> Option<PropValue> {
  let text = |text: &str| PropValue::Text(text.into());

  Some(match content {
    Content::Text => text("Skene \u{2713}"),
    Content::Runs => PropValue::Runs(Rc::from(vec![
      Run {
        text: "Read the ".into(),
        props: Rc::from([]),
        destination: None,
      },
      Run {
        text: "guide".into(),
        props: Rc::from([]),
        destination: Some("https://example.org/guide".into()),
      },
    ])),
    Content::Caption => text("Caption"),
    Content::On | Content::Mixed | Content::ReadOnly | Content::Secure | Content::Prominent => {
      PropValue::Bool(true)
    }
    Content::Enabled => PropValue::Bool(false),
    Content::Range => PropValue::Range(Range {
      value: 150.,
      minimum: 0.,
      maximum: 200.,
      step: 1.,
    }),
    Content::Choice => PropValue::Choice(Choice {
      options: Rc::from(vec![Rc::from("a"), Rc::from("b"), Rc::from("c")]),
      chosen: Some(2),
    }),
    Content::Edit => PropValue::Edit(Edit {
      text: "he\u{301}llo".into(),
      selection: Some(TextSelection { start: 1, end: 3 }),
    }),
    Content::Placeholder => text("Ada"),
    Content::Keyboard => text("email"),
    Content::Source => text("/skene/sample.png"),
    Content::Destination => text("https://example.org/"),
    Content::Picture => {
      let mut picture = Picture::new();

      picture.push(PaintOp::Fill {
        path: kurbo::Rect::new(0., 0., 2., 1.).into_path(0.1),
        paint: Paint::Color(crate::Color::rgb(0xff0000)),
      });

      PropValue::Picture(Rc::new(picture))
    }
    Content::Door(_) => return None,
  })
}

/// The gate every realization passes on its real platform: every content every kind takes
/// ([`NodeKind::contents`]) is written and, where readable, read back from the toolkit. A
/// realization is a dispatcher over `(kind, content)` that the compiler cannot prove total;
/// this proves it. Run it with the toolkit initialized, under an owner.
#[must_use]
pub fn contents_gate<R: Realization>() -> Vec<GateRow> {
  let mut rows = Vec::new();

  for kind in NodeKind::ALL {
    for content in kind.contents() {
      let Some(value) = sample(*content) else {
        continue;
      };
      let id = tree::create::<R>(NodeSpec::new(kind), &ScopeChain::root());
      let Some(native) = tree::native::<R>(id) else {
        continue;
      };

      writing(|| R::write(&native, *content, &value));

      let outcome = match R::read(&native, *content) {
        _ if !content.readable() => GateOutcome::Written,
        Some(read) if read == value => GateOutcome::Shown,
        read => GateOutcome::Dropped(read),
      };

      tree::remove::<R>(id);

      rows.push(GateRow {
        kind,
        content: *content,
        outcome,
      });
    }
  }

  rows
}

/// What a test looks for.
#[derive(Clone, PartialEq, Debug)]
pub enum Query {
  /// A node of a role whose accessible name is this.
  Role(Role, Rc<str>),
  /// A text node showing this.
  Text(Rc<str>),
  /// The node that has keyboard focus.
  Focused,
}

/// What a reader finds about a node, read from the platform's accessibility layer.
#[derive(Clone, PartialEq, Debug)]
pub struct AccessSnapshot {
  /// The role, mapped back to Skene's.
  pub role: Role,
  /// The platform's own name for the role.
  pub platform_role: Rc<str>,
  /// The accessible name.
  pub name: Rc<str>,
  /// The description.
  pub description: Option<Rc<str>>,
  /// The check.
  pub checked: Option<Checked>,
  /// Whether it is enabled.
  pub enabled: bool,
  /// Whether it has focus.
  pub focused: bool,
  /// Whether it is expanded.
  pub expanded: Option<bool>,
  /// A range value.
  pub value: Option<f64>,
  /// A text entry's text.
  pub text: Option<Rc<str>>,
  /// A text entry's selection, in grapheme clusters.
  pub selection: Option<TextSelection>,
  /// Its bounds in the window, in pixels: the border box the platform laid it out at.
  pub bounds: Rect,
}

/// What a test does, as a user does it: by real input at the node's bounds, or at the focus.
#[derive(Clone, PartialEq, Debug)]
pub enum Action {
  /// A primary click at the center.
  Click,
  /// A secondary click at the center.
  ContextClick,
  /// A primary press at the center, dragged by this offset in pixels (right and down
  /// positive) and released there.
  Drag(f32, f32),
  /// The pointer moved over the center.
  Hover,
  /// A key, by the platform's name for it (`Tab`, `Return`, `a`), with modifiers held, focus
  /// where it is.
  Press(Rc<str>, Modifiers),
  /// Text typed, a key per character, focus where it is.
  Type(Rc<str>),
  /// An input method composing this character, focus where it is, the composition left open
  /// (its preedit showing).
  Compose(char),
  /// The open composition committed.
  Commit,
  /// Wheel steps at the center: right and down positive.
  Scroll(i32, i32),
}

/// The environment a test asks the platform for, through its real path.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum EnvironmentRequest {
  /// The app's scheme, or the reader's (`None`).
  Scheme(Option<Scheme>),
  /// The window's size, in pixels.
  Size(f32, f32),
  /// Animations off (reduced motion) or on.
  ReducedMotion(bool),
  /// The reader's own scheme, as the platform's settings report it.
  ReaderScheme(Scheme),
  /// More contrast, as the reader's settings ask for it, or not.
  HighContrast(bool),
}

/// An image of a window or a node.
#[derive(Clone, PartialEq, Debug)]
pub struct Screenshot {
  /// The width in device pixels.
  pub width: u32,
  /// The height in device pixels.
  pub height: u32,
  /// The pixels, RGBA, row by row.
  pub rgba: Vec<u8>,
}

/// A problem a reader would meet, found by an audit of the accessibility layer.
#[derive(Clone, PartialEq, Debug)]
pub struct Issue {
  /// The node.
  pub snapshot: AccessSnapshot,
  /// What is wrong.
  pub problem: Rc<str>,
}

/// How each realization reads and acts for `skene_gui::testing`: pages are opened by name,
/// reads go through the platform's accessibility layer and the pages' probes, actions
/// through real input injection, waits through the platform's own clock.
pub trait Driver {
  /// A node found, as the driver holds it (an AT-SPI object matched to its bounds, a
  /// WebDriver element, an `XCUIElement`), equal to itself found again while its node lives.
  type Found: Clone + PartialEq;

  /// Opens the app's page `name` ([`register`]) in a fresh window, settled.
  fn open(&mut self, page: &str);

  /// Runs the platform until it is idle: queued input delivered, no frame drawn and no
  /// report made for one quiet window longer than a frame.
  fn settle(&mut self);

  /// What a reader finds for `query` now.
  fn find_now(&mut self, query: &Query) -> Vec<Self::Found>;

  /// What a reader reads of `found`.
  fn read(&mut self, found: &Self::Found) -> AccessSnapshot;

  /// Acts on `found` as a user, then settles.
  fn act(&mut self, found: &Self::Found, action: &Action);

  /// The open page's probe `name` ([`publish`]).
  fn probe(&mut self, name: &str) -> Option<Rc<str>>;

  /// Asks the platform for an environment, then settles.
  fn set_environment(&mut self, request: EnvironmentRequest);

  /// The window drawn.
  fn screenshot(&mut self) -> Screenshot;

  /// What a reader finds in the window now, one node a line, indented by depth: what a
  /// failure shows instead of what it looked for.
  fn outline(&mut self) -> String;

  /// What a reader would meet that it should not (a button-like node without an action, a
  /// control without a name, a disabled control that takes focus).
  fn audit(&mut self) -> Vec<Issue>;

  /// What a reader finds for `query`, looking again until `timeout` passes: a report the
  /// toolkit delivers late (a button's keyboard activation on a timer) is waited for here.
  fn find(&mut self, query: &Query, timeout: Duration) -> Vec<Self::Found> {
    let start = Instant::now();

    loop {
      let found = self.find_now(query);

      if !found.is_empty() || start.elapsed() >= timeout {
        return found;
      }

      self.settle();
    }
  }
}
