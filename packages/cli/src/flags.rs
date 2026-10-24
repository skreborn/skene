//! The flags cargo would hand rustc or rustdoc for a target, read from cargo's own sources
//! in cargo's order, and the build's platform named among them in place.
//!
//! Cargo takes rustc's flags from the first of these that gives any:
//! `CARGO_ENCODED_RUSTFLAGS`; `RUSTFLAGS`; `target.<triple>.rustflags` followed by every
//! matching `target.'cfg(..)'.rustflags` in the order of their keys; `build.rustflags`.
//! A `cfg(..)` table matches against the target's cfg as rustc prints it under the flags
//! found without the tables, then under the flags found with them, until the flags stay
//! the same. Rustdoc's are the same with `rustdocflags`, which no `cfg(..)` table
//! carries. A configuration is each `.cargo/config.toml` from the directory cargo runs in
//! up to the filesystem root, the nearer taking precedence, and beneath them all
//! `$CARGO_HOME/config.toml` where no ancestor's is that file, merged as cargo merges
//! them: an array joined across the files with the higher's last, a string the higher's.
//! What the resolver cannot read exactly (a configuration `include`, the
//! `CARGO_BUILD_*`/`CARGO_TARGET_*` variables, a `cfg(..)` table it cannot evaluate or
//! whose flags never settle, a flag naming a platform cfg in a form it does not know) is a
//! refusal, never a guess.

use std::{
  collections::{BTreeMap, BTreeSet},
  env,
  fs,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  sync::OnceLock,
};

/// The cfg naming the build's platform.
const PLATFORM: &str = "skene_platform";

/// The cfg naming the web's server leaf beside it.
const SERVER: &str = "skene_server";

/// Which tool's flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
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

  /// The environment variables cargo reads first, encoded then plain.
  const fn variables(self) -> (&'static str, &'static str) {
    match self {
      Self::Rust => ("CARGO_ENCODED_RUSTFLAGS", "RUSTFLAGS"),
      Self::Rustdoc => ("CARGO_ENCODED_RUSTDOCFLAGS", "RUSTDOCFLAGS"),
    }
  }
}

/// A build's platform as its cfgs name it: the value of `skene_platform`, and whether
/// `skene_server` stands beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
  /// The web's browser leaf.
  Web,
  /// The web's server leaf.
  Server,
  /// The terminal.
  Terminal,
}

impl Selection {
  /// The value of `skene_platform`.
  const fn platform(self) -> &'static str {
    match self {
      Self::Web | Self::Server => "web",
      Self::Terminal => "terminal",
    }
  }

  /// Whether the build is the web's server leaf.
  const fn server(self) -> bool {
    matches!(self, Self::Server)
  }

  /// The target the build compiles for: `wasm32-unknown-unknown` for the browser, the
  /// host's own (`host-tuple`) for the others.
  const fn target(self) -> &'static str {
    match self {
      Self::Web => "wasm32-unknown-unknown",
      Self::Server | Self::Terminal => "host-tuple",
    }
  }

  /// Names the build on `command`, a cargo build run in `dir`: its triple as `--target`,
  /// and the flags cargo would use there with this selection's cfgs in place of whichever
  /// the build names, as the encoded variables cargo ranks above every other source.
  /// Since the target is named, they reach what is compiled for it alone, never a build
  /// script or a proc macro.
  pub(crate) fn name(self, command: &mut Command, dir: &Path) -> Result<(), String> {
    let triple = triple(self.target())?;
    let rust = Flags::resolve(dir, &triple, Kind::Rust)?.substitute(self)?;
    let rustdoc = Flags::resolve(dir, &triple, Kind::Rustdoc)?.substitute(self)?;

    command
      .args(["--target", &triple])
      .env("CARGO_ENCODED_RUSTFLAGS", rust.join("\x1f"))
      .env("CARGO_ENCODED_RUSTDOCFLAGS", rustdoc.join("\x1f"))
      .env_remove("RUSTFLAGS")
      .env_remove("RUSTDOCFLAGS");

    Ok(())
  }
}

/// The triple `name` stands for: the host's, as rustc reports it, for `host-tuple`, and
/// `name` itself otherwise.
fn triple(name: &str) -> Result<String, String> {
  static HOST: OnceLock<Result<String, String>> = OnceLock::new();

  if name != "host-tuple" {
    return Ok(name.to_owned());
  }

  HOST
    .get_or_init(|| {
      run(Command::new("rustc").arg("-vV"), "rustc -vV")?
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .ok_or_else(|| "rustc -vV names no host".to_owned())
    })
    .clone()
}

