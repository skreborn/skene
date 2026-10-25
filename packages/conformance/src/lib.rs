//! # Conformance
//!
//! One suite of behaviors every platform shows alike, driven through a [`Driver`]: a
//! platform implements the trait over its own test seams, and [`run`] opens each
//! scenario's page on it, acts on it as a user does and checks what the user would find.
//! Every platform passing the suite behaves alike wherever the suite looks.
//!
//! A scenario names the page it opens ([`PAGES`]), which each facade writes in its own
//! vocabulary and a driver builds on its platform, in its own process or in another. A
//! scenario finds what it acts on as a reader does, by role and accessible name, and
//! checks the [`Probes`] its page publishes of the signals its views are bound to, the
//! node holding focus and what a reader finds of a node, never a platform's records.

mod scenarios;

use std::{
  any::Any,
  fmt::Debug,
  panic::{AssertUnwindSafe, catch_unwind},
  time::Duration,
};

use accesskit::Toggled;
use skene_foundation::{KeyEvent, NodeId, Role, Scalar, Size, TreeUpdate};

/// Every page of the suite by name, in the order the suite first opens them: each page a
/// scenario opens, and `text`, which no scenario acts on and a platform that draws holds
/// against its golden.
///
/// A facade writes a page of each name, holding what its scenarios find:
///
/// - `button`: a button named Save, publishing its activations as `clicks`;
/// - `checkbox`: a checkbox named Remember me, publishing whether it is `checked`;
/// - `buttons`: the buttons One, Two and Three in a column;
/// - `disabled-button`: the buttons One, Two and Three in a column, Two disabled and publishing its
///   activations as `clicks`;
/// - `text-field`: a text field named Name, publishing its text as `value` and its selection as
///   `selection` (`start..end`), above the plain text Plain words;
/// - `read-only-field`: a read-only text field named Name holding fixed, publishing its text as
///   `value`;
/// - `radio-group`: a radio group named Size of Small, Medium and Large, nothing chosen at first,
///   publishing its choice as `chosen` (`s`, `m`, `l`, empty while nothing is chosen), above a
///   button named Next;
/// - `slider`: a slider named Volume from 0 to 100 at 50, publishing its `value`;
/// - `picker`: a picker named Letter of a, b and c, b chosen and its face showing the choice,
///   publishing its choice as `chosen`, above a button named Elsewhere;
/// - `tooltip`: a button named Save, described by the tooltip Saves your work;
/// - `context-menu`: a focusable group named Note holding the text A note, serving a context menu
///   of one entry, Rename, publishing whether it ran as `renamed`;
/// - `viewport`: the text Wide, hidden below the [`BREAKPOINT`] width;
/// - `visibility`: a button named Dismiss, whose activation hides it for [`RETURN`], above a button
///   named Elsewhere;
/// - `modal`: a button named Open, which opens a modal holding a button named Inside, publishing
///   whether the modal is `open`;
/// - `form`: a sign-in form, none of it bound: a heading Sign in, a text field named Name holding
///   ada, a checked checkbox named Remember me and a disabled button named Continue;
/// - `nested-boxes`: a group named Outer around a group named Inner, neither focusable;
/// - `focus-group`: a box around the buttons a and b, publishing focus entering it as `entered` and
///   leaving it as `left`, above a button named c;
/// - `text`: text keeping its blanks and newlines, a word too wide for the page broken where the
///   page's width ends.
pub const PAGES: [&str; 18] = [
  "button",
  "checkbox",
  "buttons",
  "disabled-button",
  "text-field",
  "read-only-field",
  "radio-group",
  "slider",
  "picker",
  "tooltip",
  "context-menu",
  "viewport",
  "visibility",
  "modal",
  "form",
  "nested-boxes",
  "focus-group",
  "text",
];

/// How long the view the `visibility` page hides stays hidden.
pub const RETURN: Duration = Duration::from_secs(1);

/// The viewport width at and above which the `viewport` page shows its text.
pub const BREAKPOINT: i32 = 700;

/// A platform driven as a user drives it, for the suite [`run`] runs.
pub trait Driver {
  /// A node of the open page, as the driver finds it.
  type Node: Clone + Debug + PartialEq;

  /// The unit the platform measures lengths in.
  type Scalar: Scalar;

