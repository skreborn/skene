//! The gallery on the host platform: every control, link, picture and dialog it shows
//! is named, as a reader finds it.

#![cfg(not(any(target_arch = "wasm32", skene_platform = "server")))]

use skene_gui::{headless, prelude::*};

#[test]
fn every_control_the_gallery_shows_is_named() {
  headless::reset();

  let owner = Owner::new();
  let _root = owner.enter(|| gallery::app().into_view().build());
  let unnamed: Vec<_> = headless::find_all_unnamed()
    .into_iter()
    .map(|node| (node, headless::box_props(node).and_then(|props| props.role)))
    .collect();

  assert!(unnamed.is_empty(), "unnamed: {unnamed:?}");

  owner.dispose();
}
