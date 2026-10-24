//! The house rules no compiler checks: unspaced em-dashes, children last, the
//! framework's own vocabulary, fields and variants documented all or none, a test
//! double's simulated events named `trigger_`, the spine naming nothing of the
//! foundation outside itself, and nothing in the foundation crate-private.

use std::{
  fs,
  path::{Path, PathBuf},
};

use proc_macro2::Span;
use syn::{
  Attribute,
  Expr,
  ExprCall,
  ExprMethodCall,
  Ident,
  ItemEnum,
  ItemFn,
  ItemStruct,
  Path as SynPath,
  Stmt,
  UsePath,
  UseTree,
  Visibility,
  visit::{self, Visit},
};

/// The methods that give a view its content, which a builder chain ends with.
const CHILD_SETTERS: &[&str] = &[
  "add_case",
  "add_child",
  "add_spans",
  "derive_child",
  "derive_children",
  "derive_entries",
  "set_child",
  "set_child_builder",
  "set_child_loader",
  "set_children",
  "set_face",
];

/// The prefixes of a builder's property and handler methods.
const PROPERTY_PREFIXES: &[&str] = &["add_", "derive_", "handle_", "set_"];

/// Words the public surface never uses, the framework's own vocabulary standing in.
const BANNED_WORDS: &[&str] = &["backend", "display", "gap", "justify", "widget"];

/// The keywords a snapshot line names before the path of the item it lists.
const ITEM_KEYWORDS: &[&str] = &[
  "async", "const", "enum", "extern", "fn", "macro", "mod", "static", "struct", "trait", "type",
  "union", "unsafe", "use",
];

/// The public functions that run a test double's `trigger` helper to turn its own loop
/// rather than to simulate an event, exempt from `trigger_` naming.
const LOOP_TURNS: &[&str] = &["pump_tasks"];

/// The foundation's sources.
const FOUNDATION: &str = "packages/foundation/src";

/// The foundation's spine: its module file and the directory of its modules.
const SPINE: [&str; 2] = [
  "packages/foundation/src/spine.rs",
  "packages/foundation/src/spine",
];

/// Directories never searched for sources, the compile-fail programs among them.
const SKIPPED: &[&str] = &[
  ".git",
  "compile_fail",
  "dist",
  "node_modules",
  "target",
  "vendor",
];

/// Checks every Rust source and Markdown document in the workspace, and every public name
/// the API snapshots list, against the house rules.
pub(crate) fn check(root: &Path) -> Result<(), String> {
  let mut findings = Vec::new();
  let mut files = Vec::new();

  collect(root, &mut files);
  files.sort();

  for path in files {
    let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;

    match path.extension().and_then(|extension| extension.to_str()) {
      Some("md") => findings.extend(spaced_em_dashes(&relative, &text, false)),
      Some("rs") => {
        findings.extend(spaced_em_dashes(&relative, &text, true));

        let file =
          syn::parse_file(&text).map_err(|error| format!("{}: {error}", relative.display()))?;
        let mut rules = Rules {
          foundation: relative.starts_with(FOUNDATION),
          spine: is_spine_source(&relative),
          path: &relative,
          findings: &mut findings,
        };

        rules.visit_file(&file);
      }
      _ => {}
    }
  }

  findings.extend(banned_names(root)?);

  if findings.is_empty() {
    Ok(())
  }
  else {
    Err(findings.join("\n"))
  }
}

/// Every Rust source and Markdown document under `directory`.
fn collect(directory: &Path, files: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(directory)
  else {
    return;
  };

  for entry in entries.flatten() {
    let path = entry.path();
    let name = entry.file_name();

    if path.is_dir() {
      if !SKIPPED.iter().any(|skipped| name == *skipped) {
        collect(&path, files);
      }
    }
    else if path
      .extension()
      .is_some_and(|extension| extension == "rs" || extension == "md")
    {
      files.push(path);
    }
  }
}

