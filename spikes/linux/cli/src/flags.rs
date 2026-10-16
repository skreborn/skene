//! The flags cargo would hand rustc (or rustdoc) for a target, read from cargo's own sources in
//! cargo's precedence order, and the `skene_platform` cfg replaced among them.
//!
//! Cargo takes rustc's flags from the first of these that is set: `CARGO_ENCODED_RUSTFLAGS`;
//! `RUSTFLAGS`; `target.<triple>.rustflags` followed by every matching
//! `target.'cfg(..)'.rustflags` in the order of their keys; `build.rustflags`. A
//! configuration's values come from `$CARGO_HOME/config.toml` and each `.cargo/config.toml`
//! from the filesystem root down to the app's directory, merged before they are read: an
//! array joined across the files with the deeper file's last. Rustdoc's are the same with
//! `rustdocflags`. What the resolver cannot read exactly (a configuration `include`, a
//! `--config` given to cargo, the `CARGO_BUILD_*`/`CARGO_TARGET_*` variables, a `cfg(..)` table
//! it cannot evaluate, a flag naming `skene_platform` in a form it does not know) is a
//! refusal, never a guess.

use std::{
  collections::{BTreeMap, BTreeSet},
  env, fs,
  path::{Path, PathBuf},
  process::Command,
};

use crate::output;

/// Which tool's flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
  /// rustc's: `rustflags`.
  Rust,
  /// rustdoc's: `rustdocflags`.
  Rustdoc,
}

impl Kind {
  /// The configuration key.
  const fn key(self) -> &'static str {
    match self {
      Self::Rust => "rustflags",
      Self::Rustdoc => "rustdocflags",
    }
  }

  /// The environment variables, encoded then plain.
  const fn variables(self) -> (&'static str, &'static str) {
    match self {
      Self::Rust => ("CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS"),
      Self::Rustdoc => ("CARGO_ENCODED_RUSTDOCFLAGS", "RUSTDOCFLAGS"),
    }
  }
}

/// The flags cargo would use, and where they came from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Flags {
  /// The flags, in order.
  pub flags: Vec<String>,
  /// The source cargo would read them from.
  pub source: String,
}

/// The `--check-cfg` every platform's build carries.
pub const CHECK_CFG: &str =
  r#"cfg(skene_platform, values("web", "server", "ios", "android", "macos", "windows", "linux"))"#;

impl Flags {
  /// The flags cargo would use for `triple` from `dir`.
  pub fn resolve(dir: &Path, triple: &str, kind: Kind) -> Result<Self, String> {
    let (encoded, plain) = kind.variables();

    if let Ok(flags) = env::var(encoded) {
      return Ok(Self {
        flags: flags
          .split('\x1f')
          .filter(|flag| !flag.is_empty())
          .map(str::to_owned)
          .collect(),
        source: encoded.to_owned(),
      });
    }

    if let Ok(flags) = env::var(plain) {
      return Ok(Self {
        flags: flags.split_whitespace().map(str::to_owned).collect(),
        source: plain.to_owned(),
      });
    }

    let upper = triple.to_uppercase().replace(['-', '.'], "_");

    for variable in [
      format!("CARGO_BUILD_{}", kind.key().to_uppercase()),
      format!("CARGO_TARGET_{upper}_{}", kind.key().to_uppercase()),
    ] {
      if env::var_os(&variable).is_some() {
        return Err(format!(
          "cannot tell how cargo merges `{variable}` with the configuration; set `{plain}` instead"
        ));
      }
    }

    let files = Self::files(dir)?;
    let target_cfg = TargetCfg::of(triple)?;
    let mut triple_flags = Vec::new();
    let mut cfg_flags = BTreeMap::<&str, Vec<String>>::new();
    let mut target_sources = Vec::new();
    let mut build = Vec::new();
    let mut build_sources = Vec::new();

    for (path, table) in &files {
      if table.contains_key("include") {
        return Err(format!(
          "{}: a configuration `include` is not read; name the flags in the file itself",
          path.display()
        ));
      }

      if let Some(targets) = table.get("target").and_then(toml::Value::as_table) {
        for (key, section) in targets {
          let applies = match key.strip_prefix("cfg(") {
            Some(_) => target_cfg.holds(key, path)?,
            None => key == triple,
          };

          if applies && let Some(value) = section.get(kind.key()) {
            let flags = Self::list(value, path)?;

            if key == triple {
              triple_flags.extend(flags);
            } else {
              cfg_flags.entry(key).or_default().extend(flags);
            }

            target_sources.push(format!("{} [target.{key}]", path.display()));
          }
        }
      }

      if let Some(value) = table.get("build").and_then(|build| build.get(kind.key())) {
        build.extend(Self::list(value, path)?);
        build_sources.push(format!("{} [build]", path.display()));
      }
    }

    let target: Vec<String> = triple_flags
      .into_iter()
      .chain(cfg_flags.into_values().flatten())
      .collect();

    Ok(if !target.is_empty() {
      Self {
        flags: target,
        source: target_sources.join(", "),
      }
    } else if !build.is_empty() {
      Self {
        flags: build,
        source: build_sources.join(", "),
      }
    } else {
      Self {
        flags: Vec::new(),
        source: "nowhere (no flags)".to_owned(),
      }
    })
  }

