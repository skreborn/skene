//! What a reader finds, over AT-SPI in the display runner's private session: a text's link
//! run named by its text, which focus reaches by `Tab` and `Return` follows, and text sized
//! and set in the family the desktop's settings portal states.

use std::{cell::Cell, rc::Rc, time::Duration};

use skene_foundation::{
  Owner,
  seam::{
    access::Role,
    content::{Run, contents},
    node::{LayoutKind, NodeKind, NodeSpec},
    record::Record,
    report::Modifiers,
    testing::{Action, Driver as _, Query},
    tree,
  },
};
use skene_gui_linux::{
  Linux,
  testing::{Drawn, portal::Setting, run_test},
};

use super::node;

thread_local! {
  static FOLLOWED: Cell<u32> = const { Cell::new(0) };
}

#[test]
fn a_reader_finds_a_link_run_and_tab_and_return_follow_it() {
  run_test("a_reader_finds_a_link_run", || {
    let owner = Owner::new();
    let root = owner.run(|| {
      let (root, _) = node(
        NodeSpec::new(NodeKind::Layout(LayoutKind::Column)),
        Record::new(),
      );
      let (text, _) = node(NodeSpec::new(NodeKind::Text), Record::new());

      tree::bind::<Linux, _>(text, contents::RUNS, || {
        Rc::from(vec![
          Run {
            text: Rc::from("Read "),
            props: Rc::from(vec![]),
            destination: None,
          },
          Run {
            text: Rc::from("the docs"),
            props: Rc::from(vec![]),
            destination: Some(Rc::from("https://example.com/docs")),
          },
        ])
      });
      tree::handle_follow::<Linux, ()>(root, |_| FOLLOWED.with(|count| count.set(count.get() + 1)));
      tree::attach::<Linux>(root, text, 0);

      root
    });
    let mut driver = Drawn::new();

    driver.show(owner, root);

    let query = Query::Role(Role::Link, "the docs".into());
    let links = driver.find(&query, Duration::from_secs(1));

    assert_eq!(
      links.len(),
      1,
      "one link, named by its text:\n{}",
      driver.outline()
    );

    let link = &links[0];

    assert!(!driver.read(link).focused);

    driver.act(link, &Action::Press("Tab".into(), Modifiers::default()));
    assert!(driver.read(link).focused, "Tab reaches the link run");

    driver.act(link, &Action::Press("Return".into(), Modifiers::default()));
    assert_eq!(FOLLOWED.with(Cell::get), 1, "Return follows it");

    driver.act(link, &Action::Click);
    assert_eq!(
      FOLLOWED.with(Cell::get),
      2,
      "a click at its bounds follows it"
    );
  });
}

#[test]
fn the_desktops_text_scale_and_interface_font_come_through_the_portal() {
  run_test("the_desktops_text_scale_and_interface_font", || {
    let owner = Owner::new();
    let root = owner.run(|| {
      let (root, _) = node(
        NodeSpec::new(NodeKind::Layout(LayoutKind::Row)),
        Record::new(),
      );
      let (text, _) = node(NodeSpec::new(NodeKind::Text), Record::new());

      tree::bind::<Linux, _>(text, contents::TEXT, || Rc::from("Scaled iiii"));
      tree::attach::<Linux>(root, text, 0);

      root
    });
    let mut driver = Drawn::new();

    driver.show(owner, root);

    let query = Query::Text("Scaled iiii".into());
    let bounds = |driver: &mut Drawn| {
      let found = driver.find(&query, Duration::from_secs(1));

      driver.read(&found[0]).bounds
    };
    let plain = bounds(&mut driver);

    driver.set_portal(Setting::TextScalingFactor(1500));

    let scaled = bounds(&mut driver);

    assert!(
      scaled.width > plain.width * 1.3,
      "text-scaling-factor 1.5 scales the text: {plain:?} then {scaled:?}"
    );

    driver.set_portal(Setting::TextScalingFactor(1000));
    driver.set_portal(Setting::FontName("DejaVu Sans Mono 11"));

    let mono = bounds(&mut driver);

    assert!(
      (mono.width - plain.width).abs() > 4.,
      "font-name names the interface family: {plain:?} then {mono:?}"
    );
  });
}
