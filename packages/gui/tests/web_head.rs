//! Server rendering of the document head: the title and registered head elements.

#![cfg(all(target_arch = "wasm32", target_os = "wasi"))]

// The csr and ssr halves each use a different subset of the surface traits.
#[allow(unused_imports)]
use skene_gui::{HasDecoration, HasEvents, HasFocus, HasSize, HasStyle};
use skene_gui::{
  Text,
  head,
  reactive::{Get, Signal},
  render_to_string,
};

/// The title and head elements serialize into the head, in registration order.
#[test]
fn head_registrations_serialize() {
  let rendered = render_to_string(|| {
    head::set_title("Home — skene");
    head::add_meta("description", "A tiny page");
    head::add_meta_property("og:title", "Home");
    head::add_stylesheet("/assets/site.css");
    head::add_script("/assets/site.js");

    Text::new("body")
  });

  let head = rendered.head();

  assert!(head.starts_with("<title>Home — skene</title>"));
  assert!(head.contains("<meta name=\"description\" content=\"A tiny page\">"));
  assert!(head.contains("<meta property=\"og:title\" content=\"Home\">"));
  assert!(head.contains("<link rel=\"stylesheet\" href=\"/assets/site.css\">"));
  assert!(head.contains("<script src=\"/assets/site.js\" defer=\"\"></script>"));

  let meta = head.find("name=\"description\"").expect("the meta");
  let link = head.find("rel=\"stylesheet\"").expect("the link");

  assert!(meta < link);
  assert!(rendered.page().contains(&format!("<head>{head}")));
}

/// A derived title reads reactive state at render time.
#[test]
fn derived_title_reads_signals() {
  let rendered = render_to_string(|| {
    let count = Signal::new(3u32);

    head::derive_title(move || format!("{} items", count.get()));

    Text::new("body")
  });

  assert!(rendered.head().contains("<title>3 items</title>"));
}

/// Title text and attribute values escape their markup-significant characters.
#[test]
fn head_content_escapes() {
  let rendered = render_to_string(|| {
    head::set_title("a < b & c");
    head::add_meta("description", "say \"hi\" & <leave>");

    Text::new("body")
  });

  let head = rendered.head();

  assert!(head.contains("<title>a &lt; b &amp; c</title>"));
  assert!(head.contains("content=\"say &quot;hi&quot; &amp; &lt;leave&gt;\""));
  assert!(!head.contains("say \"hi\""));
}

/// A page with no head registrations keeps an empty head slot before the styles.
#[test]
fn empty_head_adds_nothing() {
  let rendered = render_to_string(|| Text::new("body"));

  assert_eq!(rendered.head(), "");
}
