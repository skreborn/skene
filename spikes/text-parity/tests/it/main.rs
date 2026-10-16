//! The harness end to end on the host: Parley's records of a few variants, valid, and agreeing
//! with themselves in every measure.

use std::path::PathBuf;

use text_parity::{
  compare::{self, Options},
  corpus::Corpus,
  record::{Range, RecordSet},
  run::{self, ParleyRun},
};

fn scratch(name: &str) -> PathBuf {
  let dir = std::env::temp_dir().join(format!("text-parity-{name}-{}", std::process::id()));

  let _ = std::fs::remove_dir_all(&dir);

  dir
}

#[test]
fn a_set_agrees_with_itself() {
  let out = scratch("self");
  let set = run::parley(&ParleyRun {
    out: out.clone(),
    scales: vec![1., 2.],
    repeats: 1,
    labels: 10,
    only: Some("latin-kerning".to_owned()),
    glyphs: true,
    ..ParleyRun::default()
  })
  .expect("a run");
  let corpus = Corpus::builtin();
  let problems = set.validate(&corpus, &out);

  assert_eq!(set.records.len(), 2, "both latin-kerning variants");
  assert!(
    problems.iter().all(|problem| problem.ends_with("missing")),
    "only the variants left out are missing: {problems:?}"
  );
  assert!(
    set
      .records
      .iter()
      .all(|record| !record.glyph_runs.is_empty()),
    "glyph runs recorded"
  );

  let comparison = compare::compare(&corpus, (&set, &out), (&set, &out), &Options::default());

  assert_eq!(comparison.totals.variants, 2);
  assert_eq!(comparison.totals.breaks_identical, 2);
  assert_eq!(comparison.totals.same_family, comparison.totals.characters);
  assert!(
    (comparison.totals.mean_ssim - 1.).abs() < 1e-9,
    "ssim {}",
    comparison.totals.mean_ssim
  );
  assert_eq!(comparison.totals.mean_abs_diff, 0.);
  assert!(comparison.markdown(&corpus).contains("### latin-kerning"));

  let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_native_record_in_utf16_is_normalized() {
  let corpus = Corpus::builtin();
  let json = r#"{
    "schema": 1, "corpus_version": 1,
    "engine": {"name": "native", "version": "1", "rasterizer": "none"},
    "platform": {"os": "test", "os_version": "1"},
    "records": [{
      "variant": "emoji-unicode-16/17/max",
      "lines": [{"utf16": [0, 25], "x": 0, "width": 10, "top": 0, "height": 20,
                 "baseline": 16, "ascent": 16, "descent": 4}],
      "width": 10, "height": 20,
      "runs": [{"utf16": [0, 2], "line": 0, "family": "Noto Color Emoji", "size": 17}]
    }]
  }"#;
  let mut set: RecordSet = serde_json::from_str(json).expect("a native record parses");

  set.normalize(&corpus);

  let text = &corpus.case("emoji-unicode-16").expect("case").text;

  assert_eq!(
    set.records[0].lines[0].range,
    Range {
      utf8: Some([0, text.len()]),
      utf16: Some([0, 25]),
    }
  );
  assert_eq!(
    set.records[0].runs[0].range.utf8,
    Some([0, 4]),
    "one astral emoji"
  );
}
