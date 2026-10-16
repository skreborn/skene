//! # skene-gui-macros
//!
//! `#[skene_gui::main]` and `#[skene_gui::test]`, written over `proc_macro` alone.

use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree};

/// Marks `fn app() -> impl View` as the app's root and emits the entry the build's platform
/// calls.
///
/// It takes no arguments: the build names the platform (`--cfg skene_platform="linux"`), and
/// the attribute emits that platform's entry under its `cfg`:
///
/// | platform | entry |
/// |---|---|
/// | Linux | the binary's `main`, which opens the window Skene draws the root into |
///
/// The application's id is `SKENE_APP_ID` where the build sets it, else
/// `dev.skene.<crate name>`. A crate with two roots fails to compile: two `main`s.
#[proc_macro_attribute]
pub fn main(attribute: TokenStream, item: TokenStream) -> TokenStream {
  if let Some(token) = attribute.into_iter().next() {
    return error(
      token.span(),
      "`#[skene_gui::main]` takes no arguments: the build names the platform",
    );
  }

  let tokens: Vec<TokenTree> = item.clone().into_iter().collect();
  let Some(position) = tokens.iter().position(|token| is(token, "fn")) else {
    return error(
      Span::call_site(),
      "`#[skene_gui::main]` goes on the app's root: `fn app() -> impl View`",
    );
  };

  if let Some(token) = tokens[..position]
    .iter()
    .find(|token| is(token, "async") || is(token, "unsafe"))
  {
    return error(token.span(), "the app's root is a plain function");
  }

  let Some(TokenTree::Ident(name)) = tokens.get(position + 1) else {
    return error(Span::call_site(), "expected the root function's name");
  };

  match tokens.get(position + 2) {
    Some(TokenTree::Group(parameters)) if parameters.delimiter() == Delimiter::Parenthesis => {
      if let Some(token) = parameters.stream().into_iter().next() {
        return error(
          token.span(),
          "the app's root takes no parameters: build what it needs inside it",
        );
      }
    }
    Some(token) => return error(token.span(), "the app's root takes no generic parameters"),
    None => return error(name.span(), "expected the root function's parameters"),
  }

  let returns = matches!(
    (tokens.get(position + 3), tokens.get(position + 4)),
    (Some(TokenTree::Punct(dash)), Some(TokenTree::Punct(arrow)))
      if dash.as_char() == '-' && arrow.as_char() == '>'
  );

  if !returns {
    return error(
      name.span(),
      "the app's root returns its view: `-> impl View`",
    );
  }

  let entry = ENTRY.replace("ROOT", &name.to_string());
  let mut output = item;

  output.extend(
    entry
      .parse::<TokenStream>()
      .expect("the entry is valid Rust"),
  );

  output
}

/// The entries, `ROOT` standing for the root function's name.
const ENTRY: &str = r#"
const _: () = ::skene_gui::__private::assert_app(ROOT);

/// The Linux entry: the window Skene draws the app's root into.
#[cfg(skene_platform = "linux")]
fn main() -> ::std::process::ExitCode {
  ::skene_gui::__private::start(
    match ::core::option_env!("SKENE_APP_ID") {
      ::core::option::Option::Some(id) => id,
      ::core::option::Option::None => ::core::concat!("dev.skene.", ::core::env!("CARGO_CRATE_NAME")),
    },
    ROOT,
  )
}
"#;

/// Runs a test's body under an owner of its own and a reset environment, the owner disposed
/// when it returns.
///
/// It expands to a plain `#[test]`, so `cargo test` and `cargo nextest` both run it; cargo runs
/// each test binary through the display runner the app's configuration names, which gives each
/// test process a private display, session bus and accessibility bus.
#[proc_macro_attribute]
pub fn test(attribute: TokenStream, item: TokenStream) -> TokenStream {
  if let Some(token) = attribute.into_iter().next() {
    return error(token.span(), "`#[skene_gui::test]` takes no arguments");
  }

  let tokens: Vec<TokenTree> = item.into_iter().collect();
  let Some(position) = tokens.iter().position(|token| is(token, "fn")) else {
    return error(
      Span::call_site(),
      "`#[skene_gui::test]` goes on a test function",
    );
  };
  let Some(TokenTree::Ident(name)) = tokens.get(position + 1) else {
    return error(Span::call_site(), "expected the test's name");
  };
  let Some(TokenTree::Group(body)) = tokens.last() else {
    return error(name.span(), "expected the test's body");
  };
  let mut output: TokenStream = tokens[..position].iter().cloned().collect();
  let mut inner: TokenStream = "fn body()".parse().expect("valid Rust");

  inner.extend([TokenTree::Group(body.clone())]);
  inner.extend(
    format!("::skene_gui::__private::test(::core::stringify!({name}), body)")
      .parse::<TokenStream>()
      .expect("valid Rust"),
  );

  output.extend(
    "#[::core::prelude::v1::test] fn"
      .parse::<TokenStream>()
      .expect("valid Rust"),
  );
  output.extend([
    TokenTree::Ident(name.clone()),
    TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
    TokenTree::Group(Group::new(Delimiter::Brace, inner)),
  ]);

  output
}

/// Whether `token` is the identifier `word`.
fn is(token: &TokenTree, word: &str) -> bool {
  matches!(token, TokenTree::Ident(ident) if ident.to_string() == word)
}

/// `compile_error!(message)` at `span`.
fn error(span: Span, message: &str) -> TokenStream {
  let mut literal = Literal::string(message);
  let mut bang = Punct::new('!', Spacing::Alone);
  let mut semicolon = Punct::new(';', Spacing::Alone);

  literal.set_span(span);
  bang.set_span(span);
  semicolon.set_span(span);

  let mut group = Group::new(
    Delimiter::Parenthesis,
    TokenStream::from(TokenTree::Literal(literal)),
  );

  group.set_span(span);

  [
    TokenTree::Ident(Ident::new("compile_error", span)),
    TokenTree::Punct(bang),
    TokenTree::Group(group),
    TokenTree::Punct(semicolon),
  ]
  .into_iter()
  .collect()
}
