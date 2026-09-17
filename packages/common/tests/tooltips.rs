//! Tooltips: the bubble opens after the delay on hover and at once on focus, closes on
//! leave, blur and dismiss, and the described view reports the bubble and its text.

mod common;

use skene_common::{Activate, TOOLTIP_DELAY};
use skene_core::{AccessProp, FocusOrder, HasChildren, Part, Placement, Scene, View};
use skene_headless::{
  Headless,
  advance,
  box_props,
  handle,
  is_open,
  overlay_anchor,
  overlay_props,
  part,
  reset,
  trigger_blur,
  trigger_dismiss,
  trigger_focus,
  trigger_pointer_enter,
  trigger_pointer_leave,
};
use skene_reactive::{Get, Owner, Set, Signal};

type Container = skene_common::Container<Headless>;
type Text = skene_common::Text<Headless>;
type Tooltip<C = skene_common::NoChild> = skene_common::Tooltip<Headless, C>;
type RawButton = skene_common::RawButton<Headless>;

/// The described view and its bubble, mounted in the segment `start` begins.
fn parts(start: skene_core::NodeId) -> (skene_core::NodeId, skene_core::NodeId) {
  let nodes = Scene::<Headless>::with(|scene| scene.segment_nodes(start));

  assert_eq!(
    nodes.len(),
    2,
    "the described view and its bubble: {nodes:?}"
  );

  (nodes[0], nodes[1])
}

/// A text tooltip describes its child: the bubble is a hint anchored to the child and
/// its part, and the child reports the bubble and the text. It opens once the pointer
/// has rested for the delay, closes on leave, opens at once on focus and closes on
/// blur and on a dismiss.
#[test]
fn a_tooltip_opens_on_hover_after_its_delay_and_on_focus_at_once() {
  reset();

  let owner = Owner::new();
  let start = owner.enter(|| {
    Tooltip::new("Saves your work")
      .set_placement(Placement::Bottom)
      .set_child(
        Container::new()
          .set_focus_order(FocusOrder::Auto)
          .set_child(Text::new("Save")),
      )
      .build()
  });
  let (described, bubble) = parts(start);
  let props = overlay_props(bubble).expect("a bubble overlay");

  assert!(props.hint && !props.modal);
  assert_eq!(props.placement, Placement::Bottom);
  assert_eq!(part(bubble), Some(Part::Bubble));
  assert_eq!(
    box_props(described).map(|props| props.access),
    Some(vec![
      AccessProp::DescribedBy(bubble),
      AccessProp::Description("Saves your work".to_owned()),
    ])
  );
  assert!(!is_open(bubble));

  trigger_pointer_enter(described);

  assert!(!is_open(bubble), "the pointer has not rested yet");

  advance(TOOLTIP_DELAY / 2);

  assert!(!is_open(bubble));

  advance(TOOLTIP_DELAY);

  assert!(is_open(bubble), "the delay passed");
  assert_eq!(overlay_anchor(bubble), handle(described));

  trigger_pointer_leave(described);

  assert!(!is_open(bubble));

  trigger_pointer_enter(described);
  trigger_pointer_leave(described);
  advance(TOOLTIP_DELAY * 2);

  assert!(
    !is_open(bubble),
    "a leave before the delay cancels the opening"
  );

  trigger_focus(described);

  assert!(is_open(bubble), "focus opens at once");

  trigger_dismiss(bubble);

  assert!(!is_open(bubble), "a dismiss closes");

  trigger_focus(described);
  trigger_blur(described);

  assert!(!is_open(bubble));

  owner.dispose();
}

/// A view tooltip reports only the bubble; a derived text tooltip re-describes as its
/// text changes; and a control's own report keeps the description beside it.
#[test]
fn a_tooltips_report_sits_beside_the_controls_own() {
  reset();

  let owner = Owner::new();
  let disabled = Signal::new(false);
  let text = Signal::new("Saves".to_owned());
  let derived = owner.enter(|| {
    Tooltip::derive(move || text.get())
      .set_child(
        RawButton::new(Activate::new())
          .derive_disabled(move || disabled.get())
          .set_label("Save"),
      )
      .build()
  });
  let (button, bubble) = parts(derived);
  let access = || {
    box_props(button)
      .map(|props| props.access)
      .unwrap_or_default()
  };

  assert!(access().contains(&AccessProp::DescribedBy(bubble)));
  assert!(access().contains(&AccessProp::Description("Saves".to_owned())));

  disabled.set(true);

  assert!(
    access().contains(&AccessProp::Disabled(true)),
    "the control's own report: {:?}",
    access()
  );
  assert!(
    access().contains(&AccessProp::DescribedBy(bubble)),
    "the description survives the control's rewrite: {:?}",
    access()
  );

  text.set("Saves the document".to_owned());

  assert!(access().contains(&AccessProp::Description("Saves the document".to_owned())));
  assert!(access().contains(&AccessProp::Disabled(true)));

  let viewed = owner.enter(|| {
    Tooltip::view(Text::new("rich"))
      .set_child(Container::new())
      .build()
  });
  let (box_node, view_bubble) = parts(viewed);

  assert_eq!(
    box_props(box_node).map(|props| props.access),
    Some(vec![AccessProp::DescribedBy(view_bubble)])
  );

  owner.dispose();
}
