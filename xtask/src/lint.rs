//! The house rules no compiler checks: unspaced em-dashes, children last, the
//! framework's own vocabulary, fields and variants documented all or none, a test
//! double's simulated events named `trigger_`, and the spine naming nothing of the
//! foundation outside itself.

use std::{
  fs,
  path::{Component, Path, PathBuf},
};

use proc_macro2::Span;
use syn::{
  Attribute,
  Expr,
  ExprCall,
  ExprMethodCall,
  Fields,
  Ident,
  ImplItemFn,
  ItemEnum,
  ItemFn,
  ItemMod,
  ItemStruct,
  ItemTrait,
  ItemType,
  Path as SynPath,
  UsePath,
  UseTree,
  Visibility,
  visit::{self, Visit},
};

/// The methods that give a view its content, which a builder chain ends with.
const CHILD_SETTERS: &[&str] = &[
  "add_child",
  "derive_child",
  "derive_children",
  "derive_entries",
  "set_child",
  "set_child_builder",
  "set_children",
];

/// The prefixes of a builder's property and handler methods.
const PROPERTY_PREFIXES: &[&str] = &["derive_", "handle_", "set_"];

/// Words the public surface never uses, the framework's own vocabulary standing in.
const BANNED_WORDS: &[&str] = &["backend", "gap", "justify", "widget"];

/// The public functions that run a test double's `trigger` helper to turn its own loop
/// rather than to simulate an event, exempt from `trigger_` naming.
const LOOP_TURNS: &[&str] = &["pump_tasks"];

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

/// Checks every Rust source and Markdown document in the workspace against the house rules.
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
          public_surface: is_framework_source(&relative),
          spine: is_spine_source(&relative),
          path: &relative,
          findings: &mut findings,
        };

        rules.visit_file(&file);
      }
      _ => {}
    }
  }

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
  /// Whether the file is a framework crate's source, whose public names the vocabulary
  /// binds.
  public_surface: bool,
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

  /// Reports a public name made of a banned word.
  fn check_name(&mut self, visibility: &Visibility, ident: &Ident) {
    if !self.public_surface || !matches!(visibility, Visibility::Public(_)) {
      return;
    }

    let name = ident.to_string();

    if let Some(word) = words(&name).find(|word| BANNED_WORDS.contains(&word.as_str())) {
      self.report(
        ident.span(),
        &format!("`{name}` uses `{word}`, which the framework's vocabulary replaces"),
      );
    }
  }
}

