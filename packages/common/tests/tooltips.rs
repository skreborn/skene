//! Tooltips: the bubble opens after the delay on hover and at once on focus, closes on
//! leave, blur and dismiss, and the described view reports the bubble and its text.

mod common;

use skene_common::{Activate, TOOLTIP_DELAY, TOOLTIP_GRACE};
use skene_core::{AccessProp, FocusOrder, HasChildren, Part, Placement, Scene, View};
use skene_headless::{
  Headless,
  box_props,
  handle,
  is_open,
  overlay_anchor,
  overlay_props,
  part,
  pending_timers,
  reset,
  trigger_blur,
  trigger_dismiss,
  trigger_focus,
  trigger_frame,
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
/// has rested for the delay, closes the grace period after a leave, opens at once on
/// focus and closes on blur and on a dismiss.
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
      AccessProp::DescribedBy(vec![bubble]),
      AccessProp::Description("Saves your work".to_owned()),
    ])
  );
  assert!(!is_open(bubble));

  trigger_pointer_enter(described);

  assert!(!is_open(bubble), "the pointer has not rested yet");

  trigger_frame(TOOLTIP_DELAY / 2);

  assert!(!is_open(bubble));

  trigger_frame(TOOLTIP_DELAY);

  assert!(is_open(bubble), "the delay passed");
  assert_eq!(overlay_anchor(bubble), handle(described));

  trigger_pointer_leave(described);

  assert!(is_open(bubble), "open through the grace period");

  trigger_frame(TOOLTIP_GRACE);

  assert!(!is_open(bubble), "closed once it passed");

  trigger_pointer_enter(described);
  trigger_pointer_leave(described);
  trigger_frame(TOOLTIP_DELAY * 2);

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

  trigger_pointer_enter(described);
  owner.dispose();

  assert_eq!(
    pending_timers(),
    0,
    "a rest still pending dies with the view"
  );
}

/// The pointer can move from the described view onto the bubble without closing it:
/// the bubble stays open while the pointer rests on either and closes the grace period
/// after it leaves both.
#[test]
fn the_bubble_stays_open_while_the_pointer_rests_on_it() {
  reset();

  let owner = Owner::new();
  let start = owner.enter(|| {
    Tooltip::new("Saves your work")
      .set_child(Container::new().set_child(Text::new("Save")))
      .build()
  });
  let (described, bubble) = parts(start);

  trigger_pointer_enter(described);
  trigger_frame(TOOLTIP_DELAY);

  assert!(is_open(bubble));

  trigger_pointer_leave(described);
  trigger_pointer_enter(bubble);
  trigger_frame(TOOLTIP_GRACE * 2);

  assert!(
    is_open(bubble),
    "open while the pointer rests on the bubble"
  );

  trigger_pointer_leave(bubble);
  trigger_pointer_enter(described);
  trigger_frame(TOOLTIP_GRACE * 2);

  assert!(is_open(bubble), "open again on the described view");

  trigger_pointer_leave(described);
  trigger_frame(TOOLTIP_GRACE);

  assert!(!is_open(bubble), "closed once the pointer left both");

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

  assert!(access().contains(&AccessProp::DescribedBy(vec![bubble])));
  assert!(access().contains(&AccessProp::Description("Saves".to_owned())));

  disabled.set(true);

  assert!(
    access().contains(&AccessProp::Disabled(true)),
    "the control's own report: {:?}",
    access()
  );
  assert!(
    access().contains(&AccessProp::DescribedBy(vec![bubble])),
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
    Some(vec![AccessProp::DescribedBy(vec![view_bubble])])
  );

  owner.dispose();
}

/// A control's own description and a tooltip's fold into one list, the control's
/// first, through a rewrite of the control's report.
#[test]
fn a_controls_description_and_a_tooltips_fold_into_one_list() {
  reset();

  let owner = Owner::new();
  let disabled = Signal::new(false);
  let (hint, start) = owner.enter(|| {
    let hint = Text::new("Required").build();
    let start = Tooltip::new("Saves")
      .set_child(
        RawButton::new(Activate::new())
          .derive_disabled(move || disabled.get())
          .set_described_by(hint),
      )
      .build();

    (hint, start)
  });
  let (button, bubble) = parts(start);
  let described = || {
    box_props(button)
      .map(|props| props.access)
      .unwrap_or_default()
      .into_iter()
      .filter(|prop| matches!(prop, AccessProp::DescribedBy(_)))
      .collect::<Vec<_>>()
  };

  assert_eq!(described(), [AccessProp::DescribedBy(vec![hint, bubble])]);

  disabled.set(true);

  assert_eq!(described(), [AccessProp::DescribedBy(vec![hint, bubble])]);

  owner.dispose();
}
