//! Stage one's smoke tests of the Linux realization, on real GTK: the engine lays a row out
//! through the host container, a keyed list and a derived view keep the widget order in
//! place, and a controlled switch shows its truth. They read GTK's widget tree in process;
//! `skene_gui::testing` (stage three) reads what a reader reads instead.

use std::time::Duration;

use skene_gui::{
  __private::{present, settle},
  platform::linux::gtk::{self, prelude::*},
  prelude::*,
};

fn children(widget: &gtk::Widget) -> Vec<gtk::Widget> {
  let mut children = Vec::new();
  let mut next = widget.first_child();

  while let Some(child) = next {
    next = child.next_sibling();
    children.push(child);
  }

  children
}

/// The window's root box's one child: the view the test presented.
fn presented(window: &gtk::Window) -> gtk::Widget {
  let root = window.child().expect("the window shows the root box");

  children(&root).remove(0)
}

fn labels(widget: &gtk::Widget) -> Vec<String> {
  children(widget)
    .iter()
    .filter_map(|child| {
      child
        .downcast_ref::<gtk::Label>()
        .map(|label| label.label().into())
    })
    .collect()
}

fn halves() -> impl View {
  Row::new()
    .set_padding(5)
    .set_spacing(10)
    .add_child(
      Flexible::new()
        .set_flexibility(1)
        .set_child(Container::new().set_height(20)),
    )
    .add_child(
      Flexible::new()
        .set_flexibility(1)
        .set_child(Container::new().set_height(20)),
    )
}

#[skene_gui::test]
fn flexible_children_share_a_row_through_the_host_container() {
  let window = present(halves, 420, -1);
  let row = presented(&window);
  let rects: Vec<(f32, f32, f32, f32)> = children(&row)
    .iter()
    .map(|child| {
      let bounds = child.compute_bounds(&row).expect("the child is laid out");

      (bounds.x(), bounds.y(), bounds.width(), bounds.height())
    })
    .collect();

  assert_eq!(row.width(), 420);
  assert_eq!(rects, [(5., 5., 200., 20.), (215., 5., 200., 20.)]);
}

thread_local! {
  static ORDER: std::cell::OnceCell<Signal<Vec<u32>>> = const { std::cell::OnceCell::new() };
  static SHOWN: std::cell::OnceCell<Signal<bool>> = const { std::cell::OnceCell::new() };
}

fn list() -> impl View {
  let order = Signal::new(vec![1, 2, 3]);
  let shown = Signal::new(true);

  ORDER.with(|cell| cell.set(order).ok());
  SHOWN.with(|cell| cell.set(shown).ok());

  Column::new()
    .add_child(Text::new("first"))
    .derive_children(
      move || order.get(),
      |item| *item,
      |item| Text::new(format!("item {item}")),
    )
    .add_child(Derived::new(move || {
      Text::new(if shown.get() {
        "derived on"
      } else {
        "derived off"
      })
    }))
    .add_child(Text::new("last"))
}

#[skene_gui::test]
fn keyed_and_derived_children_keep_their_places() {
  let window = present(list, 300, -1);
  let column = presented(&window);
  let order = ORDER.with(|cell| *cell.get().expect("the list was built"));
  let shown = SHOWN.with(|cell| *cell.get().expect("the list was built"));

  assert_eq!(
    labels(&column),
    ["first", "item 1", "item 2", "item 3", "derived on", "last"]
  );

  let kept = children(&column)[3].clone();

  order.set(vec![3, 1]);
  shown.set(false);
  settle(Duration::from_millis(30));

  assert_eq!(
    labels(&column),
    ["first", "item 3", "item 1", "derived off", "last"]
  );
  assert_eq!(children(&column)[1], kept, "a kept key keeps its widget");

  order.set(vec![1, 2, 3, 4]);
  settle(Duration::from_millis(30));

  assert_eq!(
    labels(&column),
    [
      "first",
      "item 1",
      "item 2",
      "item 3",
      "item 4",
      "derived off",
      "last"
    ]
  );
}

thread_local! {
  static SYNC: std::cell::OnceCell<Signal<bool>> = const { std::cell::OnceCell::new() };
}

fn sync() -> impl View {
  let sync = Signal::new(true);

  SYNC.with(|cell| cell.set(sync).ok());

  Column::new().add_child(Toggle::new("Sync").derive_on(move || sync.get()))
}

