//! The document a page is served as: one template for the server leaf's pages and the
//! CLI's client-rendered shell.

use crate::hydration::STATE_SCRIPT_ID;

/// The dev loop's script: it keeps the state snapshot in the page's own storage across a
/// reload—the origin private file system on the dev server's word, session storage as
/// the best effort on a reload the reader made—and hands it back at boot through
/// `skene_state()`.
const DEV_SCRIPT: &str = include_str!("dev.js");

/// The document a page is served as around its content.
///
/// The doctype; the root, in the page's language; a head holding the character set, the
/// viewport and the client bundle's preloads ahead of the page's own head and styles;
/// and a body ending in the hydration state and the script booting the bundle. A page in
/// the dev loop carries the dev marker and the dev server's script besides, and every
/// inline script carries the nonce.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Document {
  /// The page's language, a BCP 47 tag written as its root's `lang`.
  pub language: Option<String>,
  /// The client bundle the page boots: the name wasm-bindgen gave its files, served
  /// from the site root as `/{bundle}.js` and `/{bundle}_bg.wasm`.
  pub bundle: Option<String>,
  /// The nonce the page's inline scripts carry, as the response's Content Security
  /// Policy names it.
  pub nonce: Option<String>,
  /// Whether the page joins the dev loop of `skene run web`.
  pub dev: bool,
}

impl Document {
  /// The document up to its body's content, `head` and `styles` closing its head.
  #[must_use]
  pub fn opening(&self, head: &str, styles: &str) -> String {
    let mut out = String::from("<!DOCTYPE html><html");

    if let Some(language) = &self.language {
      out.push_str(" lang=\"");
      escape_attr(language, &mut out);
      out.push('"');
    }

    out.push_str(
      "><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
       content=\"width=device-width, initial-scale=1\">",
    );

    if let Some(bundle) = &self.bundle {
      out.push_str("<link rel=\"modulepreload\" href=\"/");
      escape_attr(bundle, &mut out);
      out.push_str(".js\"><link rel=\"preload\" href=\"/");
      escape_attr(bundle, &mut out);
      out.push_str("_bg.wasm\" as=\"fetch\" type=\"application/wasm\" crossorigin>");
    }

    if self.dev {
      out.push_str("<meta name=\"skene-dev\">");
      push_script(&mut out, self.nonce.as_deref(), false, DEV_SCRIPT);
    }

    out.push_str(head);
    out.push_str(styles);
    out.push_str("</head><body>");

    out
  }

  /// The document after its body's content: the hydration `state` where there is one,
  /// the script booting the bundle, and the end of the body and the document.
  #[must_use]
  pub fn closing(&self, state: Option<&str>) -> String {
    let mut out = String::new();

    if let Some(state) = state {
      out.push_str(&state_script(state));
    }

    if let Some(bundle) = &self.bundle {
      let boot = format!(
        "import init from {};init();",
        script_string(&format!("/{bundle}.js"))
      );

      push_script(&mut out, self.nonce.as_deref(), true, &boot);
    }

    out.push_str("</body></html>");

    out
  }
}

/// Appends an inline script running `body`, a module when `module` is set, carrying
/// `nonce` where there is one; `body` must hold no `</`.
pub(crate) fn push_script(out: &mut String, nonce: Option<&str>, module: bool, body: &str) {
  out.push_str("<script");

  if module {
    out.push_str(" type=\"module\"");
  }

  if let Some(nonce) = nonce {
    out.push_str(" nonce=\"");
    escape_attr(nonce, out);
    out.push('"');
  }

  out.push('>');
  out.push_str(body);
  out.push_str("</script>");
}

/// `text` as a JavaScript string literal that closes no element it sits in.
fn script_string(text: &str) -> String {
  serde_json::to_string(text)
    .expect("a string serializes")
    .replace('<', "\\u003c")
}

/// The script element carrying the hydration `state`, its markup escaped so it closes
/// nothing: what a page of an app's own embeds for the client to read.
pub(crate) fn state_script(state: &str) -> String {
  format!(
    "<script id=\"{STATE_SCRIPT_ID}\" type=\"application/json\">{}</script>",
    state.replace('<', "\\u003c"),
  )
}

/// Appends `text` with the characters HTML text content reserves escaped.
#[cfg_attr(skene_leaf = "host", expect(dead_code))]
pub(crate) fn escape_text(text: &str, out: &mut String) {
  for character in text.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      other => out.push(other),
    }
  }
}

/// Appends `value` with the characters an HTML attribute value reserves escaped.
pub(crate) fn escape_attr(value: &str, out: &mut String) {
  for character in value.chars() {
    match character {
      '&' => out.push_str("&amp;"),
      '<' => out.push_str("&lt;"),
      '>' => out.push_str("&gt;"),
      '"' => out.push_str("&quot;"),
      other => out.push(other),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Document;

  /// A document booting nothing is the bare template around its content.
  #[test]
  fn a_bare_document_boots_nothing() {
    let document = Document::default();

    assert_eq!(
      document.opening("<title>t</title>", ""),
      "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
       content=\"width=device-width, initial-scale=1\"><title>t</title></head><body>"
    );
    assert_eq!(document.closing(None), "</body></html>");
  }

  /// A bundle is preloaded in the head and booted at the end of the body, after the
  /// state, by a module script carrying the nonce.
  #[test]
  fn a_bundle_is_preloaded_and_booted_under_the_nonce() {
    let document = Document {
      bundle: Some("app".to_owned()),
      nonce: Some("n0".to_owned()),
      ..Document::default()
    };
    let opening = document.opening("", "");
    let closing = document.closing(Some("{}"));

    assert!(
      opening.contains("<link rel=\"modulepreload\" href=\"/app.js\">"),
      "{opening}"
    );
    assert!(
      opening.contains(
        "<link rel=\"preload\" href=\"/app_bg.wasm\" as=\"fetch\" type=\"application/wasm\" \
         crossorigin>"
      ),
      "{opening}"
    );
    assert!(
      closing.ends_with(
        "<script type=\"module\" nonce=\"n0\">import init from \"/app.js\";init();</script>\
         </body></html>"
      ),
      "{closing}"
    );
    assert!(
      closing.starts_with("<script id=\"skene-state\""),
      "{closing}"
    );
  }

  /// A page in the dev loop carries the marker and the dev script, under the nonce.
  #[test]
  fn a_dev_page_carries_the_marker_and_the_script() {
    let document = Document {
      nonce: Some("n0".to_owned()),
      dev: true,
      ..Document::default()
    };
    let opening = document.opening("", "");

    assert!(
      opening.contains("<meta name=\"skene-dev\"><script nonce=\"n0\">(() => {"),
      "{opening}"
    );
    assert!(opening.contains("/skene:reload"), "{opening}");
  }

  /// A bundle name closes neither the attribute nor the script it is written into.
  #[test]
  fn a_bundle_name_is_escaped_in_both_places() {
    let document = Document {
      bundle: Some("a\"</script>".to_owned()),
      ..Document::default()
    };

    assert!(
      document
        .opening("", "")
        .contains("href=\"/a&quot;&lt;/script&gt;.js\"")
    );
    assert!(
      document
        .closing(None)
        .contains("import init from \"/a\\\"\\u003c/script>.js\";")
    );
  }
}