/// Each em-dash with a space beside it in `text`; in a source, only those in comments.
fn spaced_em_dashes(path: &Path, text: &str, comments_only: bool) -> Vec<String> {
  text
    .lines()
    .enumerate()
    .filter(|(_, line)| {
      let prose = if comments_only {
        line.find("//").map_or("", |start| &line[start..])
      }
      else {
        line
      };

      prose.contains(" —") || prose.contains("— ")
    })
    .map(|(index, _)| {
      format!(
        "{}:{}: an em-dash has a space beside it",
        path.display(),
        index + 1
      )
    })
    .collect()
}

/// The syntactic house rules, checked over one file.
struct Rules<'a> {
  /// Whether the file is the foundation's, which keeps nothing crate-private.
  foundation: bool,
  /// Whether the file is the foundation's spine, which names nothing of its crate
  /// outside itself.
  spine: bool,
  /// The file's path from the workspace root.
  path: &'a Path,
  /// Where the rules report what they find.
  findings: &'a mut Vec<String>,
}

impl Rules<'_> {
  fn report(&mut self, span: Span, message: &str) {
    let line = span.start().line;

    self
      .findings
      .push(format!("{}:{line}: {message}", self.path.display()));
  }

  /// Reports a type documenting some of its fields or variants but not all: every one,
  /// none, or exactly the public ones pass. Each member is whether it is documented and
  /// whether it is public.
  fn check_member_docs(&mut self, name: &Ident, members: &[(bool, bool)]) {
    let documented = members.iter().filter(|(documented, _)| *documented).count();

    if documented == 0
      || documented == members.len()
      || members
        .iter()
        .all(|(documented, public)| documented == public)
    {
      return;
    }

    self.report(
      name.span(),
      &format!(
        "`{name}` documents {documented} of its {} members: all, none, or exactly the \
         public ones",
        members.len()
      ),
    );
  }

  /// Reports a public function that simulates an event through the `trigger` helper
  /// without a `trigger_` name.
  fn check_trigger_name(&mut self, item: &ItemFn) {
    let name = item.sig.ident.to_string();

    if !matches!(item.vis, Visibility::Public(_))
      || name.starts_with("trigger_")
      || LOOP_TURNS.contains(&name.as_str())
    {
      return;
    }

    let mut calls = TriggerCalls(false);

    calls.visit_block(&item.block);

    if calls.0 {
      self.report(
        item.sig.ident.span(),
        &format!("`{name}` simulates an event through `trigger`: name it `trigger_<event>`"),
      );
    }
  }

  /// Reports a path in the spine naming something of its crate outside the spine:
  /// `crate` followed by anything but `spine`, `pub(crate)` among them.
  fn check_spine_path(&mut self, span: Span, segments: &[String]) {
    if !self.spine || segments.first().is_none_or(|first| first != "crate") {
      return;
    }

    if segments.get(1).is_some_and(|second| second == "spine") {
      return;
    }

    self.report(
      span,
      &format!(
        "`{}` names the foundation outside the spine, which names nothing beyond itself",
        segments.join("::")
      ),
    );
  }
}