  /// Whether cargo would run `triple`'s binaries through a runner the environment or a
  /// configuration file names (`CARGO_TARGET_<TRIPLE>_RUNNER`, `target.<triple>.runner`, a
  /// matching `target.'cfg(..)'.runner`).
  pub fn names_runner(dir: &Path, triple: &str) -> Result<bool, String> {
    let upper = triple.to_uppercase().replace(['-', '.'], "_");

    if env::var_os(format!("CARGO_TARGET_{upper}_RUNNER")).is_some() {
      return Ok(true);
    }

    let target_cfg = TargetCfg::of(triple)?;

    for (path, table) in Self::files(dir)? {
      let Some(targets) = table.get("target").and_then(toml::Value::as_table) else {
        continue;
      };

      for (key, section) in targets {
        let applies = match key.strip_prefix("cfg(") {
          Some(_) => target_cfg.holds(key, &path)?,
          None => key == triple,
        };

        if applies && section.get("runner").is_some() {
          return Ok(true);
        }
      }
    }

    Ok(false)
  }

  /// Refuses a `--config` among `arguments` (cargo's own, up to a `--` that ends them) that
  /// may set what the resolver reads: one naming flags, or a file it does not read.
  pub fn refuse_config(arguments: &[String]) -> Result<(), String> {
    let mut arguments = arguments.iter().take_while(|argument| *argument != "--");

    while let Some(argument) = arguments.next() {
      let value = match argument.strip_prefix("--config") {
        Some("") => arguments.next().map(String::as_str),
        Some(value) => value.strip_prefix('='),
        None => None,
      };

      if let Some(value) = value
        && (value.contains("rustflags")
          || value.contains("rustdocflags")
          || value.contains("include")
          || !value.contains('='))
      {
        return Err(format!(
          "cannot tell how `--config {value}` changes the flags cargo would use; name them in .cargo/config.toml or `RUSTFLAGS` instead"
        ));
      }
    }

    Ok(())
  }

  /// The configuration files cargo reads from `dir`, lowest precedence first, parsed.
  fn files(dir: &Path) -> Result<Vec<(PathBuf, toml::Table)>, String> {
    let home = env::var_os("CARGO_HOME")
      .map(PathBuf::from)
      .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    let mut dirs: Vec<PathBuf> = dir.ancestors().map(|dir| dir.join(".cargo")).collect();

    dirs.reverse();

    // `$CARGO_HOME` is read once, first, even where it is also an ancestor's `.cargo`.
    if let Some(home) = home {
      dirs.retain(|dir| *dir != home);
      dirs.insert(0, home);
    }

    let mut files = Vec::new();

    for dir in dirs {
      let (toml_file, legacy) = (dir.join("config.toml"), dir.join("config"));

      if toml_file.exists() && legacy.exists() {
        return Err(format!(
          "{} has both `config` and `config.toml`; cargo reads one, so the CLI reads none",
          dir.display()
        ));
      }

      for path in [toml_file, legacy] {
        if let Ok(text) = fs::read_to_string(&path) {
          let table = text
            .parse::<toml::Table>()
            .map_err(|error| format!("{}: {error}", path.display()))?;

          files.push((path, table));
        }
      }
    }

    Ok(files)
  }