/// Runs `command`, and returns what it printed; `what` names it in the error.
fn run(command: &mut Command, what: &str) -> Result<String, String> {
  let output = command
    .stdin(Stdio::null())
    .output()
    .map_err(|error| format!("could not run {what}: {error}"))?;

  if !output.status.success() {
    return Err(format!(
      "{what} failed: {}",
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }

  Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The flags cargo would use, and where they came from.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Flags {
  /// The flags, in order.
  flags: Vec<String>,
  /// The source cargo would read them from.
  source: String,
}

impl Flags {
  /// The flags cargo run in `dir` would use for `triple`.
  fn resolve(dir: &Path, triple: &str, kind: Kind) -> Result<Self, String> {
    if let Some(flags) = Self::from_environment(triple, kind, |name| env::var(name).ok())? {
      return Ok(flags);
    }

    Config::read(dir)?.flags(triple, kind, |flags| TargetCfg::of(dir, triple, flags))
  }

  /// The flags the environment `variable` reads gives, where it gives them; a variable
  /// cargo merges with the configuration is a refusal.
  fn from_environment(
    triple: &str,
    kind: Kind,
    variable: impl Fn(&str) -> Option<String>,
  ) -> Result<Option<Self>, String> {
    let (encoded, plain) = kind.variables();

    if let Some(flags) = variable(encoded) {
      return Ok(Some(Self {
        flags: flags
          .split('\x1f')
          .filter(|flag| !flag.is_empty())
          .map(str::to_owned)
          .collect(),
        source: encoded.to_owned(),
      }));
    }

    if let Some(flags) = variable(plain) {
      return Ok(Some(Self {
        flags: flags
          .split(' ')
          .map(str::trim)
          .filter(|flag| !flag.is_empty())
          .map(str::to_owned)
          .collect(),
        source: plain.to_owned(),
      }));
    }

    let key = kind.key().to_uppercase();
    let upper = triple.to_uppercase().replace(['-', '.'], "_");

    for merged in [
      format!("CARGO_BUILD_{key}"),
      format!("CARGO_TARGET_{upper}_{key}"),
    ] {
      if variable(&merged).is_some() {
        return Err(format!(
          "cannot tell how cargo merges `{merged}` with the configuration; set `{plain}` \
           instead"
        ));
      }
    }

    Ok(None)
  }

  /// The flags of a configuration value: an array of strings, or one string split at
  /// blanks.
  fn list(value: &toml::Value, key: &str) -> Result<Vec<String>, String> {
    match value {
      toml::Value::String(flags) => Ok(flags.split_whitespace().map(str::to_owned).collect()),
      toml::Value::Array(flags) => flags
        .iter()
        .map(|flag| {
          flag
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("`{key}`: a flag is not a string"))
        })
        .collect(),
      _ => Err(format!("`{key}`: flags are a string or an array")),
    }
  }

  /// The flags with `selection`'s cfgs in place of the platform cfgs they name, so a
  /// build for the platform the flags name keeps them byte for byte: `skene_platform`'s
  /// value replaced where it stands, `skene_server` kept, removed or added after it as
  /// the selection asks, and both appended where the flags name no platform.
  fn substitute(&self, selection: Selection) -> Result<Vec<String>, String> {
    let platform = format!("{PLATFORM}=\"{}\"", selection.platform());
    let cfgs = Cfgs::of(&self.flags, &self.source)?;
    let add_server = selection.server()
      && !cfgs
        .iter()
        .any(|(_, cfg)| matches!(cfg, Some(Cfg::Server(_))));
    let mut out = Vec::new();
    let mut named = false;

    for (flag, cfg) in cfgs {
      match cfg {
        Some(Cfg::Platform(spelling)) => {
          named = true;
          spelling.push(&platform, &mut out);

          if add_server {
            spelling.push(SERVER, &mut out);
          }
        }
        Some(Cfg::Server(spelling)) => {
          if selection.server() {
            spelling.push(SERVER, &mut out);
          }
        }
        None => out.extend(flag.iter().cloned()),
      }
    }

    if !named {
      Spelling::Apart.push(&platform, &mut out);

      if add_server {
        Spelling::Apart.push(SERVER, &mut out);
      }
    }

    Ok(out)
  }
}

/// How a `--cfg` is spelled: as two flags, or joined by `=`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spelling {
  /// `--cfg`, then its value.
  Apart,
  /// `--cfg=<value>`.
  Joined,
}

impl Spelling {
  /// Pushes `--cfg <value>`, spelled so, onto `out`.
  fn push(self, value: &str, out: &mut Vec<String>) {
    match self {
      Self::Apart => out.extend(["--cfg".to_owned(), value.to_owned()]),
      Self::Joined => out.push(format!("--cfg={value}")),
    }
  }
}

/// A platform cfg among flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cfg {
  /// `skene_platform="<name>"`.
  Platform(Spelling),
  /// `skene_server`.
  Server(Spelling),
}

