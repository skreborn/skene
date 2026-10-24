# Testing

A test runs on the platform it tests, against its real realization: a terminal test in
the terminal's in-process tier, a web test in a real browser or on the server leaf.
There is no mock platform. A test finds what a reader finds, by role and accessible
name or by text, and acts as a user acts, through the gestures of the `user` module:

```rust
use skene_simple_terminal::prelude::*;
use skene_terminal::{
  mount,
  prelude::*,
  testing::{find_by_role, find_by_text, render_to_text, user, with_platform},
};

fn counter() -> impl View {
  let count = Signal::new(0u32);

  Row::new().set_spacing(2).set_children((
    Text::derive(move || count.get().to_string()),
    SimpleButton::new("Add").handle_activate(move || count.update(|count| *count += 1)),
  ))
}

with_platform(|| {
  let _counter = mount(|| counter().build());

  // A gesture lands where the frame drew the view, so the test paints one first.
  let _ = render_to_text(20, 3);

  user::click(find_by_role(Role::Button, "Add").expect("the Add button"));

  assert!(find_by_text("1").is_some());
  assert!(render_to_text(20, 3).starts_with('1'));
});
```

`with_platform` installs a fresh terminal for the closure and removes it afterwards, and
`mount` builds a view under its root. `user::click`, `user::type_text` and
`user::press_key` turn a gesture into the events a browser dispatches for it, so a test
exercises focus, keys and the controls' own handling as a reader would. The
`trigger_*` seams simulate what reaches the app from outside (a key, a resize, a
frame of the hand-stepped clock), and a frame renders to text a snapshot compares:
each cell's character, and its style as a character beside it.

## The web

A web test runs in a real browser, built for `wasm32-unknown-unknown` and run under
`wasm-bindgen-test`'s runner, which drives Firefox or Chromium over WebDriver.
`skene_web::testing` installs the browser leaf on a fresh root, and the test reads the
elements it built and the styles the engine computes for them, through `web-sys` and
`wasm-bindgen` beside `wasm-bindgen-test` among the crate's dev-dependencies:

```rust,ignore
#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use skene_simple_web::prelude::*;
use skene_web::{
  platform::mount,
  prelude::*,
  testing::{computed, fresh_root, only_child},
};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

fn counter() -> impl View {
  let count = Signal::new(0u32);

  Row::new().set_spacing(8).set_children((
    Text::derive(move || count.get().to_string()),
    SimpleButton::new("Add").handle_activate(move || count.update(|count| *count += 1)),
  ))
}

#[wasm_bindgen_test]
fn a_click_counts() {
  let (_platform, root) = fresh_root();
  let _counter = mount(|| counter().build());
  let row = only_child(&root);

  assert_eq!(computed(&row, "column-gap"), "8px");

  let button: web_sys::HtmlElement = row
    .query_selector("button")
    .expect("a selector")
    .expect("the Add button")
    .unchecked_into();

  button.click();

  assert!(row.text_content().unwrap_or_default().starts_with('1'));
}
```

`testing::dispatch` fires a bubbling event a test names. An event a page dispatches to
itself is untrusted and runs no browser default, so what needs trusted input (hover, a
real key) is a conformance scenario, whose web harness acts through WebDriver's actions.

The server leaf is tested natively, in its own build: a plain `#[test]` renders a page
with `render_to_string` and reads the markup and the stylesheet the browser would
receive, here for the same counter.

```rust
# use skene_simple_web::prelude::*;
use skene_web::{prelude::*, render_to_string};
#
# fn counter() -> impl View {
#   let count = Signal::new(0u32);
#
#   Row::new().set_spacing(8).set_children((
#     Text::derive(move || count.get().to_string()),
#     SimpleButton::new("Add").handle_activate(move || count.update(|count| *count += 1)),
#   ))
# }

let page = render_to_string(counter);

assert!(page.body().contains(">0</span>"), "{}", page.body());
assert!(page.body().contains("<button"), "{}", page.body());
```

## Shared logic

An app's shared crate of logic and state names no platform, so its tests are plain
`#[test]`s over its signals and stores, which build no view and install no platform.

## What platforms share

A behavior every platform must share is a scenario in `skene-conformance`, which runs
one suite through a driver each platform implements over its own accessibility layer
and input; a divergence it finds is a bug in the platform, never a special case in the
suite.