  /// Opens the page of the suite named `page` ([`PAGES`]) as the whole of a fresh page,
  /// in place of whatever the driver opened before.
  fn open(&mut self, page: &str);

  /// What the open page publishes as its probe `name` ([`Probes::publish`]).
  fn probe(&mut self, name: &str) -> String;

  /// The first node in tree order whose role is `role` and whose accessible name is
  /// `name`, outside what is hidden.
  fn find(&mut self, role: Role, name: &str) -> Option<Self::Node>;

  /// The first run in tree order whose own text is `text`, outside what is hidden.
  fn find_text(&mut self, text: &str) -> Option<Self::Node>;

  /// What a reader finds of `node` beyond its role and name.
  fn read(&mut self, node: &Self::Node) -> Reading;

  /// Moves the pointer over `node`, as a user does.
  fn hover(&mut self, node: &Self::Node);

  /// Clicks `node` with the primary button, as a user does.
  fn click(&mut self, node: &Self::Node);

  /// Presses the primary button on `node` and holds it, as a user does.
  fn hold(&mut self, node: &Self::Node);

  /// Releases the button [`hold`](Self::hold) pressed, over `node`, as a user does.
  fn release(&mut self, node: &Self::Node);

  /// Presses and releases the key `key` describes where focus is, as a user does.
  fn press(&mut self, key: &KeyEvent);

  /// Clicks the text entry `node` and types `text` into it, as a user does.
  fn type_text(&mut self, node: &Self::Node, text: &str);

  /// Moves focus to `node`, as a script does.
  fn focus(&mut self, node: &Self::Node);

  /// Resizes the viewport to `size`, in the platform's unit.
  fn resize(&mut self, size: Size<Self::Scalar>);

  /// Lets `duration` pass on the platform's clock, running what comes due.
  fn advance(&mut self, duration: Duration);

  /// The node holding focus.
  fn focused(&mut self) -> Option<Self::Node>;

  /// Whether the node holding focus shows it, as `:focus-visible` matches.
  fn focus_shows(&mut self) -> bool;

  /// Whether `node` holds a press, as `:active` matches each node along a press's path.
  fn is_pressed(&mut self, node: &Self::Node) -> bool;
}

/// What a reader finds of a node beyond its role and name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Reading {
  /// The text it holds, where it is a text entry.
  pub value: Option<String>,
  /// Whether it is checked.
  pub checked: bool,
  /// Whether it is present but not operable.
  pub disabled: bool,
}

impl Reading {
  /// What the accessibility tree `tree` a scene reports
  /// ([`Scene::access_tree`](skene_foundation::Scene::access_tree)) says of `node`;
  /// the default where it holds no such node.
  pub fn of_tree(tree: &TreeUpdate, node: NodeId) -> Self {
    let id = node.access_id();
    let Some((_, access)) = tree.nodes.iter().find(|(held, _)| *held == id)
    else {
      return Self::default();
    };
    let entry = matches!(
      access.role(),
      Role::TextInput
        | Role::MultilineTextInput
        | Role::SearchInput
        | Role::EmailInput
        | Role::NumberInput
        | Role::PasswordInput
        | Role::PhoneNumberInput
        | Role::UrlInput
    );

    Self {
      value: access.value().filter(|_| entry).map(str::to_owned),
      checked: access.toggled() == Some(Toggled::True),
      disabled: access.is_disabled(),
    }
  }
}

/// The probes a page publishes: each a named reading of its state as text, which a
/// scenario checks through [`Driver::probe`].
#[derive(Default)]
#[must_use]
pub struct Probes(Vec<(&'static str, Probe)>);

/// What reads one probe.
type Probe = Box<dyn Fn() -> String>;

impl Probes {
  /// Publishes `read` as the probe `name`.
  pub fn publish(&mut self, name: &'static str, read: impl Fn() -> String + 'static) {
    self.0.push((name, Box::new(read)));
  }

  /// What the probe `name` reads now; `None` where the page publishes no such probe.
  #[must_use]
  pub fn read(&self, name: &str) -> Option<String> {
    self
      .0
      .iter()
      .find(|(published, _)| *published == name)
      .map(|(_, read)| read())
  }
}

impl Debug for Probes {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_list()
      .entries(self.0.iter().map(|(name, _)| name))
      .finish()
  }
}

/// One behavior of the suite on the driver `D`: what it shows, the page it opens, and
/// the function acting it out, which panics where the platform behaves otherwise.
pub struct Scenario<D> {
  /// What the scenario shows, as a sentence.
  pub name: &'static str,
  /// The name of the page it opens ([`PAGES`]).
  pub page: &'static str,
  /// Acts the scenario out on a driver its page is open on.
  pub act: fn(&mut D),
}

impl<D> Debug for Scenario<D> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Scenario")
      .field("name", &self.name)
      .field("page", &self.page)
      .finish_non_exhaustive()
  }
}

