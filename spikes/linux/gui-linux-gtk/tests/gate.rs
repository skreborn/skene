//! The contents gate on real GTK: every content every node kind takes is written through the
//! realization and, where readable, read back equal from the toolkit.

use skene_foundation::seam::testing::{GateOutcome, contents_gate};
use skene_gui_linux::{Linux, app::run_test};

#[test]
fn every_content_every_kind_takes_is_shown_and_read_back() {
  run_test("contents gate", || {
    let rows = contents_gate::<Linux>();
    let dropped: Vec<String> = rows
      .iter()
      .filter(|row| matches!(row.outcome, GateOutcome::Dropped(_)))
      .map(|row| format!("{:?} {:?}: {:?}", row.kind, row.content, row.outcome))
      .collect();
    let shown = rows
      .iter()
      .filter(|row| row.outcome == GateOutcome::Shown)
      .count();

    println!(
      "contents gate: {} rows, {shown} shown and read back, dropped {dropped:?}",
      rows.len()
    );

    assert!(dropped.is_empty(), "{dropped:#?}");
    assert!(rows.len() >= 30, "{} rows", rows.len());
  });
}