  /// The flags of a configuration value: an array of strings, or one string split at blanks.
  fn list(value: &toml::Value, path: &Path) -> Result<Vec<String>, String> {
    match value {
      toml::Value::String(flags) => Ok(flags.split_whitespace().map(str::to_owned).collect()),
      toml::Value::Array(flags) => flags
        .iter()
        .map(|flag| {
          flag
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("{}: a flag is not a string", path.display()))
        })
        .collect(),
      _ => Err(format!(
        "{}: flags are a string or an array",
        path.display()
      )),
    }
  }

  /// The flags with the platform's cfg where the build named one (in place, so a build for
  /// the configured platform keeps its flags byte for byte), else appended with its
  /// `--check-cfg`.
  pub fn substitute(&self, platform: &str) -> Result<Vec<String>, String> {
    let cfg = format!(r#"skene_platform="{platform}""#);
    let mut out = Vec::new();
    let mut named = false;
    let mut checked = false;
    let mut flags = self.flags.iter().peekable();

    while let Some(flag) = flags.next() {
      let value = if flag == "--cfg" || flag == "--check-cfg" {
        flags.peek().map(|value| (flag.as_str(), value.as_str()))
      } else {
        flag
          .strip_prefix("--cfg=")
          .map(|value| ("--cfg", value))
          .or_else(|| {
            flag
              .strip_prefix("--check-cfg=")
              .map(|value| ("--check-cfg", value))
          })
      };

      match value {
        Some(("--cfg", value)) if Self::names_platform(value) => {
          if !Self::is_platform_cfg(value) {
            return Err(format!(
              "{}: cannot tell what `--cfg {value}` names; write `--cfg skene_platform=\"<platform>\"`",
              self.source
            ));
          }

          if named {
            return Err(format!(
              "{} names `skene_platform` twice; name it once",
              self.source
            ));
          }

          named = true;

          if flag == "--cfg" {
            flags.next();
            out.extend(["--cfg".to_owned(), cfg.clone()]);
          } else {
            out.push(format!("--cfg={cfg}"));
          }
        }
        Some(("--check-cfg", value)) if Self::names_platform(value) => {
          checked = true;
          out.push(flag.clone());

          if flag == "--check-cfg" {
            out.push(value.to_owned());
            flags.next();
          }
        }
        Some((_, value)) if flag == "--cfg" || flag == "--check-cfg" => {
          out.extend([flag.clone(), value.to_owned()]);
          flags.next();
        }
        _ if Self::names_platform(flag) => {
          return Err(format!(
            "{}: cannot tell what `{flag}` does to `skene_platform`",
            self.source
          ));
        }
        _ => out.push(flag.clone()),
      }
    }

    if !named {
      out.extend(["--cfg".to_owned(), cfg]);
    }

    if !checked {
      out.extend(["--check-cfg".to_owned(), CHECK_CFG.to_owned()]);
    }

    Ok(out)
  }

  /// Whether `text` mentions the platform cfg at all.
  fn names_platform(text: &str) -> bool {
    text.contains("skene_platform")
  }

  /// Whether `value` is exactly `skene_platform="<name>"`.
  fn is_platform_cfg(value: &str) -> bool {
    value
      .strip_prefix("skene_platform=\"")
      .and_then(|rest| rest.strip_suffix('"'))
      .is_some_and(|name| name.chars().all(|character| character.is_ascii_lowercase()))
  }
}