impl<'ast> Visit<'ast> for Rules<'_> {
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

  fn visit_item_mod(&mut self, item: &'ast ItemMod) {
    let public_surface = self.public_surface;

    if item.attrs.iter().any(is_test_configuration) {
      self.public_surface = false;
    }

    self.check_name(&item.vis, &item.ident);
    visit::visit_item_mod(self, item);
    self.public_surface = public_surface;
  }

  fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
    self.check_name(&item.vis, &item.ident);

    if let Fields::Named(fields) = &item.fields {
      for field in &fields.named {
        if let Some(ident) = &field.ident {
          self.check_name(&field.vis, ident);
        }
      }
    }

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
    self.check_name(&item.vis, &item.ident);

    for variant in &item.variants {
      self.check_name(&item.vis, &variant.ident);
    }

    let members: Vec<(bool, bool)> = item
      .variants
      .iter()
      .map(|variant| (variant.attrs.iter().any(is_doc), true))
      .collect();

    self.check_member_docs(&item.ident, &members);
    visit::visit_item_enum(self, item);
  }

  fn visit_item_fn(&mut self, item: &'ast ItemFn) {
    self.check_name(&item.vis, &item.sig.ident);
    self.check_trigger_name(item);
    visit::visit_item_fn(self, item);
  }

  fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
    self.check_name(&item.vis, &item.sig.ident);
    visit::visit_impl_item_fn(self, item);
  }

  fn visit_item_trait(&mut self, item: &'ast ItemTrait) {
    self.check_name(&item.vis, &item.ident);
    visit::visit_item_trait(self, item);
  }

  fn visit_item_type(&mut self, item: &'ast ItemType) {
    self.check_name(&item.vis, &item.ident);
    visit::visit_item_type(self, item);
  }

  fn visit_expr_method_call(&mut self, call: &'ast ExprMethodCall) {
    let method = call.method.to_string();

    if !CHILD_SETTERS.contains(&method.as_str())
      && PROPERTY_PREFIXES
        .iter()
        .any(|prefix| method.starts_with(prefix))
      && let Expr::MethodCall(receiver) = &*call.receiver
      && CHILD_SETTERS.contains(&receiver.method.to_string().as_str())
    {
      self.report(
        call.method.span(),
        &format!(
          "`{method}` follows `{}`: a chain ends with the children",
          receiver.method
        ),
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

/// Whether `path`, relative to the workspace, is a framework crate's source.
fn is_framework_source(path: &Path) -> bool {
  let mut components = path.components().map(Component::as_os_str);

  components.next() == Some("packages".as_ref())
    && components.next().is_some()
    && components.next() == Some("src".as_ref())
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

fn is_test_configuration(attribute: &Attribute) -> bool {
  attribute.path().is_ident("cfg")
    && attribute
      .parse_args::<Ident>()
      .is_ok_and(|argument| argument == "test")
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

  use super::{Rules, spaced_em_dashes};

  fn findings(source: &str, public_surface: bool) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let mut findings = Vec::new();

    Rules {
      public_surface,
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
      findings(
        "fn app() { Row::new().set_children(()).set_spacing(8.); }",
        false
      ),
      ["test.rs:1: `set_spacing` follows `set_children`: a chain ends with the children"]
    );
  }

  #[test]
  fn a_chain_ending_with_its_children_passes() {
    assert!(
      findings(
        "fn app() { Row::new().set_spacing(8.).set_children(()); }",
        false
      )
      .is_empty()
    );
  }

  #[test]
  fn a_public_name_made_of_a_banned_word_is_reported() {
    assert_eq!(
      findings("pub struct Shape { pub row_gap: f32 }", true),
      ["test.rs:1: `row_gap` uses `gap`, which the framework's vocabulary replaces"]
    );
  }

  #[test]
  fn a_private_or_test_name_may_use_any_word() {
    assert!(
      findings(
        "struct Widget; #[cfg(test)] mod tests { pub fn widget() {} }",
        true
      )
      .is_empty()
    );
  }

  #[test]
  fn a_type_documenting_some_members_is_reported() {
    assert_eq!(
      findings("struct Half { /// Documented.\n a: u8, b: u8 }", false).len(),
      1
    );
    assert_eq!(
      findings("enum Half { /// Documented.\n A, B }", false).len(),
      1
    );
  }

  #[test]
  fn a_type_documenting_all_none_or_its_public_members_passes() {
    assert!(findings("struct All { /// A.\n a: u8, /// B.\n b: u8 }", false).is_empty());
    assert!(findings("struct None { a: u8, b: u8 }", false).is_empty());
    assert!(findings("pub struct Public { /// A.\n pub a: u8, b: u8 }", false).is_empty());
  }

  #[test]
  fn a_public_function_triggering_an_event_under_another_name_is_reported() {
    assert_eq!(
      findings("pub fn set_viewport() { trigger(|| ()); }", false),
      [
        "test.rs:1: `set_viewport` simulates an event through `trigger`: name it \
         `trigger_<event>`"
      ]
    );
  }

  #[test]
  fn a_triggered_event_its_name_a_loop_turn_or_a_private_helper_passes() {
    assert!(
      findings(
        "pub fn trigger_resize() { trigger(|| ()); } pub fn pump_tasks() -> bool { \
         trigger(pump) } fn resize() { trigger(|| ()); }",
        false
      )
      .is_empty()
    );
  }

  /// What the rules find in `source` read as the spine's.
  fn spine_findings(source: &str) -> Vec<String> {
    let file = syn::parse_file(source).expect("the source parses");
    let mut findings = Vec::new();

    Rules {
      public_surface: true,
      spine: true,
      path: Path::new("test.rs"),
      findings: &mut findings,
    }
    .visit_file(&file);

    findings
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
    assert!(
      spine_findings(
        "use crate::spine::{scene::Scene, view::View}; pub(in crate::spine) fn helper() { \
         crate::spine::runtime::end_turn(); }"
      )
      .is_empty()
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
