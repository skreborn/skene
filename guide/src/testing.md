# Testing

A view test runs on the host, against the headless platform the web's facade builds
for there, and needs neither a browser nor a terminal. A test finds what a reader
finds, by role and accessible name or by text, and acts as a user acts, through the
gestures of the `user` module:

```rust
use skene_simple_web::prelude::*;
use skene_web::{
  headless::{find_by_role, find_by_text, reset, user},
  prelude::*,
};

fn counter() -> impl View {
  let count = Signal::new(0u32);

  Row::new().set_spacing(8.).set_children((
    Text::derive(move || count.get().to_string()),
    SimpleButton::new("Add").handle_activate(move || count.update(|count| *count += 1)),
  ))
}

reset();

let owner = Owner::new();
let _counter = owner.enter(|| counter().build());

user::click(find_by_role(Role::Button, "Add").expect("the Add button"));

assert!(find_by_text("1").is_some());
```

`reset` gives each test a fresh platform. `user::click`, `user::type_text` and
`user::press_key` turn a gesture into the events a browser dispatches for it, so a
test exercises focus, keys and the controls' own handling as a reader would.

## The terminal

The terminal's seams live in `skene_terminal::testing`, under the same names, and a
frame renders to text a snapshot compares: each cell's character, and its style as a
character beside it.

## What platforms share

A behavior every platform must share is a scenario in `skene-conformance`, which runs
one suite through a driver each platform implements. A platform of your own proves
itself by implementing that driver.
