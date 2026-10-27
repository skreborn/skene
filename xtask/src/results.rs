//! What a test run reported, read from what it printed: a libtest summary, the status
//! nextest prints for each test, and the doctest sections of `cargo test --doc`; and the
//! least counts a run is held to.

use std::collections::{BTreeMap, BTreeSet};

use crate::Outcome;

/// What one binary reported in its libtest summary.
pub(crate) struct Results {
  /// The number of tests that passed.
  pub(crate) passed: usize,
  /// The number of tests that failed.
  pub(crate) failed: usize,
}

impl Results {
  /// What one binary's run reported in `output`, where it reported a summary: its last.
  pub(crate) fn read(output: &str) -> Option<Self> {
    output
      .lines()
      .rev()
      .find_map(Self::summary)
      .map(Self::parse)
  }

  /// The summary a line such as `test result: ok. 48 passed; 0 failed` reports, if it is
  /// one.
  fn summary(line: &str) -> Option<&str> {
    line.trim().strip_prefix("test result: ")
  }

  /// What `summary`, such as `ok. 48 passed; 0 failed`, reports.
  fn parse(summary: &str) -> Self {
    Self {
      passed: Self::count(summary, "passed"),
      failed: Self::count(summary, "failed"),
    }
  }

  /// The number before `label` in a test summary such as `ok. 48 passed; 0 failed`.
  fn count(summary: &str, label: &str) -> usize {
    summary
      .split(';')
      .find_map(|part| {
        let words: Vec<&str> = part.split_whitespace().collect();

        match words.as_slice() {
          [.., number, last] if *last == label => number.parse().ok(),
          _ => None,
        }
      })
      .unwrap_or(0)
  }
}

/// How many tests each binary or doctest set of a run passed, by its name.
struct Passed<'o>(BTreeMap<&'o str, usize>);

impl<'o> Passed<'o> {
  /// Each binary's passed tests in what `cargo nextest run` printed, by the binary's id: a
  /// test whose status is `PASS` or `LEAK` counts once, however often its status is
  /// printed.
  fn tests(output: &'o str) -> Self {
    let passed: BTreeSet<(&str, &str)> = output.lines().filter_map(Self::test).collect();
    let mut counts = BTreeMap::new();

    for (binary, _) in passed {
      *counts.entry(binary).or_insert(0) += 1;
    }

    Self(counts)
  }

  /// The binary id and the name of the test a nextest status line such as
  /// `TRY 2 PASS [   0.011s] (  1/550) skene-tui::it a_test` reports passed, if it is one.
  fn test(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let line = match line.strip_prefix("TRY ") {
      Some(attempt) => attempt.split_once(' ')?.1.trim_start(),
      None => line,
    };
    let timed = line
      .strip_prefix("PASS [")
      .or_else(|| line.strip_prefix("LEAK ["))?;
    let (_, rest) = timed.split_once(']')?;
    let rest = rest.trim_start();
    let rest = match rest.strip_prefix('(') {
      Some(counted) => counted.split_once(')')?.1.trim_start(),
      None => rest,
    };
    let (binary, test) = rest.split_once(' ')?;

    Some((binary, test.trim()))
  }

  /// Each doctest set's passed doctests in what `cargo test --doc` printed, by the crate
  /// name after its `Doc-tests`: the sum of every summary its section prints.
  fn doctests(output: &'o str) -> Self {
    let mut counts = BTreeMap::new();
    let mut set = None;

    for line in output.lines() {
      if let Some(name) = line.trim().strip_prefix("Doc-tests ") {
        let name = name.trim();

        counts.entry(name).or_insert(0);
        set = Some(name);
      }
      else if let (Some(set), Some(summary)) = (set, Results::summary(line)) {
        *counts.entry(set).or_insert(0) += Results::parse(summary).passed;
      }
    }

    Self(counts)
  }

  /// What is wrong with these counts against `least`, each problem's subject named by
  /// `subject` from the binary's or the set's name.
  fn shortfalls(
    &self,
    least: &[(&str, usize)],
    subject: impl Fn(&str) -> String,
    problems: &mut Vec<String>,
  ) {
    for (name, expected) in least {
      match self.0.get(name) {
        None => problems.push(format!("{}: reported no results", subject(name))),
        Some(passed) if passed < expected => problems.push(format!(
          "{} passed {passed} tests, fewer than {expected}",
          subject(name)
        )),
        Some(_) => {}
      }
    }

    for (name, passed) in &self.0 {
      if *passed > 0 && !least.iter().any(|(listed, _)| listed == name) {
        problems.push(format!(
          "{} passed {passed} tests and lists no least count",
          subject(name)
        ));
      }
    }
  }
}

/// The least number of tests each binary and doctest set of one `cargo nextest run` and
/// its `cargo test --doc` passes.
pub(crate) struct Minimums {
  /// Each nextest binary, by its binary id, with the number of tests it passes at least.
  pub(crate) binaries: &'static [(&'static str, usize)],
  /// Each doctest set, by the crate name rustdoc prints after `Doc-tests`, with the
  /// number of doctests it passes at least.
  pub(crate) doctests: &'static [(&'static str, usize)],
}