/// The cfg a target triple builds under, as rustc prints it.
struct TargetCfg(BTreeSet<String>);

impl TargetCfg {
  /// What `rustc --print cfg --target <triple>` prints.
  fn of(triple: &str) -> Result<Self, String> {
    let printed = output(
      Command::new("rustc").args(["--print", "cfg", "--target", triple]),
      "rustc --print cfg",
    )?;

    Ok(Self(printed.lines().map(str::to_owned).collect()))
  }

  /// Whether the table key `key` (`cfg(..)`) holds for the target.
  fn holds(&self, key: &str, path: &Path) -> Result<bool, String> {
    let refuse = || {
      format!(
        "{}: cannot tell whether `[target.'{key}']` applies; name the triple instead",
        path.display()
      )
    };

    if key.contains("skene_platform") {
      return Err(refuse());
    }

    let tokens = tokenize(key).ok_or_else(refuse)?;
    let mut position = 0;
    let holds = self.predicate(&tokens, &mut position).ok_or_else(refuse)?;

    // `cfg(predicate)`: the outer `cfg` is the predicate's one argument.
    (position == tokens.len())
      .then_some(holds)
      .ok_or_else(refuse)
  }

  /// Evaluates the predicate at `position`, advancing past it.
  fn predicate(&self, tokens: &[Token], position: &mut usize) -> Option<bool> {
    let Token::Word(word) = tokens.get(*position)? else {
      return None;
    };

    *position += 1;

    match (word.as_str(), tokens.get(*position)) {
      ("cfg" | "all" | "any" | "not", Some(Token::Open)) => {
        *position += 1;

        let mut values = Vec::new();

        while tokens.get(*position)? != &Token::Close {
          values.push(self.predicate(tokens, position)?);

          if tokens.get(*position)? == &Token::Comma {
            *position += 1;
          }
        }

        *position += 1;

        match word.as_str() {
          "cfg" if values.len() == 1 => Some(values[0]),
          "all" => Some(values.iter().all(|value| *value)),
          "any" => Some(values.iter().any(|value| *value)),
          "not" if values.len() == 1 => Some(!values[0]),
          _ => None,
        }
      }
      (name, Some(Token::Equals)) => {
        let Token::Text(value) = tokens.get(*position + 1)? else {
          return None;
        };

        *position += 2;

        Some(self.0.contains(&format!("{name}=\"{value}\"")))
      }
      ("true", _) => Some(true),
      ("false", _) => Some(false),
      (name, _) => Some(self.0.contains(name)),
    }
  }
}

/// A token of a cfg predicate.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Token {
  Word(String),
  Text(String),
  Open,
  Close,
  Comma,
  Equals,
}

/// The tokens of `text`, where it is a well-formed predicate's.
fn tokenize(text: &str) -> Option<Vec<Token>> {
  let mut tokens = Vec::new();
  let mut characters = text.chars().peekable();

  while let Some(character) = characters.next() {
    match character {
      '(' => tokens.push(Token::Open),
      ')' => tokens.push(Token::Close),
      ',' => tokens.push(Token::Comma),
      '=' => tokens.push(Token::Equals),
      '"' => {
        let mut value = String::new();

        loop {
          match characters.next()? {
            '"' => break,
            character => value.push(character),
          }
        }

        tokens.push(Token::Text(value));
      }
      character if character.is_whitespace() => {}
      character if character.is_alphanumeric() || character == '_' => {
        let mut word = character.to_string();

        while let Some(next) = characters
          .peek()
          .filter(|next| next.is_alphanumeric() || **next == '_')
        {
          word.push(*next);
          characters.next();
        }

        tokens.push(Token::Word(word));
      }
      _ => return None,
    }
  }

  Some(tokens)
}

#[cfg(test)]
mod tests {
  use super::{Flags, TargetCfg};

  fn flags(flags: &[&str]) -> Flags {
    Flags {
      flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
      source: "the test".to_owned(),
    }
  }