impl<D: Driver> Scenario<D> {
  /// Opens the scenario's page on `driver` and acts it out, returning what it reported
  /// where it failed.
  ///
  /// # Errors
  ///
  /// Returns the failure's message where the platform behaves otherwise.
  pub fn check(&self, driver: &mut D) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(|| {
      driver.open(self.page);
      (self.act)(driver);
    }))
    .map_err(|panic| message(&*panic))
  }
}

/// Every scenario of the suite, on the driver `D`.
#[must_use]
pub fn scenarios<D: Driver>() -> Vec<Scenario<D>> {
  scenarios::all()
}

/// Runs every scenario of the suite, each on a driver `driver` makes afresh, and panics
/// listing each scenario that failed with what it reported.
///
/// # Panics
///
/// Panics where any scenario fails.
pub fn run<D: Driver>(mut driver: impl FnMut() -> D) {
  let all = scenarios::<D>();
  let failed: Vec<String> = all
    .iter()
    .filter_map(|scenario| {
      let mut driver = driver();
      let outcome = scenario.check(&mut driver);

      drop(driver);

      outcome
        .err()
        .map(|failure| format!("{}: {failure}", scenario.name))
    })
    .collect();

  assert!(
    failed.is_empty(),
    "{} of {} scenarios failed:\n{}",
    failed.len(),
    all.len(),
    failed.join("\n")
  );
}

/// What a panic said.
fn message(panic: &(dyn Any + Send)) -> String {
  panic
    .downcast_ref::<String>()
    .cloned()
    .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
    .unwrap_or_else(|| "a panic without a message".to_owned())
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use skene_foundation::{KeyEvent, Role, Size};

  use crate::{Driver, PAGES, Reading, scenarios};

  /// A driver no value is, which names the suite's table and acts nothing out.
  #[derive(Clone, Copy, Debug, PartialEq)]
  enum Unbuilt {}

  impl Driver for Unbuilt {
    type Node = Self;
    type Scalar = i32;

    fn open(&mut self, _: &str) {
      match *self {}
    }

    fn probe(&mut self, _: &str) -> String {
      match *self {}
    }

    fn find(&mut self, _: Role, _: &str) -> Option<Self> {
      match *self {}
    }

    fn find_text(&mut self, _: &str) -> Option<Self> {
      match *self {}
    }

    fn read(&mut self, _: &Self) -> Reading {
      match *self {}
    }

    fn hover(&mut self, _: &Self) {
      match *self {}
    }

    fn click(&mut self, _: &Self) {
      match *self {}
    }

    fn hold(&mut self, _: &Self) {
      match *self {}
    }

    fn release(&mut self, _: &Self) {
      match *self {}
    }

    fn press(&mut self, _: &KeyEvent) {
      match *self {}
    }

    fn type_text(&mut self, _: &Self, _: &str) {
      match *self {}
    }

    fn focus(&mut self, _: &Self) {
      match *self {}
    }

    fn resize(&mut self, _: Size<i32>) {
      match *self {}
    }

    fn advance(&mut self, _: Duration) {
      match *self {}
    }

    fn focused(&mut self) -> Option<Self> {
      match *self {}
    }

    fn focus_shows(&mut self) -> bool {
      match *self {}
    }

    fn is_pressed(&mut self, _: &Self) -> bool {
      match *self {}
    }
  }

  #[test]
  fn the_pages_are_those_the_scenarios_open_then_the_text() {
    let mut opened: Vec<&str> = Vec::new();

    for scenario in scenarios::<Unbuilt>() {
      if !opened.contains(&scenario.page) {
        opened.push(scenario.page);
      }
    }

    opened.push("text");

    assert_eq!(opened, PAGES);
  }
}