/// A flag, or one taking a value with its value, and the platform cfg it names.
type Group<'a> = (&'a [String], Option<Cfg>);

/// Flags grouped as rustc reads them, each group with the platform cfg it names.
struct Cfgs;

impl Cfgs {
  /// The groups of `flags` (a flag, or one taking a value with its value), each with the
  /// platform cfg it names; `source` names the flags in a refusal.
  fn of<'a>(flags: &'a [String], source: &str) -> Result<Vec<Group<'a>>, String> {
    let mut groups = Vec::new();
    let mut platforms = 0;
    let mut servers = 0;
    let mut index = 0;

    while index < flags.len() {
      let flag = flags[index].as_str();
      let (length, value, spelling) = match flag {
        "--cfg" => {
          let value = flags
            .get(index + 1)
            .ok_or_else(|| format!("{source} ends in `--cfg`"))?;

          (2, Some(value.as_str()), Spelling::Apart)
        }
        "--check-cfg" => (2, None, Spelling::Apart),
        _ => (1, flag.strip_prefix("--cfg="), Spelling::Joined),
      };
      let group = &flags[index..(index + length).min(flags.len())];
      let cfg = match value {
        Some(value) if Self::is_platform(value) => {
          platforms += 1;

          Some(Cfg::Platform(spelling))
        }
        Some(SERVER) => {
          servers += 1;

          Some(Cfg::Server(spelling))
        }
        Some(value) if Self::mentions(value) => {
          return Err(format!(
            "{source}: cannot tell what `--cfg {value}` names; write \
             `--cfg {PLATFORM}=\"<platform>\"` and `--cfg {SERVER}`"
          ));
        }
        None if !flag.starts_with("--check-cfg") && Self::mentions(flag) => {
          return Err(format!(
            "{source}: cannot tell what `{flag}` does to the platform's cfgs"
          ));
        }
        _ => None,
      };

      groups.push((group, cfg));
      index += length;
    }

    if platforms > 1 || servers > 1 {
      return Err(format!(
        "{source} names a platform cfg twice; name each once"
      ));
    }

    Ok(groups)
  }

  /// Whether `text` mentions a platform cfg at all.
  fn mentions(text: &str) -> bool {
    text.contains(PLATFORM) || text.contains(SERVER)
  }

  /// Whether `value` is exactly `skene_platform="<name>"`.
  fn is_platform(value: &str) -> bool {
    value
      .strip_prefix(PLATFORM)
      .and_then(|rest| rest.strip_prefix("=\""))
      .and_then(|rest| rest.strip_suffix('"'))
      .is_some_and(|name| {
        !name.is_empty() && name.chars().all(|character| character.is_ascii_lowercase())
      })
  }
}

/// The configuration cargo reads, its files merged.
#[derive(Debug, Default)]
struct Config {
  /// The merged values.
  table: toml::Table,
}

