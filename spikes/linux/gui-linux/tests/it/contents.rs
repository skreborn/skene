//! Contents as the seam writes and reads them, and the controls' keys.

use std::rc::Rc;

use skene_foundation::seam::{
  content::{Choice, Content, PropValue, Range},
  node::ControlKind,
  report::{KeyEvent, Modifiers},
};
use skene_gui_linux::{
  controls::{Act, Contents},
  node::Drawn,
};

fn event(key: &str, down: bool) -> KeyEvent {
  KeyEvent {
    key: key.to_owned(),
    code: String::new(),
    down,
    repeat: false,
    modifiers: Modifiers::default(),
  }
}

#[test]
fn each_content_reads_back_what_was_written() {
  let mut contents = Contents::default();
  let samples = [
    (Content::Text, PropValue::Text("Skene".into())),
    (Content::Caption, PropValue::Text("Greet".into())),
    (Content::On, PropValue::Bool(true)),
    (Content::Prominent, PropValue::Bool(true)),
    (Content::Enabled, PropValue::Bool(false)),
    (
      Content::Range,
      PropValue::Range(Range {
        value: 0.5,
        minimum: 0.,
        maximum: 1.,
        step: 0.1,
      }),
    ),
    (
      Content::Choice,
      PropValue::Choice(Choice {
        options: Rc::from(vec![Rc::from("a"), Rc::from("b")]),
        chosen: Some(1),
      }),
    ),
  ];

  for (content, value) in samples {
    contents.write(content, &value);
    assert_eq!(contents.read(content), Some(value), "{content:?}");
  }
}

fn control(kind: ControlKind) -> std::rc::Rc<Drawn> {
  Drawn::new(
    skene_foundation::Scene::<()>::new().insert(()),
    skene_foundation::seam::node::NodeSpec::new(skene_foundation::seam::node::NodeKind::Control(
      kind,
    )),
  )
}

#[test]
fn a_button_acts_on_enter_at_once_and_on_space_at_its_release() {
  let node = control(ControlKind::Button);
  let enter = node.key_control(ControlKind::Button, &event("Enter", true), None);

  assert!(enter.consumed);
  assert_eq!(enter.act, Some(Act::Activate));

  let space = node.key_control(ControlKind::Button, &event(" ", true), None);

  assert!(space.consumed);
  assert_eq!(space.act, None);
  assert_eq!(space.pressed, Some(true));
  // The shell shows it pressed until the release.
  node.key_pressed.set(true);

  let release = node.key_control(ControlKind::Button, &event(" ", false), None);

  assert_eq!(release.act, Some(Act::Activate));
  assert!(
    !node
      .key_control(ControlKind::Button, &event("a", true), None)
      .consumed
  );
}

#[test]
fn a_switch_flips_on_space_at_its_release_and_ignores_enter() {
  let node = control(ControlKind::Switch);

  let press = node.key_control(ControlKind::Switch, &event(" ", true), None);

  assert_eq!(press.act, None);
  assert_eq!(press.pressed, Some(true));
  node.key_pressed.set(true);
  assert_eq!(
    node
      .key_control(ControlKind::Switch, &event(" ", false), None)
      .act,
    Some(Act::Change(Content::On, PropValue::Bool(true)))
  );
  node.key_pressed.set(false);
  assert_eq!(
    node
      .key_control(ControlKind::Switch, &event("Enter", true), None)
      .act,
    None
  );
}

#[test]
fn a_slider_steps_and_stops_at_its_bounds() {
  let node = control(ControlKind::Slider);

  node.contents.borrow_mut().write(
    Content::Range,
    &PropValue::Range(Range {
      value: 0.9,
      minimum: 0.,
      maximum: 1.,
      step: 0.1,
    }),
  );

  let Some(Act::Change(Content::Range, PropValue::Range(moved))) = node
    .key_control(ControlKind::Slider, &event("ArrowRight", true), None)
    .act
  else {
    panic!("the slider moves");
  };

  assert!((moved.value - 1.).abs() < 1e-9);

  node
    .contents
    .borrow_mut()
    .write(Content::Range, &PropValue::Range(moved));

  let at_end = node.key_control(ControlKind::Slider, &event("ArrowRight", true), None);

  assert!(at_end.consumed, "the key is the slider's");
  assert_eq!(at_end.act, None, "at its greatest it stays");
}

#[test]
fn a_picker_is_as_wide_as_its_widest_option_laid_out_by_the_engine() {
  use skene_foundation::{
    seam::{
      content::contents,
      node::{LayoutKind, NodeKind, NodeSpec},
      record::Record,
      tree,
    },
    text::{AttributedText, Paragraph as _, ParagraphStyle, RunStyle, TextEngine as _},
  };
  use skene_gui_linux::{Linux, shell::Shell};

  super::owned(|| {
    let (root, root_node) = super::node(
      NodeSpec::new(NodeKind::Layout(LayoutKind::Row)),
      Record::new(),
    );
    let (picker, picker_node) = super::node(
      NodeSpec::new(NodeKind::Control(ControlKind::Picker)),
      Record::new(),
    );
    let widest = "漢字の選択肢はとても長いです";

    tree::bind::<Linux, _>(picker, contents::CHOICE, move || Choice {
      options: Rc::from(vec![Rc::from("a"), Rc::from(widest)]),
      chosen: Some(0),
    });
    tree::attach::<Linux>(root, picker, 0);

    let mut shell = Shell::new("test", 800, 100);

    shell.set_root(root_node);
    shell.configure(800, 100);
    shell.frame();

    let engine = skene_parley::ParleyEngine::system();
    let width = engine
      .paragraph(
        &AttributedText::plain(widest, RunStyle::new(14.)),
        &ParagraphStyle::default(),
      )
      .intrinsic()
      .max;
    let face = picker_node.frame.get().width;

    assert!(
      face >= width + 28.,
      "the face ({face}) holds the widest option ({width}) and its padding"
    );
  });
}