  #[test]
  fn the_platform_is_replaced_in_place() {
    let configured = flags(&[
      "-Copt-level=1",
      "--cfg",
      r#"skene_platform="ios""#,
      "--check-cfg",
      super::CHECK_CFG,
    ]);

    assert_eq!(
      configured.substitute("linux").expect("substituted"),
      [
        "-Copt-level=1",
        "--cfg",
        r#"skene_platform="linux""#,
        "--check-cfg",
        super::CHECK_CFG
      ]
    );
  }

  #[test]
  fn flags_naming_the_platform_already_are_kept_byte_for_byte() {
    let configured = flags(&[
      "--cfg",
      r#"skene_platform="linux""#,
      "--check-cfg",
      super::CHECK_CFG,
    ]);

    assert_eq!(
      configured.substitute("linux").expect("substituted"),
      configured.flags
    );
  }

  #[test]
  fn a_build_naming_none_gets_the_cfg_and_its_check() {
    assert_eq!(
      flags(&["-Dwarnings"])
        .substitute("linux")
        .expect("substituted"),
      [
        "-Dwarnings",
        "--cfg",
        r#"skene_platform="linux""#,
        "--check-cfg",
        super::CHECK_CFG
      ]
    );
  }

  #[test]
  fn what_it_cannot_read_is_refused() {
    for unreadable in [
      &["--cfg", "skene_platform"][..],
      &["--cfg=skene_platform=linux"],
      &["-Zcrate-attr=cfg(skene_platform)"],
      &[
        "--cfg",
        r#"skene_platform="ios""#,
        "--cfg",
        r#"skene_platform="web""#,
      ],
    ] {
      assert!(
        flags(unreadable).substitute("linux").is_err(),
        "{unreadable:?} is refused"
      );
    }
  }

  #[test]
  fn a_config_override_naming_flags_is_refused() {
    let arguments = |arguments: &[&str]| -> Vec<String> {
      arguments
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect()
    };

    for refused in [
      &[
        "--config",
        r#"target.x86_64-unknown-linux-gnu.rustflags=["--cfg","x"]"#,
      ][..],
      &[r#"--config=build.rustflags=["-Dwarnings"]"#],
      &["--test", "it", "--config", "extra.toml"],
    ] {
      assert!(
        Flags::refuse_config(&arguments(refused)).is_err(),
        "{refused:?} is refused"
      );
    }

    for kept in [
      &["--config", "net.offline=true"][..],
      &["--test", "it", "--", "--config", "extra.toml"],
    ] {
      assert!(
        Flags::refuse_config(&arguments(kept)).is_ok(),
        "{kept:?} is kept"
      );
    }
  }

  #[test]
  fn a_target_cfg_table_is_evaluated_against_the_triple() {
    let linux = TargetCfg(
      [
        "unix",
        r#"target_os="linux""#,
        r#"target_arch="x86_64""#,
        r#"target_family="unix""#,
      ]
      .into_iter()
      .map(str::to_owned)
      .collect(),
    );
    let path = std::path::Path::new("config.toml");

    assert_eq!(linux.holds("cfg(all())", path), Ok(true));
    assert_eq!(linux.holds("cfg(unix)", path), Ok(true));
    assert_eq!(linux.holds(r#"cfg(target_os = "macos")"#, path), Ok(false));
    assert_eq!(
      linux.holds(r#"cfg(all(unix, not(target_arch = "aarch64")))"#, path),
      Ok(true)
    );
    assert_eq!(
      linux.holds(r#"cfg(any(windows, target_os = "ios"))"#, path),
      Ok(false)
    );
    assert_eq!(linux.holds("cfg(true)", path), Ok(true));
    assert_eq!(linux.holds("cfg(not(false))", path), Ok(true));
    assert_eq!(linux.holds("cfg(any(false, windows))", path), Ok(false));
    assert!(linux.holds("cfg(all(unix)", path).is_err(), "malformed");
    assert!(
      linux
        .holds(r#"cfg(skene_platform = "linux")"#, path)
        .is_err()
    );
  }
}
