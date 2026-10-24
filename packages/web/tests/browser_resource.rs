//! Asynchronously loaded data, from the load to the swapped-in tree.
//!
//! Run in a headless browser:
//! `cargo test-web --test browser_resource`.
#![cfg(all(skene_platform = "web", not(skene_server)))]

use std::{cell::Cell, rc::Rc};

use skene_web::{
  Container,
  ContainerBase,
  HasChild,
  Resource,
  Text,
  TextBase,
  View,
  derived,
  platform::web::mount,
  reactive::Owner,
  testing::{fresh_root, tick},
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

/// Enough turns for a load parked on a few awaits to finish.
async fn settle() {
  for _ in 0..16 {
    tick().await;
  }
}

#[wasm_bindgen_test]
async fn a_resource_is_none_until_it_lands() {
  let scope = Owner::new();

  let resource = scope.enter(|| {
    Resource::new(|| async {
      tick().await;

      7
    })
  });

  assert_eq!(resource.get(), None, "loading");
  assert!(resource.is_loading());

  settle().await;

  assert_eq!(resource.get(), Some(7), "loaded");
  assert!(!resource.is_loading());
}

/// Failure is the loaded type's business: load a `Result` and match on it.
#[wasm_bindgen_test]
async fn a_resource_can_load_a_result() {
  let scope = Owner::new();

  let resource: Resource<Result<i32, String>> =
    scope.enter(|| Resource::new(|| async { Err("boom".to_owned()) }));

  settle().await;

  assert_eq!(resource.get(), Some(Err("boom".to_owned())));
}

/// The headline: load data, show a placeholder meanwhile, then display it—end to end,
/// against a real DOM.
#[wasm_bindgen_test]
async fn a_loading_tree_is_swapped_for_the_loaded_one() {
  let (_guard, root) = fresh_root();

  let _ = mount(|| {
    let user = Resource::new(|| async {
      tick().await;

      "Ada".to_owned()
    });

    derived(move || {
      user.with(|user| match user {
        None => Text::new("Loading…").build(),
        Some(name) => Container::new().set_child(Text::new(name.clone())).build(),
      })
    })
  });

  assert_eq!(
    root
      .first_element_child()
      .unwrap()
      .text_content()
      .as_deref(),
    Some("Loading…"),
    "the placeholder shows while loading",
  );

  settle().await;

  let loaded = root.first_element_child().unwrap();

  assert_eq!(root.children().length(), 1, "the placeholder is torn down");

  assert_eq!(loaded.tag_name(), "DIV");
  assert_eq!(loaded.text_content().as_deref(), Some("Ada"));
}

/// A resource's load is scope-bound like any other task: disposing the scope stops it,
/// so it never lands in a torn-down tree.
#[wasm_bindgen_test]
async fn disposing_the_scope_cancels_the_load() {
  let scope = Owner::new();
  let finished = Rc::new(Cell::new(false));

  scope.enter({
    let finished = Rc::clone(&finished);

    || {
      let _load = Resource::new(move || {
        let finished = Rc::clone(&finished);

        async move {
          for _ in 0..8 {
            tick().await;
          }

          finished.set(true);

          1
        }
      });
    }
  });

  tick().await;

  scope.dispose();

  settle().await;

  // The resource itself is not read back: its signal was disposed with the scope, and
  // reading a disposed signal panics. What matters is that the load stopped.
  assert!(!finished.get(), "the load stopped rather than completing");
}

/// `reload` re-runs the retained loader into the value, keeping the last value
/// readable until the next one lands.
#[wasm_bindgen_test]
async fn reload_reruns_the_loader() {
  let scope = Owner::new();
  let runs = Rc::new(Cell::new(0));

  let resource: Resource<i32> = scope.enter({
    let runs = Rc::clone(&runs);

    move || {
      Resource::new(move || {
        let runs = Rc::clone(&runs);

        async move {
          let run = runs.get();
          runs.set(run + 1);

          run
        }
      })
    }
  });

  settle().await;

  assert_eq!(resource.get(), Some(0), "the first load ran once");

  resource.reload();

  assert_eq!(
    resource.get(),
    Some(0),
    "the last value stays readable while it reloads"
  );
  assert!(resource.is_refetching());

  settle().await;

  assert_eq!(resource.get(), Some(1), "reload re-ran the loader");
}
