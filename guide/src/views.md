# Views and layout

A view is its own builder. Its setters each take and return the view, so a chain of them
describes it, and a parent takes it as a child. The examples in this chapter use the
web's facade; the terminal's views are the same, measured in cells.

```rust
use skene_web::prelude::*;

fn greeting() -> impl View {
  Text::new("Hello")
    .set_font_size(24.)
    .set_font_weight(600.)
    .set_color(Color::srgb(0.2, 0.3, 0.8))
}
```

Every property has one setter, `set_<property>`, and a reactive twin,
`derive_<property>`, which takes a closure and follows the signals it reads. An event
takes a handler through `handle_<event>`. A view's children come last in its chain:
`set_child` for a view of one child, and `add_child` or `set_children` for a layout,
which takes a tuple of views of any types.

A setting the chain may lack goes through `apply`, which calls the setter where there is
a value and leaves the view as it is where there is none:

```rust
use skene_web::prelude::*;

fn caption(text: &str, color: Option<Color>) -> impl View {
  Text::new(text).apply(color, Text::set_color)
}
```

## Layout

Each layout is its own view, and exposes only the properties it honors: `Row` and
`Column` place their children along one axis, `Grid` in tracks, and `Stack` on top of
one another.

```rust
use skene_web::prelude::*;

fn toolbar() -> impl View {
  Row::new()
    .set_spacing(8.)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((
      Text::new("Title"),
      Flexible::new()
        .set_flexibility(Flexibility::grow(1.))
        .set_child(Text::new("Fills the rest")),
      Text::new("End"),
    ))
}
```

A child's own place in its layout travels in a wrapper: `Flexible` in a row or a
column, `GridCell` in a grid, `Pinned` in a stack. A wrapper creates no node of its own,
and a layout accepts only the wrapper that belongs to it, so a misplaced one does not
compile.

`Arrangement` distributes the children along the main axis, and `ItemAlignment` places
each across it:

```rust
use skene_web::prelude::*;

fn centered() -> impl View {
  Column::new()
    .set_arrangement(Arrangement::Center)
    .set_item_alignment(ItemAlignment::Center)
    .set_children((Text::new("One"), Text::new("Two")))
}
```

## Decoration

A layout carries layout and its own padding alone. `Container` is the one decorated box:
a background, a border, corners, a size, and exactly one child.

```rust
use skene_web::prelude::*;

fn panel() -> impl View {
  Container::new()
    .set_padding(Inset::symmetric(16., 12.))
    .set_background(Color::srgb(0.95, 0.95, 0.97))
    .set_border_width(Inset::all(1.))
    .set_border_color(Color::srgb(0.85, 0.85, 0.9))
    .set_corner_radius(8.)
    .set_child(Column::new().set_spacing(8.).set_children((
      Text::new("A panel"),
      Text::new("Its contents"),
    )))
}
```

## Conditions

A value varies under a condition through `when`, on the value itself, so no property
has a conditional setter of its own. A condition is a medium's (`Media::MinWidth`,
`Media::Dark`) or an interaction's (`State::Hovered`, `State::FocusVisible`):

```rust
use skene_web::prelude::*;

fn responsive() -> impl View {
  Row::new()
    .set_spacing(Spacing::from(8.).when(Media::MinWidth(900.), 16.))
    .set_children((Text::new("Narrow"), Text::new("Wide")))
}
```

On the web every condition compiles to CSS, so a page the server rendered is
responsive, hoverable and themed before any script runs.

## Reactive children

A layout's child can follow signals: `derive_child` rebuilds one child when the signals
it reads change, and `derive_children` keeps one child per item, keyed, so an item that
moves keeps its view and the state beneath it.

```rust
use skene_web::prelude::*;

fn names(names: Signal<Vec<String>>) -> impl View {
  Column::new().set_spacing(4.).derive_children(
    move || names.get(),
    |name| name.clone(),
    |name| Text::new(name),
  )
}
```