impl<'ast> Visit<'ast> for Rules<'_> {
  fn visit_visibility(&mut self, visibility: &'ast Visibility) {
    if self.foundation
      && !self.spine
      && let Visibility::Restricted(restricted) = visibility
      && restricted.path.is_ident("crate")
    {
      self.report(
        restricted
          .in_token
          .map_or_else(|| restricted.pub_token.span, |token| token.span),
        "`pub(crate)` in the foundation: nothing on the path a built-in takes is crate-private",
      );
    }

    visit::visit_visibility(self, visibility);
  }

  fn visit_path(&mut self, path: &'ast SynPath) {
    let segments: Vec<String> = path
      .segments
      .iter()
      .map(|segment| segment.ident.to_string())
      .collect();

    if let Some(first) = path.segments.first() {
      self.check_spine_path(first.ident.span(), &segments);
    }

    visit::visit_path(self, path);
  }

  fn visit_use_path(&mut self, item: &'ast UsePath) {
    if item.ident == "crate" {
      for second in use_heads(&item.tree) {
        self.check_spine_path(item.ident.span(), &["crate".to_owned(), second]);
      }
    }

    visit::visit_use_path(self, item);
  }

  fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
    let members: Vec<(bool, bool)> = item
      .fields
      .iter()
      .map(|field| {
        (
          field.attrs.iter().any(is_doc),
          matches!(field.vis, Visibility::Public(_)),
        )
      })
      .collect();

    self.check_member_docs(&item.ident, &members);
    visit::visit_item_struct(self, item);
  }

  fn visit_item_enum(&mut self, item: &'ast ItemEnum) {
    let members: Vec<(bool, bool)> = item
      .variants
      .iter()
      .map(|variant| (variant.attrs.iter().any(is_doc), true))
      .collect();

    self.check_member_docs(&item.ident, &members);
    visit::visit_item_enum(self, item);
  }

  fn visit_item_fn(&mut self, item: &'ast ItemFn) {
    self.check_trigger_name(item);
    visit::visit_item_fn(self, item);
  }

  fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
    if let Some(method) = property_set_by(call)
      && let Expr::MethodCall(receiver) = &*call.receiver
      && let Some(children) = children_set_by(receiver)
    {
      self.report(
        call.method.span(),
        &format!("`{method}` follows `{children}`: a chain ends with the children"),
      );
    }

    visit::visit_expr_method_call(self, call);
  }
}

/// Whether a body calls a test double's `trigger` helper.
struct TriggerCalls(bool);

impl<'ast> Visit<'ast> for TriggerCalls {
  fn visit_expr_call(&mut self, call: &'ast ExprCall) {
    if let Expr::Path(function) = &*call.func
      && function.path.is_ident("trigger")
    {
      self.0 = true;
    }

    visit::visit_expr_call(self, call);
  }
}

/// The child setter a builder call gives a view its content through, written out or
/// applied (`.apply(child, Container::set_child)`).
fn children_set_by(call: &ExprMethodCall) -> Option<String> {
  let method = call.method.to_string();

  if CHILD_SETTERS.contains(&method.as_str()) {
    return Some(method);
  }

  applied_setter(call).filter(|setter| CHILD_SETTERS.contains(&setter.as_str()))
}

/// The property, handler or applied option a builder call sets, where it sets no
/// content.
fn property_set_by(call: &ExprMethodCall) -> Option<String> {
  if children_set_by(call).is_some() {
    return None;
  }

  let method = call.method.to_string();

  (method == "apply"
    || PROPERTY_PREFIXES
      .iter()
      .any(|prefix| method.starts_with(prefix)))
  .then_some(method)
}

/// The setter an `apply` call applies: the last segment of the path it names, or the
/// method a closure it is given calls on the builder.
fn applied_setter(call: &ExprMethodCall) -> Option<String> {
  if call.method != "apply" {
    return None;
  }

  match call.args.last()? {
    Expr::Path(setter) => setter
      .path
      .segments
      .last()
      .map(|segment| segment.ident.to_string()),
    Expr::Closure(closure) => called_method(&closure.body),
    _ => None,
  }
}

/// The method an expression calls last: its own, or its block's final expression's.
fn called_method(body: &Expr) -> Option<String> {
  match body {
    Expr::MethodCall(call) => Some(call.method.to_string()),
    Expr::Block(block) => match block.block.stmts.last()? {
      Stmt::Expr(tail, None) => called_method(tail),
      _ => None,
    },
    Expr::Paren(inner) => called_method(&inner.expr),
    _ => None,
  }
}

/// Whether `path`, relative to the workspace, is the foundation's spine.
fn is_spine_source(path: &Path) -> bool {
  path == Path::new(SPINE[0]) || path.starts_with(SPINE[1])
}

