//! `text-parity`: the corpus, Parley's records and images of it, their validation, and the
//! comparison of two record sets. `text-parity help` prints the usage.

use std::{
  collections::BTreeMap,
  path::{Path, PathBuf},
  process::ExitCode,
};

use text_parity::{
  compare::{self, Options as CompareOptions},
  corpus::Corpus,
  fonts::FontSource,
  record::{self, RecordSet},
  run::{self, ParleyRun},
};

const USAGE: &str = "\
usage:
  text-parity corpus [--expanded]
      print the corpus (--expanded: every variant spelled out)
  text-parity parley --out DIR [--scales 1,2,3] [--fonts FONTS.json] [--glyphs] [--only TEXT]
                     [--hint] [--quantize] [--plain-emoji] [--repeats 5] [--labels 1000]
                     [--no-images] [--os-version TEXT] [--device TEXT]
      lay out and draw every variant with Parley and vello_cpu: DIR/records.json, DIR/png/
  text-parity restamp PARLEY.json --images DIR --rasterizer TEXT
      option C: Parley's records over the images a platform rasterizer drew of their glyph
      runs into DIR/png/ by the same names: DIR/records.json
  text-parity validate RECORDS.json
      check a record set against the corpus and the image convention
  text-parity compare A.json B.json [--out REPORT.md] [--json COMPARE.json] [--diff DIR]
                      [--scale 2] [--threshold 64] [--tolerance 0.5]
      compare set A (the reference, a platform's engine) with set B (Parley)
";

/// The arguments after the subcommand: flags with values, switches, and the rest in order.
struct Arguments {
  values: BTreeMap<String, String>,
  switches: Vec<String>,
  positional: Vec<String>,
}

impl Arguments {
  fn parse(arguments: &[String], with_values: &[&str]) -> Self {
    let mut values = BTreeMap::new();
    let mut switches = Vec::new();
    let mut positional = Vec::new();
    let mut at = 0;

    while at < arguments.len() {
      let argument = &arguments[at];

      if let Some(name) = argument.strip_prefix("--") {
        if with_values.contains(&name) {
          values.insert(
            name.to_owned(),
            arguments.get(at + 1).cloned().unwrap_or_default(),
          );
          at += 1;
        }
        else {
          switches.push(name.to_owned());
        }
      }
      else {
        positional.push(argument.clone());
      }

      at += 1;
    }

    Self {
      values,
      switches,
      positional,
    }
  }

  fn value(&self, name: &str) -> Option<&str> {
    self.values.get(name).map(String::as_str)
  }

  fn switch(&self, name: &str) -> bool {
    self.switches.iter().any(|switch| switch == name)
  }

  fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
    self.value(name).map_or(Ok(default), |value| {
      value
        .parse()
        .map_err(|_| format!("--{name}: {value:?} is not a number"))
    })
  }
}

fn corpus_command(arguments: &Arguments) -> Result<(), String> {
  let corpus = Corpus::builtin();
  let json = if arguments.switch("expanded") {
    serde_json::to_string_pretty(&corpus.expanded())
  }
  else {
    serde_json::to_string_pretty(&corpus)
  }
  .map_err(|error| error.to_string())?;

  println!("{json}");

  Ok(())
}

fn parley_command(arguments: &Arguments) -> Result<(), String> {
  let defaults = ParleyRun::default();
  let run = ParleyRun {
    out: PathBuf::from(arguments.value("out").ok_or("--out DIR is required")?),
    scales: arguments
      .value("scales")
      .map_or(Ok(defaults.scales), |scales| {
        scales
          .split(',')
          .map(|scale| {
            scale
              .trim()
              .parse()
              .map_err(|_| format!("--scales: {scale:?}"))
          })
          .collect()
      })?,
    fonts: match arguments.value("fonts") {
      Some(path) => FontSource::read(Path::new(path))?,
      None => FontSource::system(),
    },
    hint: arguments.switch("hint"),
    quantize: arguments.switch("quantize"),
    plain_emoji: arguments.switch("plain-emoji"),
    repeats: arguments.number("repeats", defaults.repeats)?,
    labels: arguments.number("labels", defaults.labels)?,
    images: !arguments.switch("no-images"),
    glyphs: arguments.switch("glyphs"),
    only: arguments.value("only").map(str::to_owned),
    os_version: arguments.value("os-version").map(str::to_owned),
    device: arguments.value("device").map(str::to_owned),
  };
  let set = run::parley(&run)?;

  eprintln!(
    "{} -> {}",
    set.summary(),
    run.out.join("records.json").display()
  );

  Ok(())
}

