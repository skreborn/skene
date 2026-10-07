//! # Conformance
//!
//! One suite of behaviors every platform shows alike, driven through a [`Driver`]: a
//! platform implements the trait over its own test seams, and [`run`] opens each
//! scenario's page on it, acts on it as a user does and checks what the user would find.
//! Every platform passing the suite behaves alike wherever the suite looks.
//!
//! A scenario names the [`Page`] it opens, which a driver builds on its platform, in its
//! own process or in another. A scenario finds what it acts on as a reader does, by role
//! and accessible name, and checks the [`Probes`] its page publishes of the signals its
//! views are bound to, the node holding focus and what a reader finds of a node, never a
//! platform's records.

mod scenarios;

use std::{
  any::Any,
  fmt::Debug,
  panic::{AssertUnwindSafe, catch_unwind},
  time::Duration,
};

use accesskit::Toggled;
use skene_foundation::{AnyView, KeyEvent, NodeId, Platform, Role, Scalar, Size, TreeUpdate};

/// A platform driven as a user drives it, for the suite [`run`] runs.
pub trait Driver {
  /// A node of the open page, as the driver finds it.
  type Node: Clone + Debug + PartialEq;

  /// The unit the platform measures lengths in.
  type Scalar: Scalar;

  /// Opens the page of the suite named `page` ([`pages`]) as the whole of a fresh page,
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

/// A page of the suite: the view a scenario opens, built on the platform `P`.
pub struct Page<P: Platform> {
  /// The name a scenario opens it by.
  pub name: &'static str,
  /// Builds the view, publishing into the probes what a scenario checks of it.
  pub build: fn(&mut Probes) -> AnyView<P>,
}

impl<P: Platform> Debug for Page<P> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Page")
      .field("name", &self.name)
      .finish_non_exhaustive()
  }
}

/// Every page of the suite, on the platform `P`.
#[must_use]
pub fn pages<P: Platform>() -> Vec<Page<P>> {
  scenarios::pages()
}

/// The page of the suite named `name`, on the platform `P`.
#[must_use]
pub fn page<P: Platform>(name: &str) -> Option<Page<P>> {
  pages().into_iter().find(|page| page.name == name)
}

/// One behavior of the suite on the driver `D`: what it shows, the page it opens, and
/// the function acting it out, which panics where the platform behaves otherwise.
pub struct Scenario<D> {
  /// What the scenario shows, as a sentence.
  pub name: &'static str,
  /// The name of the page it opens ([`pages`]).
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