impl Config {
  /// The configuration cargo run in `dir` reads.
  fn read(dir: &Path) -> Result<Self, String> {
    let home = env::var_os("CARGO_HOME")
      .map(PathBuf::from)
      .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));

    Self::merge(Self::files(dir, home.as_deref())?)
  }

  /// The configuration files cargo run in `dir` reads, `home` its `$CARGO_HOME`, each
  /// with its table, lowest precedence first: `home`'s, where no ancestor's `.cargo` is
  /// `home`, then each ancestor's from the filesystem root down to `dir`.
  fn files(dir: &Path, home: Option<&Path>) -> Result<Vec<(PathBuf, toml::Table)>, String> {
    let mut dirs: Vec<PathBuf> = dir.ancestors().map(|dir| dir.join(".cargo")).collect();

    if let Some(home) = home
      && !dirs.iter().any(|dir| dir == home)
    {
      dirs.push(home.to_owned());
    }

    dirs.reverse();

    let mut files = Vec::new();

    for dir in dirs {
      let (toml_file, legacy) = (dir.join("config.toml"), dir.join("config"));

      if toml_file.is_file() && legacy.is_file() {
        return Err(format!(
          "{} holds both `config` and `config.toml`; name the flags in one",
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

  /// `files`, lowest precedence first, merged as cargo merges them: tables key by key,
  /// arrays joined, any other value the later file's.
  fn merge(files: Vec<(PathBuf, toml::Table)>) -> Result<Self, String> {
    let mut config = Self::default();

    for (path, table) in files {
      if table.contains_key("include") {
        return Err(format!(
          "{}: a configuration `include` is not read; name the flags in the file itself",
          path.display()
        ));
      }

      Self::merge_table(&mut config.table, table, &path, "")?;
    }

    Ok(config)
  }

  /// Merges `from`, read from `path`, into `into`, whose keys sit under `prefix`.
  fn merge_table(
    into: &mut toml::Table,
    from: toml::Table,
    path: &Path,
    prefix: &str,
  ) -> Result<(), String> {
    for (key, value) in from {
      let name = format!("{prefix}{key}");

      match (into.get_mut(&key), value) {
        (None, value) => {
          into.insert(key, value);
        }
        (Some(toml::Value::Table(old)), toml::Value::Table(new)) => {
          Self::merge_table(old, new, path, &format!("{name}."))?;
        }
        (Some(toml::Value::Array(old)), toml::Value::Array(new)) => old.extend(new),
        (Some(old), new)
          if matches!(old, toml::Value::Table(_) | toml::Value::Array(_))
            || matches!(new, toml::Value::Table(_) | toml::Value::Array(_)) =>
        {
          return Err(format!(
            "{}: cargo cannot merge `{name}` with the value an earlier file gives it",
            path.display()
          ));
        }
        (Some(old), new) => *old = new,
      }
    }

    Ok(())
  }

  /// The flags of `kind` the configuration gives `triple`, `cfg` the target's cfg under
  /// the flags it is given, asked where a `cfg(..)` table might match.
  fn flags(
    &self,
    triple: &str,
    kind: Kind,
    cfg: impl Fn(&[String]) -> Result<TargetCfg, String>,
  ) -> Result<Flags, String> {
    let mut flags = self.flags_under(triple, kind, None)?;

    if self.cfg_tables(kind).is_empty() {
      return Ok(flags);
    }

    // At most the turns cargo takes before it stops.
    for _ in 0..=20 {
      let next = self.flags_under(triple, kind, Some(&cfg(&flags.flags)?))?;

      if next.flags == flags.flags {
        return Ok(next);
      }

      flags = next;
    }

    Err(format!(
      "cannot tell which `[target.'cfg(..)']` tables apply to {triple}: the flags they \
       give change which apply; name the triple instead"
    ))
  }

  /// The `cfg(..)` tables giving flags of `kind`, by key in the order of their keys.
  fn cfg_tables(&self, kind: Kind) -> BTreeMap<&str, &toml::Value> {
    let key = kind.key();

    // `cfg(..)` tables carry rustc's flags alone.
    match kind {
      Kind::Rust => self
        .table
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flatten()
        .filter(|(name, _)| name.starts_with("cfg("))
        .filter_map(|(name, section)| Some((name.as_str(), section.get(key)?)))
        .collect(),
      Kind::Rustdoc => BTreeMap::new(),
    }
  }

  /// The flags of `kind` the configuration gives `triple`, the `cfg(..)` tables matched
  /// against `cfg` where it is given and left out where it is not: the triple's and the
  /// matching tables', joined, or `build`'s where those give none.
  fn flags_under(
    &self,
    triple: &str,
    kind: Kind,
    cfg: Option<&TargetCfg>,
  ) -> Result<Flags, String> {
    let key = kind.key();
    let mut flags = Vec::new();
    let mut sources = Vec::new();

    if let Some(value) = self
      .table
      .get("target")
      .and_then(|targets| targets.get(triple))
      .and_then(|section| section.get(key))
    {
      flags.extend(Flags::list(value, &format!("target.{triple}.{key}"))?);
      sources.push(format!("target.{triple}.{key}"));
    }

    if let Some(cfg) = cfg {
      for (name, value) in self.cfg_tables(kind) {
        if cfg.holds(name)? {
          flags.extend(Flags::list(value, &format!("target.'{name}'.{key}"))?);
          sources.push(format!("target.'{name}'.{key}"));
        }
      }
    }

    if !flags.is_empty() {
      return Ok(Flags {
        flags,
        source: sources.join(", "),
      });
    }

    if let Some(value) = self.table.get("build").and_then(|build| build.get(key)) {
      return Ok(Flags {
        flags: Flags::list(value, &format!("build.{key}"))?,
        source: format!("build.{key}"),
      });
    }

    Ok(Flags {
      flags: Vec::new(),
      source: "no source".to_owned(),
    })
  }
}

/// The cfg a target triple builds under, as rustc prints it.
#[derive(Debug)]
struct TargetCfg(BTreeSet<String>);

impl TargetCfg {
  /// What `rustc --print cfg --target <triple>` prints under `flags`, run in `dir` as
  /// cargo runs it, through `RUSTC` where it is set.
  fn of(dir: &Path, triple: &str, flags: &[String]) -> Result<Self, String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let printed = run(
      Command::new(rustc)
        .current_dir(dir)
        .args(["--print", "cfg", "--target", triple])
        .args(flags),
      "rustc --print cfg",
    )?;

    Ok(Self(printed.lines().map(str::to_owned).collect()))
  }

  /// Whether the table key `key` (`cfg(..)`) holds for the target.
  fn holds(&self, key: &str) -> Result<bool, String> {
    let refuse =
      || format!("cannot tell whether `[target.'{key}']` applies; name the triple instead");

    if Cfgs::mentions(key) {
      return Err(refuse());
    }

    let tokens = Token::all(key).ok_or_else(refuse)?;
    let mut position = 0;
    let holds = self.predicate(&tokens, &mut position).ok_or_else(refuse)?;

    // `cfg(predicate)`: the outer `cfg` is the predicate's one argument.
    (position == tokens.len())
      .then_some(holds)
      .ok_or_else(refuse)
  }

  /// Evaluates the predicate at `position`, advancing past it.
  fn predicate(&self, tokens: &[Token], position: &mut usize) -> Option<bool> {
    let Token::Word(word) = tokens.get(*position)?
    else {
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
        let Token::Text(value) = tokens.get(*position + 1)?
        else {
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
#[derive(Clone, Debug, PartialEq, Eq)]
enum Token {
  /// A name.
  Word(String),
  /// A quoted value.
  Text(String),
  /// `(`.
  Open,
  /// `)`.
  Close,
  /// `,`.
  Comma,
  /// `=`.
  Equals,
}

impl Token {
  /// The tokens of `text`, where it is a well-formed predicate's.
  fn all(text: &str) -> Option<Vec<Self>> {
    let mut tokens = Vec::new();
    let mut characters = text.chars().peekable();

    while let Some(character) = characters.next() {
      match character {
        '(' => tokens.push(Self::Open),
        ')' => tokens.push(Self::Close),
        ',' => tokens.push(Self::Comma),
        '=' => tokens.push(Self::Equals),
        '"' => {
          let mut value = String::new();

          loop {
            match characters.next()? {
              '"' => break,
              character => value.push(character),
            }
          }

          tokens.push(Self::Text(value));
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

          tokens.push(Self::Word(word));
        }
        _ => return None,
      }
    }

    Some(tokens)
  }
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    path::{Path, PathBuf},
  };

  use super::{Config, Flags, Kind, Selection, TargetCfg, triple};

  /// Flags from a test.
  fn flags(flags: &[&str]) -> Flags {
    Flags {
      flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
      source: "the test".to_owned(),
    }
  }

  /// Linux on x86-64, as rustc prints its cfg.
  fn linux() -> TargetCfg {
    TargetCfg(
      [
        "unix",
        r#"target_os="linux""#,
        r#"target_arch="x86_64""#,
        r#"target_family="unix""#,
      ]
      .into_iter()
      .map(str::to_owned)
      .collect(),
    )
  }

  /// `files`, each a path and its text, merged.
  fn config(files: &[(&str, &str)]) -> Config {
    Config::merge(
      files
        .iter()
        .map(|(path, text)| (PathBuf::from(path), text.parse().expect("a table")))
        .collect(),
    )
    .expect("merged")
  }

  /// A substitution replaces the platform's value where it stands.
  #[test]
  fn the_platform_is_replaced_in_place() {
    let configured = flags(&[
      "-Copt-level=1",
      "--cfg",
      r#"skene_platform="web""#,
      "-Dwarnings",
    ]);

    assert_eq!(
      configured
        .substitute(Selection::Terminal)
        .expect("substituted"),
      [
        "-Copt-level=1",
        "--cfg",
        r#"skene_platform="terminal""#,
        "-Dwarnings"
      ]
    );
    assert_eq!(
      flags(&[r#"--cfg=skene_platform="terminal""#])
        .substitute(Selection::Web)
        .expect("substituted"),
      [r#"--cfg=skene_platform="web""#]
    );
  }

  /// The server leaf's cfg is added after the platform's, kept where it stands, and
  /// removed for any other build.
  #[test]
  fn the_server_leaf_is_added_kept_or_removed() {
    let browser = flags(&["--cfg", r#"skene_platform="web""#, "-Dwarnings"]);
    let server = flags(&[
      "--cfg",
      "skene_server",
      "-Dwarnings",
      "--cfg",
      r#"skene_platform="web""#,
    ]);

    assert_eq!(
      browser.substitute(Selection::Server).expect("substituted"),
      [
        "--cfg",
        r#"skene_platform="web""#,
        "--cfg",
        "skene_server",
        "-Dwarnings"
      ]
    );
    assert_eq!(
      server.substitute(Selection::Server).expect("substituted"),
      server.flags
    );
    assert_eq!(
      server.substitute(Selection::Web).expect("substituted"),
      ["-Dwarnings", "--cfg", r#"skene_platform="web""#]
    );
  }

  /// Flags naming the build's own platform already come back byte for byte, so nothing
  /// the configured build compiled is compiled again.
  #[test]
  fn flags_naming_the_platform_already_are_kept_byte_for_byte() {
    for (configured, selection) in [
      (
        flags(&[
          "--cfg",
          r#"skene_platform="terminal""#,
          "--check-cfg",
          "cfg(skene_server)",
          "--check-cfg=cfg(skene_platform)",
        ]),
        Selection::Terminal,
      ),
      (
        flags(&["--cfg", r#"skene_platform="web""#, "--cfg", "skene_server"]),
        Selection::Server,
      ),
      (flags(&[r#"--cfg=skene_platform="web""#]), Selection::Web),
    ] {
      assert_eq!(
        configured.substitute(selection).expect("substituted"),
        configured.flags
      );
    }
  }

  /// A build naming no platform gets the selection's cfgs appended.
  #[test]
  fn a_build_naming_none_gets_the_cfgs() {
    assert_eq!(
      flags(&["-Dwarnings"])
        .substitute(Selection::Server)
        .expect("substituted"),
      [
        "-Dwarnings",
        "--cfg",
        r#"skene_platform="web""#,
        "--cfg",
        "skene_server"
      ]
    );
    assert_eq!(
      flags(&[]).substitute(Selection::Web).expect("substituted"),
      ["--cfg", r#"skene_platform="web""#]
    );
  }

  /// A platform cfg in a form the substitution does not know is refused.
  #[test]
  fn what_it_cannot_read_is_refused() {
    for unreadable in [
      &["--cfg", "skene_platform"][..],
      &["--cfg=skene_platform=web"],
      &["--cfg", r#"skene_server="yes""#],
      &["-Zcrate-attr=cfg(skene_platform)"],
      &["--cfg"],
      &[
        "--cfg",
        r#"skene_platform="terminal""#,
        "--cfg",
        r#"skene_platform="web""#,
      ],
      &["--cfg", "skene_server", "--cfg=skene_server"],
    ] {
      assert!(
        flags(unreadable).substitute(Selection::Web).is_err(),
        "{unreadable:?} is refused"
      );
    }
  }

  /// The encoded variable wins over the plain one, both over the configuration, and a
  /// variable cargo merges with the configuration is refused.
  #[test]
  fn the_environment_is_read_in_cargos_order() {
    let read = |variables: &[(&str, &str)], kind| {
      Flags::from_environment("x86_64-unknown-linux-gnu", kind, |name| {
        variables
          .iter()
          .find(|(variable, _)| *variable == name)
          .map(|(_, value)| (*value).to_owned())
      })
    };

    assert_eq!(
      read(
        &[
          ("CARGO_ENCODED_RUSTFLAGS", "--cfg\x1fx"),
          ("RUSTFLAGS", "--cfg y")
        ],
        Kind::Rust
      )
      .expect("read")
      .map(|flags| flags.flags),
      Some(vec!["--cfg".to_owned(), "x".to_owned()])
    );
    assert_eq!(
      read(&[("RUSTDOCFLAGS", "--cfg  y ")], Kind::Rustdoc)
        .expect("read")
        .map(|flags| flags.flags),
      Some(vec!["--cfg".to_owned(), "y".to_owned()])
    );
    assert_eq!(
      read(&[("RUSTFLAGS", " -C\topt-level=1 ")], Kind::Rust)
        .expect("read")
        .map(|flags| flags.flags),
      Some(vec!["-C\topt-level=1".to_owned()]),
      "`RUSTFLAGS` splits at spaces alone, as cargo splits it"
    );
    assert_eq!(read(&[("RUSTFLAGS", "x")], Kind::Rustdoc), Ok(None));

    for merged in [
      "CARGO_BUILD_RUSTFLAGS",
      "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
    ] {
      assert!(read(&[(merged, "x")], Kind::Rust).is_err(), "{merged}");
    }
  }

  /// The configuration's flags are the triple's and its matching `cfg(..)` tables',
  /// joined in the order of their keys, and `build`'s only where none of those gives
  /// any; rustdoc's take no `cfg(..)` table.
  #[test]
  fn the_configuration_is_read_in_cargos_order() {
    let read = |text: &str, kind| {
      config(&[("config.toml", text)])
        .flags("x86_64-unknown-linux-gnu", kind, |_| Ok(linux()))
        .expect("read")
        .flags
    };
    let text = r#"
      build.rustflags = ["-Dbuild"]
      build.rustdocflags = "-D doc"
      target.x86_64-unknown-linux-gnu.rustflags = ["-Dtriple"]
      target.'cfg(unix)'.rustflags = ["-Dunix"]
      target.'cfg(all())'.rustflags = ["-Dall"]
      target.'cfg(windows)'.rustflags = ["-Dwindows"]
    "#;

    assert_eq!(read(text, Kind::Rust), ["-Dtriple", "-Dall", "-Dunix"]);
    assert_eq!(read(text, Kind::Rustdoc), ["-D", "doc"]);
    assert_eq!(
      read(
        "build.rustflags = ['-Dbuild']\n[target.'cfg(windows)']\nrustflags = ['-Dw']",
        Kind::Rust
      ),
      ["-Dbuild"]
    );
    assert_eq!(read("", Kind::Rust), Vec::<String>::new());
    assert_eq!(
      read(
        "build.rustflags = ['-Dbuild']\ntarget.x86_64-unknown-linux-gnu.rustflags = []",
        Kind::Rust
      ),
      ["-Dbuild"],
      "a triple giving no flags falls back to `build`'s"
    );
    assert!(
      config(&[(
        "config.toml",
        "[target.'cfg(skene_server)']\nrustflags = []"
      )])
      .flags("x86_64-unknown-linux-gnu", Kind::Rust, |_| Ok(linux()))
      .is_err(),
      "a table on a platform cfg is refused"
    );
  }

  /// A `cfg(..)` table matches against the cfg rustc prints under the flags found before
  /// it, until the flags settle, and flags that never settle are refused.
  #[test]
  fn a_cfg_table_matches_under_the_flags_found_before_it() {
    // The target's cfg as rustc prints it under `flags`: Linux's, and each `--cfg` named.
    let cfg = |flags: &[String]| {
      let mut cfg = linux();

      cfg.0.extend(
        flags
          .windows(2)
          .filter(|pair| pair[0] == "--cfg")
          .map(|pair| pair[1].clone()),
      );

      Ok(cfg)
    };
    let read = |text: &str| {
      config(&[("config.toml", text)]).flags("x86_64-unknown-linux-gnu", Kind::Rust, cfg)
    };

    assert_eq!(
      read(
        "target.x86_64-unknown-linux-gnu.rustflags = ['--cfg', 'simd']\n\
         target.'cfg(simd)'.rustflags = ['-Dsimd']"
      )
      .expect("read")
      .flags,
      ["--cfg", "simd", "-Dsimd"]
    );
    assert_eq!(
      read("build.rustflags = ['--cfg', 'simd']\ntarget.'cfg(not(simd))'.rustflags = ['-Dno']")
        .expect("read")
        .flags,
      ["--cfg", "simd"],
      "`build`'s flags decide which tables match"
    );
    assert!(
      read("build.rustflags = ['--cfg', 'simd']\ntarget.'cfg(simd)'.rustflags = ['-Dsimd']")
        .is_err(),
      "a table whose flags unmatch it never settles"
    );
  }

  /// Cargo reads each ancestor's `.cargo`, nearer first, then `$CARGO_HOME` where no
  /// ancestor's is it, and refuses a directory holding both `config` and `config.toml`.
  #[test]
  fn configuration_files_are_found_as_cargo_finds_them() {
    let root = std::env::temp_dir().join(format!("skene-flags-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let write = |dir: &Path, file: &str, flag: &str| {
      fs::create_dir_all(dir.join(".cargo")).expect("created");
      fs::write(
        dir.join(".cargo").join(file),
        format!("build.rustflags = ['{flag}']"),
      )
      .expect("written");
    };
    let home = root.join("user");
    let app = home.join("work/app");
    // The files found within `root`, by their paths below it, lowest precedence first.
    let found = |home: &Path| -> Vec<PathBuf> {
      Config::files(&app, Some(home))
        .expect("found")
        .into_iter()
        .filter_map(|(path, _)| Some(path.strip_prefix(&root).ok()?.to_owned()))
        .collect()
    };

    write(&root, "config.toml", "-Droot");
    write(&home, "config.toml", "-Dhome");
    write(&app, "config", "-Dapp");

    assert_eq!(
      found(&home.join(".cargo")),
      [
        ".cargo/config.toml",
        "user/.cargo/config.toml",
        "user/work/app/.cargo/config"
      ]
      .map(PathBuf::from),
      "a home among the ancestors is read where it stands"
    );

    let elsewhere = root.join("elsewhere");

    write(&elsewhere, "config.toml", "-Delsewhere");

    assert_eq!(
      found(&elsewhere.join(".cargo")),
      [
        "elsewhere/.cargo/config.toml",
        ".cargo/config.toml",
        "user/.cargo/config.toml",
        "user/work/app/.cargo/config"
      ]
      .map(PathBuf::from),
      "a home elsewhere is read first"
    );

    write(&app, "config.toml", "-Dapp");

    assert!(Config::files(&app, None).is_err(), "both files are refused");

    fs::remove_dir_all(&root).expect("removed");
  }

  /// Files merge as cargo merges them: arrays joined with the deeper file's last, a
  /// string the deeper file's, an array beside a string refused, and an `include`
  /// refused.
  #[test]
  fn configuration_files_merge_as_cargo_merges_them() {
    let merged = config(&[
      (
        "home/config.toml",
        "build.rustflags = ['-Dhome']\nbuild.rustdocflags = '-Dhome'",
      ),
      (
        "app/config.toml",
        "build.rustflags = ['-Dapp']\nbuild.rustdocflags = '-Dapp'",
      ),
    ]);
    let read = |kind| {
      merged
        .flags("wasm32-unknown-unknown", kind, |_| Ok(linux()))
        .expect("read")
        .flags
    };

    assert_eq!(read(Kind::Rust), ["-Dhome", "-Dapp"]);
    assert_eq!(read(Kind::Rustdoc), ["-Dapp"]);

    for files in [
      &[
        ("home/config.toml", "build.rustflags = ['-Dhome']"),
        ("app/config.toml", "build.rustflags = '-Dapp'"),
      ][..],
      &[("app/config.toml", "include = 'more.toml'")],
    ] {
      assert!(
        Config::merge(
          files
            .iter()
            .map(|(path, text)| (PathBuf::from(path), text.parse().expect("a table")))
            .collect()
        )
        .is_err(),
        "{files:?} is refused"
      );
    }
  }

  /// A `cfg(..)` table key is evaluated against the triple's cfg.
  #[test]
  fn a_target_cfg_table_is_evaluated_against_the_triple() {
    let linux = linux();

    assert_eq!(linux.holds("cfg(all())"), Ok(true));
    assert_eq!(linux.holds("cfg(unix)"), Ok(true));
    assert_eq!(linux.holds(r#"cfg(target_os = "macos")"#), Ok(false));
    assert_eq!(
      linux.holds(r#"cfg(all(unix, not(target_arch = "aarch64")))"#),
      Ok(true)
    );
    assert_eq!(
      linux.holds(r#"cfg(any(windows, target_os = "ios"))"#),
      Ok(false)
    );
    assert_eq!(linux.holds("cfg(true)"), Ok(true));
    assert_eq!(linux.holds("cfg(not(false))"), Ok(true));
    assert_eq!(linux.holds("cfg(any(false, windows))"), Ok(false));
    assert!(linux.holds("cfg(all(unix)").is_err(), "malformed");
    assert!(linux.holds(r#"cfg(skene_platform = "web")"#).is_err());
  }

  /// `host-tuple` is the host's triple as rustc reports it, and any other triple is
  /// itself.
  #[test]
  fn host_tuple_is_the_hosts_triple() {
    let host = triple("host-tuple").expect("rustc reports a host");

    assert!(!host.is_empty() && host != "host-tuple", "{host}");
    assert_eq!(
      triple("wasm32-unknown-unknown").as_deref(),
      Ok("wasm32-unknown-unknown")
    );
  }

  /// The browser builds for wasm32, the server leaf and the terminal for the host.
  #[test]
  fn each_selection_builds_for_its_target() {
    assert_eq!(Selection::Web.target(), "wasm32-unknown-unknown");
    assert_eq!(Selection::Server.target(), "host-tuple");
    assert_eq!(Selection::Terminal.target(), "host-tuple");
  }
}