fn restamp_command(arguments: &Arguments) -> Result<(), String> {
  let source = PathBuf::from(
    arguments
      .positional
      .first()
      .ok_or("PARLEY.json is required")?,
  );
  let dir = PathBuf::from(
    arguments
      .value("images")
      .ok_or("--images DIR is required")?,
  );
  let mut set = RecordSet::read(&source)?;
  let root = record::root_of(&source);

  set.engine.rasterizer = arguments
    .value("rasterizer")
    .ok_or("--rasterizer TEXT is required")?
    .to_owned();

  for record in &mut set.records {
    record
      .images
      .retain(|_, image| dir.join(image.as_str()).exists());

    for image in record.images.values() {
      if !root.join(image).exists() {
        return Err(format!("{image} is not in {}", root.display()));
      }
    }
  }

  set.scales.retain(|scale| {
    let key = record::scale_key(*scale);

    set
      .records
      .iter()
      .any(|record| record.images.contains_key(&key))
  });

  let path = dir.join("records.json");

  set.write(&path)?;
  eprintln!("{} -> {}", set.summary(), path.display());

  Ok(())
}

fn validate_command(arguments: &Arguments) -> Result<(), String> {
  let path = PathBuf::from(
    arguments
      .positional
      .first()
      .ok_or("RECORDS.json is required")?,
  );
  let set = RecordSet::read(&path)?;
  let problems = set.validate(&Corpus::builtin(), &record::root_of(&path));

  for problem in &problems {
    println!("{problem}");
  }

  if problems.is_empty() {
    println!("{}: valid", set.summary());

    Ok(())
  }
  else {
    Err(format!("{} problems", problems.len()))
  }
}

fn compare_command(arguments: &Arguments) -> Result<(), String> {
  let [a, b] = arguments.positional.as_slice()
  else {
    return Err("A.json and B.json are required".to_owned());
  };
  let (a, b) = (PathBuf::from(a), PathBuf::from(b));
  let corpus = Corpus::builtin();
  let options = CompareOptions {
    threshold: arguments.number("threshold", 64_u8)?,
    scale: arguments
      .value("scale")
      .map(str::parse)
      .transpose()
      .map_err(|_| "--scale".to_owned())?,
    diff_dir: arguments.value("diff").map(PathBuf::from),
    tolerance: arguments.number("tolerance", 0.5_f32)?,
  };
  let comparison = compare::compare(
    &corpus,
    (&RecordSet::read(&a)?, &record::root_of(&a)),
    (&RecordSet::read(&b)?, &record::root_of(&b)),
    &options,
  );
  let markdown = comparison.markdown(&corpus);

  match arguments.value("out") {
    Some(out) => std::fs::write(out, &markdown).map_err(|error| format!("{out}: {error}"))?,
    None => print!("{markdown}"),
  }

  if let Some(json) = arguments.value("json") {
    let text = serde_json::to_string_pretty(&comparison).map_err(|error| error.to_string())?;

    std::fs::write(json, text + "\n").map_err(|error| format!("{json}: {error}"))?;
  }

  let totals = &comparison.totals;

  eprintln!(
    "{} variants: breaks identical {}, same family {}/{} characters, mean SSIM {:.3}",
    totals.variants,
    totals.breaks_identical,
    totals.same_family,
    totals.characters,
    totals.mean_ssim
  );

  Ok(())
}

fn main() -> ExitCode {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let Some((command, rest)) = arguments.split_first()
  else {
    eprint!("{USAGE}");

    return ExitCode::FAILURE;
  };
  let parsed = Arguments::parse(
    rest,
    &[
      "out",
      "scales",
      "fonts",
      "only",
      "repeats",
      "labels",
      "os-version",
      "device",
      "json",
      "diff",
      "scale",
      "threshold",
      "tolerance",
      "images",
      "rasterizer",
    ],
  );
  let result = match command.as_str() {
    "corpus" => corpus_command(&parsed),
    "parley" => parley_command(&parsed),
    "validate" => validate_command(&parsed),
    "compare" => compare_command(&parsed),
    "restamp" => restamp_command(&parsed),
    "help" | "--help" | "-h" => {
      print!("{USAGE}");

      Ok(())
    }
    other => Err(format!("unknown command {other:?}\n{USAGE}")),
  };

  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("text-parity: {error}");

      ExitCode::FAILURE
    }
  }
}