#[skene_gui::test]
fn a_switch_shows_its_truth_and_refuses_a_change_it_does_not_take() {
  let window = present(sync, 300, -1);
  let toggle = children(&presented(&window)).remove(0);
  let switch = children(&toggle)
    .into_iter()
    .find_map(|child| child.downcast::<gtk::Switch>().ok())
    .expect("the toggle holds a GtkSwitch");
  let truth = SYNC.with(|cell| *cell.get().expect("the page was built"));

  assert!(switch.is_active());
  truth.set(false);
  settle(Duration::from_millis(30));
  assert!(!switch.is_active());

  // A toggle with no change handler keeps its truth: the flip GTK proposes is refused.
  switch.set_active(true);
  settle(Duration::from_millis(30));
  assert!(!switch.is_active());
  assert!(!switch.state());
}

fn padded() -> impl View {
  Container::new()
    .set_padding(8.when(Media::MinWidth(900), 32))
    .set_child(Container::new().set_height(10))
}

#[skene_gui::test]
fn a_breakpoint_follows_the_window_width() {
  let window = present(padded, 480, -1);
  let outer = presented(&window);
  let inner = children(&outer).remove(0);
  let x = |inner: &gtk::Widget| inner.compute_bounds(&outer).expect("laid out").x();

  assert_eq!(x(&inner), 8.);
  window.set_default_size(1000, -1);
  settle(Duration::from_millis(50));
  assert_eq!(outer.width(), 1000);
  assert_eq!(x(&inner), 32.);
}

thread_local! {
  static RED: std::cell::OnceCell<Signal<bool>> = const { std::cell::OnceCell::new() };
}

fn swatch() -> impl View {
  let red = Signal::new(false);

  RED.with(|cell| cell.set(red).ok());

  Container::new()
    .set_alignment(Alignment::TOP_LEADING)
    .set_child(
      Container::new()
        .set_width(40)
        .set_height(20)
        .derive_background(move || {
          if red.get() {
            Color::rgb(0xff0000)
          } else {
            Color::rgb(0x0000ff)
          }
        }),
    )
}

/// The color drawn at the center of `widget`, as its window renders it.
fn center(widget: &gtk::Widget) -> [u8; 4] {
  let texture = skene_gui_linux::app::render(widget);
  let (width, height) = (texture.width() as usize, texture.height() as usize);
  let mut pixels = vec![0; width * height * 4];

  texture.download(&mut pixels, width * 4);

  let at = ((height / 2) * width + width / 2) * 4;

  // GDK downloads in the platform's native byte order, BGRA on little-endian hosts.
  [pixels[at + 2], pixels[at + 1], pixels[at], pixels[at + 3]]
}

#[skene_gui::test]
fn a_derived_color_is_a_per_widget_rule_and_no_new_class() {
  let window = present(swatch, 200, -1);
  let swatch = children(&presented(&window)).remove(0);
  let red = RED.with(|cell| *cell.get().expect("the swatch was built"));
  let before = skene_gui_linux::sheet::stats();

  assert_eq!(center(&swatch), [0, 0, 255, 255]);
  red.set(true);
  settle(Duration::from_millis(30));

  let after = skene_gui_linux::sheet::stats();

  assert_eq!(center(&swatch), [255, 0, 0, 255]);
  assert_eq!(after.applies, before.applies, "no style was applied anew");
  assert_eq!(after.slot_writes, before.slot_writes + 1);
}

#[skene_gui::test]
fn the_app_s_scheme_choice_and_back_to_the_reader_s() {
  let settings = gtk::Settings::default().expect("GTK is initialized");
  let dark = || Environment::current().dark.get_untracked();

  // The reader prefers dark, as GtkSettings reports where no portal answers.
  settings.set_gtk_application_prefer_dark_theme(true);
  settle(Duration::from_millis(10));
  assert!(dark());

  media::set_scheme(Some(Scheme::Light));
  assert!(!dark());
  assert!(!settings.is_gtk_application_prefer_dark_theme());

  media::set_scheme(None);
  assert!(dark(), "following the reader again is not forcing light");
  assert!(settings.is_gtk_application_prefer_dark_theme());

  media::set_scheme(Some(Scheme::Dark));
  settings.set_gtk_application_prefer_dark_theme(false);
  settle(Duration::from_millis(10));
  media::set_scheme(None);

  assert!(
    !dark(),
    "the reader's change while the app chose is followed"
  );
}
