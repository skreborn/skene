//! Byte-exact SSR snapshot: the rendered styles, body, and state for a representative
//! tree. Guards output parity (interned class names, hydration markers, markup shape)
//! across refactors of the realization pipeline; `browser_hydrate` hydrates the same
//! bytes in a browser.

#![cfg(all(skene_platform = "web", skene_server))]

use skene_gui::render_to_string;

/// The representative tree and the bytes the server leaf renders for it.
mod representative {
  include!("fixtures/representative.rs");
}

#[test]
fn representative_render_is_stable() {
  let rendered = render_to_string(representative::tree);

  assert_eq!(rendered.state_script(), representative::STATE_SCRIPT);
  assert_eq!(rendered.styles(), representative::STYLES);
  assert_eq!(rendered.body(), representative::BODY);
}
