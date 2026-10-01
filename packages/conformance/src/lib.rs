//! # Conformance
//!
//! One suite of behaviors every platform shows alike, driven through a [`Driver`]: a
//! platform implements the trait over its own test seams, and [`run`] builds each
//! scenario's view on it, acts on it as a user does and checks what the user would
//! find. Every platform passing the suite behaves alike wherever the suite looks.
//!
//! A scenario finds what it acts on as a reader does, by role and accessible name, and
//! checks the signals its view is bound to, the node holding focus and what a reader can
//! find, never a platform's records.

mod scenarios;

use std::{
  any::Any,
  panic::{AssertUnwindSafe, catch_unwind},
  time::Duration,
};

use skene_foundation::{AnyView, KeyEvent, NodeId, Platform, Role, Size};

/// A platform driven as a user drives it, for the suite [`run`] runs.
pub trait Driver {
  /// The platform driven.
  type Platform: Platform;

  /// Builds the view `build` returns as the whole page in place of whatever the driver
  /// mounted before.
  fn mount(&mut self, build: impl FnOnce() -> AnyView<Self::Platform> + 'static);

  /// The first node in tree order whose role is `role` and whose accessible name is
  /// `name`, outside what is hidden.
  fn find(&mut self, role: Role, name: &str) -> Option<NodeId>;

  /// The first run in tree order whose own text is `text`, outside what is hidden.
  fn find_text(&mut self, text: &str) -> Option<NodeId>;

  /// Moves the pointer over `node`, as a user does.
  fn hover(&mut self, node: NodeId);

  /// Clicks `node` with the primary button, as a user does.
  fn click(&mut self, node: NodeId);

  /// Presses and releases the key `key` describes where focus is, as a user does.
  fn press(&mut self, key: &KeyEvent);

  /// Clicks the text entry `node` and types `text` into it, as a user does.
  fn type_text(&mut self, node: NodeId, text: &str);

  /// Moves focus to `node`, as a script does.
  fn focus(&mut self, node: NodeId);

  /// Resizes the viewport to `size`, in the platform's unit.
  fn resize(&mut self, size: Size<<Self::Platform as Platform>::Scalar>);

  /// Advances the platform's clock by `duration`, running what comes due.
  fn advance(&mut self, duration: Duration);

  /// The node holding focus.
  fn focused(&mut self) -> Option<NodeId>;

  /// The accessibility tree as indented text, one node a line
  /// ([`Scene::access_outline`](skene_foundation::Scene::access_outline)).
  fn access_outline(&mut self) -> String;
}

/// One behavior of the suite on the driver `D`: what it shows, and the function acting it
/// out, which panics where the platform behaves otherwise.
pub struct Scenario<D> {
  /// What the scenario shows, as a sentence.
  pub name: &'static str,
  /// Acts the scenario out on a freshly made driver.
  pub act: fn(&mut D),
}

impl<D> std::fmt::Debug for Scenario<D> {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter
      .debug_struct("Scenario")
      .field("name", &self.name)
      .finish_non_exhaustive()
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
      let outcome = catch_unwind(AssertUnwindSafe(|| (scenario.act)(&mut driver)));

      drop(driver);

      outcome
        .err()
        .map(|panic| format!("{}: {}", scenario.name, message(&*panic)))
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