/// The first name of every path a use tree imports.
fn use_heads(tree: &UseTree) -> Vec<String> {
  match tree {
    UseTree::Path(path) => vec![path.ident.to_string()],
    UseTree::Name(name) => vec![name.ident.to_string()],
    UseTree::Rename(rename) => vec![rename.ident.to_string()],
    UseTree::Glob(_) => vec!["*".to_owned()],
    UseTree::Group(group) => group.items.iter().flat_map(use_heads).collect(),
  }
}

/// Whether `attribute` is documentation, as a `///` comment is.
fn is_doc(attribute: &Attribute) -> bool {
  attribute.path().is_ident("doc")
}

/// Reports every public name the API snapshots list made of a banned word: the
/// framework's whole public surface, re-exports included.
fn banned_names(root: &Path) -> Result<Vec<String>, String> {
  let directory = root.join("api");
  let mut snapshots: Vec<PathBuf> = fs::read_dir(&directory)
    .map_err(|error| format!("{}: {error}", directory.display()))?
    .flatten()
    .map(|entry| entry.path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
    .collect();
  let mut findings = Vec::new();

  snapshots.sort();

  for path in snapshots {
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;

    findings.extend(banned_in_snapshot(
      path.strip_prefix(root).unwrap_or(&path),
      &text,
    ));
  }

  Ok(findings)
}

/// Reports each name made of a banned word among those the snapshot `text` lists, at
/// its line.
fn banned_in_snapshot(path: &Path, text: &str) -> Vec<String> {
  text
    .lines()
    .enumerate()
    .filter_map(|(index, line)| {
      let name = listed_name(line)?;
      let word = words(name).find(|word| BANNED_WORDS.contains(&word.as_str()))?;

      Some(format!(
        "{}:{}: `{name}` uses `{word}`, which the framework's vocabulary replaces",
        path.display(),
        index + 1
      ))
    })
    .collect()
}

/// The name of the item a snapshot line lists, the last segment of its path; `None` for
/// a line listing no item of its own, as an `impl` does.
fn listed_name(line: &str) -> Option<&str> {
  let mut rest = line.strip_prefix("pub ")?;

  while let Some((keyword, after)) = rest.split_once(' ')
    && ITEM_KEYWORDS.contains(&keyword)
  {
    rest = after;
  }

  let mut depth = 0usize;
  let mut start = 0;
  let mut end = rest.len();

  for (index, character) in rest.char_indices() {
    match character {
      '<' => depth += 1,
      '>' => depth = depth.saturating_sub(1),
      '(' | ' ' if depth == 0 => {
        end = index;

        break;
      }
      ':' if depth == 0 && rest[index + 1..].starts_with(' ') => {
        end = index;

        break;
      }
      ':' if depth == 0 && rest[index + 1..].starts_with(':') => start = index + 2,
      _ => {}
    }
  }

  let name = &rest[start..end];
  let name = name.find('<').map_or(name, |generics| &name[..generics]);

  (!name.is_empty()).then_some(name)
}

/// The lowercase words of a snake-case or camel-case name.
fn words(name: &str) -> impl Iterator<Item = String> {
  let mut words = Vec::new();
  let mut word = String::new();

  for character in name.chars() {
    if character == '_' {
      words.push(std::mem::take(&mut word));
    }
    else if character.is_uppercase() && !word.is_empty() {
      words.push(std::mem::take(&mut word));
      word.extend(character.to_lowercase());
    }
    else {
      word.extend(character.to_lowercase());
    }
  }

  words.push(word);
  words.into_iter().filter(|word| !word.is_empty())
}

#[cfg(test)]
mod tests {
  use std::path::Path;

  use syn::visit::Visit as _;

  use super::{Rules, banned_in_snapshot, listed_name, spaced_em_dashes};

  fn findings(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let mut findings = Vec::new();

    Rules {
      foundation: false,
      spine: false,
      path: Path::new("test.rs"),
      findings: &mut findings,
    }
    .visit_file(&file);

    findings
  }

  #[test]
  fn a_property_after_the_children_is_reported() {
    assert_eq!(
      findings("fn app() { Row::new().set_children(()).set_spacing(8.); }"),
      ["test.rs:1: `set_spacing` follows `set_children`: a chain ends with the children"]
    );
  }

  #[test]
  fn a_chain_ending_with_its_children_passes() {
    assert_eq!(
      findings("fn app() { Row::new().set_spacing(8.).set_children(()); }"),
      Vec::<String>::new()
    );
  }

  #[test]
  fn an_applied_option_after_the_children_is_reported() {
    assert_eq!(
      findings("fn app() { Row::new().set_children(()).apply(spacing, Row::set_spacing); }"),
      ["test.rs:1: `apply` follows `set_children`: a chain ends with the children"]
    );
  }

  #[test]
  fn an_applied_child_is_a_child_setter() {
    assert_eq!(
      findings("fn app() { Container::new().apply(child, Container::set_child).set_padding(4.); }"),
      ["test.rs:1: `set_padding` follows `set_child`: a chain ends with the children"]
    );
  }

  #[test]
  fn a_closure_applying_a_child_is_a_child_setter() {
    assert_eq!(
      findings(
        "fn app() { Column::new().add_child(a).apply(b, |column, b| column.add_child(b)); \
         Column::new().apply(b, |column, b| column.add_child(b)).set_spacing(4.); \
         Column::new().apply(b, |column, b| { column.add_child(b) }).set_spacing(4.); }"
      ),
      [
        "test.rs:1: `set_spacing` follows `add_child`: a chain ends with the children",
        "test.rs:1: `set_spacing` follows `add_child`: a chain ends with the children",
      ]
    );
  }

  #[test]
  fn a_chain_ending_with_an_applied_child_passes() {
    assert_eq!(
      findings(
        "fn app() { Container::new().apply(padding, Container::set_padding).apply(child, Container::set_child); }"
      ),
      Vec::<String>::new()
    );
  }

  #[test]
  fn a_property_after_a_loader_or_a_face_is_reported() {
    assert_eq!(
      findings(
        "fn app() { AsyncView::new().set_child_loader(load).set_key(1); \
         BasicPicker::new().set_face(face).handle_choose(choose); }"
      ),
      [
        "test.rs:1: `set_key` follows `set_child_loader`: a chain ends with the children",
        "test.rs:1: `handle_choose` follows `set_face`: a chain ends with the children",
      ]
    );
  }

  #[test]
  fn a_listed_name_made_of_a_banned_word_is_reported() {
    assert_eq!(
      banned_in_snapshot(
        Path::new("api/test.txt"),
        "pub skene_foundation::Spacing::row_gap: f32\n\
         impl core::fmt::Display for skene_foundation::Color\n\
         pub fn skene_foundation::Run<P: skene_foundation::Realization>::set_display(self) -> Self\n\
         pub use skene_web::Widget\n"
      ),
      [
        "api/test.txt:1: `row_gap` uses `gap`, which the framework's vocabulary replaces",
        "api/test.txt:3: `set_display` uses `display`, which the framework's vocabulary replaces",
        "api/test.txt:4: `Widget` uses `widget`, which the framework's vocabulary replaces",
      ]
    );
  }

  #[test]
  fn a_listed_name_is_its_items_own() {
    for (line, name) in [
      (
        "pub const fn skene_foundation::Expr<skene_foundation::Length>::font_size(f32) -> Self",
        Some("font_size"),
      ),
      (
        "pub skene_foundation::RunFacts::placement: skene_foundation::RunPlacement",
        Some("placement"),
      ),
      (
        "pub struct skene_foundation::Run<P: skene_foundation::Realization>",
        Some("Run"),
      ),
      (
        "pub type skene_web::Resource<T> = skene_foundation::web::Resource<skene_web::Web, T>",
        Some("Resource"),
      ),
      (
        "pub skene_foundation::GridTrack::Between(T, T)",
        Some("Between"),
      ),
      ("impl core::fmt::Display for skene_foundation::Color", None),
    ] {
      assert_eq!(listed_name(line), name, "{line}");
    }
  }

  #[test]
  fn a_type_documenting_some_members_is_reported() {
    assert_eq!(
      findings("struct Half { /// Documented.\n a: u8, b: u8 }").len(),
      1
    );
    assert_eq!(findings("enum Half { /// Documented.\n A, B }").len(), 1);
  }

  #[test]
  fn a_type_documenting_all_none_or_its_public_members_passes() {
    assert_eq!(
      findings("struct All { /// A.\n a: u8, /// B.\n b: u8 }"),
      Vec::<String>::new()
    );
    assert_eq!(
      findings("struct None { a: u8, b: u8 }"),
      Vec::<String>::new()
    );
    assert_eq!(
      findings("pub struct Public { /// A.\n pub a: u8, b: u8 }"),
      Vec::<String>::new()
    );
  }

  #[test]
  fn a_public_function_triggering_an_event_under_another_name_is_reported() {
    assert_eq!(
      findings("pub fn set_viewport() { trigger(|| ()); }"),
      [
        "test.rs:1: `set_viewport` simulates an event through `trigger`: name it \
         `trigger_<event>`"
      ]
    );
  }

  #[test]
  fn a_triggered_event_its_name_a_loop_turn_or_a_private_helper_passes() {
    assert_eq!(
      findings(
        "pub fn trigger_resize() { trigger(|| ()); } pub fn pump_tasks() -> bool { \
         trigger(pump) } fn resize() { trigger(|| ()); }"
      ),
      Vec::<String>::new()
    );
  }

  /// What the rules find in `source` read as the spine's.
  fn spine_findings(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let mut findings = Vec::new();

    Rules {
      foundation: true,
      spine: true,
      path: Path::new("test.rs"),
      findings: &mut findings,
    }
    .visit_file(&file);

    findings
  }

  /// What the rules find in `source` read as the foundation's outside the spine.
  fn foundation_findings(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let mut findings = Vec::new();

    Rules {
      foundation: true,
      spine: false,
      path: Path::new("test.rs"),
      findings: &mut findings,
    }
    .visit_file(&file);

    findings
  }

  #[test]
  fn a_crate_private_item_in_the_foundation_is_reported() {
    assert_eq!(foundation_findings("pub(crate) fn helper() {}").len(), 1);
    assert_eq!(
      foundation_findings("fn helper() {} pub fn api() {}"),
      Vec::<String>::new()
    );
  }

  #[test]
  fn the_spine_naming_the_crate_outside_itself_is_reported() {
    assert_eq!(
      spine_findings("use crate::{spine::Scene, container::Container};").len(),
      1
    );
    assert_eq!(
      spine_findings("fn build() { crate::theme::define(); }").len(),
      1
    );
    assert_eq!(spine_findings("pub(crate) fn helper() {}").len(), 1);
  }

  #[test]
  fn the_spine_naming_itself_passes() {
    assert_eq!(
      spine_findings(
        "use crate::spine::{scene::Scene, view::View}; pub(in crate::spine) fn helper() { \
         crate::spine::runtime::end_turn(); }"
      ),
      Vec::<String>::new()
    );
  }

  #[test]
  fn a_spaced_em_dash_is_reported_in_a_comment_alone() {
    let comment = ["/", "/ a comment — spaced"].concat();
    let source = format!("let title = \"Home — skene\";\n{comment}\n");

    assert_eq!(
      spaced_em_dashes(Path::new("test.rs"), &source, true),
      ["test.rs:2: an em-dash has a space beside it"]
    );
  }
}