impl Minimums {
  /// Checks what the nextest run printed (`tests`) and what the doctest run printed
  /// (`doctests`) against the least counts, reporting every shortfall at once: a listed
  /// binary or set passing fewer, or reporting nothing, and one passing tests unlisted.
  pub(crate) fn check(&self, tests: &str, doctests: &str) -> Outcome {
    let mut problems = Vec::new();

    Passed::tests(tests).shortfalls(self.binaries, |name| format!("`{name}`"), &mut problems);
    Passed::doctests(doctests).shortfalls(
      self.doctests,
      |name| format!("the doctests of `{name}`"),
      &mut problems,
    );

    if problems.is_empty() {
      Ok(())
    }
    else {
      Err(problems.join("\n"))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{Minimums, Passed, Results};

  /// The counts of `passed`, by name.
  fn counts<'o>(passed: &Passed<'o>) -> Vec<(&'o str, usize)> {
    passed
      .0
      .iter()
      .map(|(name, count)| (*name, *count))
      .collect()
  }

  #[test]
  fn a_summary_reads_its_passed_and_failed_counts() {
    let results = Results::read(
      "running 3 tests\ntest result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured\n",
    )
    .expect("the output has a summary");

    assert_eq!((results.passed, results.failed), (2, 1));
    assert!(Results::read("running 0 tests\n").is_none());
  }

  #[test]
  fn a_pass_counts_with_and_without_its_counter() {
    let output = "    Starting 2 tests across 2 binaries\n        PASS [   0.011s] (  1/2) \
                  skene-tui::it a_label_wraps\n        PASS [   0.012s] skene-tui-terminal \
                  layout::tests::a_row_flows\n     Summary [   0.020s] 2 tests run: 2 passed, \
                  0 skipped\n";

    assert_eq!(
      counts(&Passed::tests(output)),
      [("skene-tui-terminal", 1), ("skene-tui::it", 1)]
    );
  }

  #[test]
  fn a_leak_and_a_retried_pass_count_and_a_failure_does_not() {
    let output = "        LEAK [   0.011s] (  1/4) skene-dev a_child_outlives_its_test\n  TRY 2 \
                  PASS [   0.300s] (  2/4) skene-dev a_flaky_test\n        FAIL [   0.010s] \
                  (  3/4) skene-dev a_broken_test\n        PASS [   0.010s] (  4/4) \
                  xtask::bin/xtask a_test\n";

    assert_eq!(
      counts(&Passed::tests(output)),
      [("skene-dev", 2), ("xtask::bin/xtask", 1)]
    );
  }

  #[test]
  fn a_final_status_repeating_a_pass_counts_once() {
    let output = "        PASS [   0.011s] (  1/1) skene-layout a_test\n────────────\n     \
                  Summary [   0.020s] 1 test run: 1 passed, 0 skipped\n        PASS [   \
                  0.011s] (  1/1) skene-layout a_test\n";

    assert_eq!(counts(&Passed::tests(output)), [("skene-layout", 1)]);
  }

  #[test]
  fn a_doctest_set_sums_every_summary_in_its_section() {
    let output = "   Doc-tests guide\n\nrunning 5 tests\ntest result: ok. 5 passed; 0 \
                  failed; 0 ignored\n\nrunning 1 test\ntest result: ok. 0 passed; 0 failed; 1 \
                  ignored\n   Doc-tests readme\n\nrunning 2 tests\ntest result: ok. 2 passed; \
                  0 failed; 0 ignored\ntest result: ok. 1 passed; 0 failed; 0 ignored\n   \
                  Doc-tests tui_counter\n\nrunning 0 tests\ntest result: ok. 0 passed; 0 \
                  failed; 0 ignored\n";

    assert_eq!(
      counts(&Passed::doctests(output)),
      [("guide", 5), ("readme", 3), ("tui_counter", 0)]
    );
  }

  #[test]
  fn every_shortfall_is_reported_at_once() {
    let minimums = Minimums {
      binaries: &[("skene-tui::it", 2), ("skene-tui-terminal", 1)],
      doctests: &[("skene_tui", 2)],
    };
    let tests = "        PASS [   0.011s] (  1/2) skene-tui::it a_test\n        PASS [   \
                 0.011s] (  2/2) tui-gallery::styled a_frame\n";
    let doctests = "   Doc-tests skene_tui\ntest result: ok. 1 passed; 0 failed\n";

    assert_eq!(
      minimums.check(tests, doctests),
      Err(
        [
          "`skene-tui::it` passed 1 tests, fewer than 2",
          "`skene-tui-terminal`: reported no results",
          "`tui-gallery::styled` passed 1 tests and lists no least count",
          "the doctests of `skene_tui` passed 1 tests, fewer than 2",
        ]
        .join("\n")
      )
    );
  }

  #[test]
  fn counts_at_their_least_pass() {
    let minimums = Minimums {
      binaries: &[("skene-tui::it", 1)],
      doctests: &[("skene_tui", 1)],
    };
    let tests = "        PASS [   0.011s] (  1/1) skene-tui::it a_test\n";
    let doctests = "   Doc-tests skene_tui\ntest result: ok. 1 passed; 0 failed\n   Doc-tests \
                    skene_tui_terminal\ntest result: ok. 0 passed; 0 failed\n";

    assert_eq!(minimums.check(tests, doctests), Ok(()));
  }
}
