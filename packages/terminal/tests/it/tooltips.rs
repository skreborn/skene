//! Tooltips in cells: the bubble paints beside its anchor once the pointer has rested
//! for the delay, at once on focus that shows, stays while the pointer rests on it, and
//! leaves with the pointer, the focus or `Escape`; the node holding focus within the
//! described view reports the bubble and its text beside its own reports, as a reader
//! finds them in the accessibility tree.

use std::time::Duration;

use skene_terminal::{
  self as terminal,
  Activate,
  NodeId,
  TOOLTIP_DELAY,
  TOOLTIP_GRACE,
  prelude::*,
  testing,
};

use crate::common::with_platform;

/// What a reader finds `node` described by and its description, nothing where the tree
/// holds no node for it.
fn description(node: NodeId) -> (Vec<accesskit::NodeId>, Option<String>) {
  let id = node.access_id();

  testing::access_tree()
    .nodes
    .into_iter()
    .find(|(held, _)| *held == id)
    .map(|(_, access)| {
      (
        access.described_by().to_vec(),
        access.description().map(str::to_owned),
      )
    })
    .unwrap_or_default()
}

/// The tooltips a reader finds open, in tree order.
fn open_tooltips() -> Vec<accesskit::NodeId> {
  testing::access_tree()
    .nodes
    .into_iter()
    .filter(|(_, access)| access.role() == accesskit::Role::Tooltip)
    .map(|(id, _)| id)
    .collect()
}

/// A tooltip describes the node that takes focus within the view it wraps, not the
/// view's outer box, so a reader hears the description where focus lands.
#[test]
fn a_tooltip_describes_the_focus_holder_within_its_view() {
  with_platform(|| {
    let (outer, holder) = (NodeHandle::new(), NodeHandle::new());

    let _ = terminal::mount(move || {
      Tooltip::new("Saves your work")
        .set_child(
          Container::new().bind_handle(outer).set_child(
            Container::new()
              .bind_handle(holder)
              .set_focus_order(FocusOrder::Auto)
              .set_child(Text::new("Save")),
          ),
        )
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(24, 4);
    let node = |handle: NodeHandle| handle.node().expect("a bound box");
    let (described_by, text) = description(node(holder));

    assert_eq!(described_by.len(), 1, "the focus holder names the bubble");
    assert_eq!(text.as_deref(), Some("Saves your work"));
    assert_eq!(
      description(node(outer)),
      (vec![], None),
      "the outer box does not"
    );

    testing::trigger_key("Tab");

    assert_eq!(
      open_tooltips(),
      described_by,
      "the named node is the open bubble"
    );
  });
}

/// A click focuses a described button without opening its bubble at once; the rest
/// before the delay still opens it.
#[test]
fn a_click_leaves_the_opening_to_the_delay() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Tooltip::new("Saves your work")
            .set_placement(Placement::Bottom)
            .set_child(BasicButton::new(Activate::new()).set_label("save"))
            .into_any(),
          Text::new("below").into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(24, 4);

    testing::trigger_mouse_click(1, 0);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "focus from a click does not show"
    );

    testing::trigger_frame(TOOLTIP_DELAY);

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "the pointer rested"
    );
  });
}

/// A tooltip on a button opens below it after the delay while the pointer rests on
/// the button, closes the grace period after the pointer leaves, opens at once as the
/// button takes focus, and closes on `Escape`.
#[test]
fn a_tooltip_paints_beside_its_anchor_on_hover_and_focus() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Tooltip::new("Saves your work")
            .set_placement(Placement::Bottom)
            .set_child(BasicButton::new(Activate::new()).set_label("save"))
            .into_any(),
          Text::new("below").into_any(),
        ))
        .into_any()
        .build()
    });
    let frame = testing::render_to_text(24, 4);

    assert!(
      frame
        .lines()
        .next()
        .is_some_and(|line| line.contains("save")),
      "{frame}"
    );
    assert!(!frame.contains("Saves"), "{frame}");

    testing::trigger_mouse_move(1, 0);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "the pointer has not rested"
    );

    testing::trigger_frame(TOOLTIP_DELAY);

    let open = testing::render_to_text(24, 4);
    let lines: Vec<&str> = open.lines().collect();

    assert!(
      lines[1].contains("Saves your work"),
      "the bubble sits below: {open}"
    );

    testing::trigger_mouse_move(20, 3);

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "open through the grace period"
    );

    testing::trigger_frame(TOOLTIP_GRACE);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "the pointer left"
    );

    testing::trigger_key("Tab");

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "focus opens at once"
    );

    testing::trigger_key("Escape");

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "escape closed it"
    );

    testing::trigger_mouse_move(1, 0);
    testing::trigger_frame(Duration::from_millis(100));
    testing::trigger_mouse_move(20, 3);
    testing::trigger_frame(TOOLTIP_DELAY);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "a leave before the delay cancels"
    );
  });
}

/// The pointer can cross from the button onto the bubble, which stays open while the
/// pointer rests on it and closes the grace period after the pointer leaves both.
#[test]
fn a_tooltip_stays_open_while_the_pointer_rests_on_its_bubble() {
  with_platform(|| {
    let _ = terminal::mount(|| {
      Column::new()
        .set_children((
          Tooltip::new("Saves your work")
            .set_placement(Placement::Bottom)
            .set_child(BasicButton::new(Activate::new()).set_label("save"))
            .into_any(),
          Text::new("below").into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(24, 4);

    testing::trigger_mouse_move(1, 0);
    testing::trigger_frame(TOOLTIP_DELAY);

    let open = testing::render_to_text(24, 4);
    let column = open
      .lines()
      .nth(1)
      .and_then(|line| line.split("Saves").next())
      .map(|before| before.chars().count())
      .expect("the bubble");

    testing::trigger_mouse_move(u16::try_from(column).expect("a column"), 1);
    testing::trigger_frame(TOOLTIP_GRACE * 2);

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "open while the pointer rests on the bubble"
    );

    testing::trigger_mouse_move(1, 0);
    testing::trigger_frame(TOOLTIP_GRACE * 2);

    assert!(
      testing::render_to_text(24, 4).contains("Saves your work"),
      "open again on the button"
    );

    testing::trigger_mouse_move(20, 3);
    testing::trigger_frame(TOOLTIP_GRACE);

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "closed once the pointer left both"
    );
  });
}

/// Focus moving on closes the bubble focus opened, and a rest still pending when the
/// view leaves dies with it: the delay passing opens nothing.
#[test]
fn a_tooltip_closes_on_blur_and_its_pending_rest_leaves_with_it() {
  with_platform(|| {
    let shown = Signal::new(true);

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          derived(move || {
            if shown.get() {
              Tooltip::new("Saves your work")
                .set_placement(Placement::Bottom)
                .set_child(BasicButton::new(Activate::new()).set_label("save"))
                .into_any()
                .build()
            }
            else {
              Text::new("gone").build()
            }
          }),
          Text::new("").into_any().build(),
          BasicButton::new(Activate::new())
            .set_label("next")
            .into_any()
            .build(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(24, 4);

    testing::trigger_key("Tab");

    assert!(testing::render_to_text(24, 4).contains("Saves your work"));

    testing::trigger_key("Tab");

    assert!(
      !testing::render_to_text(24, 4).contains("Saves"),
      "blur closes it"
    );

    testing::trigger_mouse_move(1, 0);
    shown.set(false);
    testing::trigger_frame(TOOLTIP_DELAY * 2);

    let frame = testing::render_to_text(24, 4);

    assert!(
      frame.contains("gone") && !frame.contains("Saves"),
      "the rest left with the view: {frame}"
    );
  });
}

/// A tooltip around a reactive view hears the pointer and focus on the view's content,
/// opens against the content's box and describes the node holding focus in it, the
/// description following content rebuilt in place once focus reaches it.
#[test]
fn a_tooltip_around_a_reactive_view_hears_its_content() {
  with_platform(|| {
    let version = Signal::new(0);
    let content = NodeHandle::new();

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Text::new("top").into_any(),
          Tooltip::new("Saves your work")
            .set_placement(Placement::Bottom)
            .set_child(AnyView::new(move || {
              derived(move || {
                let _ = version.get();

                Container::new()
                  .bind_handle(content)
                  .set_focus_order(FocusOrder::Auto)
                  .set_child(Text::new("Save"))
                  .build()
              })
            }))
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(24, 5);
    let described = || {
      let (described_by, text) = description(content.node().expect("the content"));

      !described_by.is_empty() && text.as_deref() == Some("Saves your work")
    };

    assert!(described(), "the content's focus holder is described");

    testing::trigger_mouse_move(1, 1);
    testing::trigger_frame(TOOLTIP_DELAY);

    let frame = testing::render_to_text(24, 5);

    assert_eq!(
      testing::locate(&frame, "Saves your work").1,
      2,
      "resting on the content opens the bubble below it: {frame}"
    );

    testing::trigger_key("Escape");
    version.set(1);

    let frame = testing::render_to_text(24, 5);

    assert!(!frame.contains("Saves"), "{frame}");
    assert!(!described(), "the rebuilt content is not yet described");

    testing::trigger_key("Tab");

    assert_eq!(testing::focused_node(), content.node());
    assert!(described(), "focus moves the description onto it");
    assert!(
      testing::render_to_text(24, 5).contains("Saves your work"),
      "and opens the bubble"
    );
  });
}

/// A derived text tooltip re-describes as its text changes and its report sits beside
/// the control's own through a rewrite of either; a view tooltip reports only the
/// bubble.
#[test]
fn a_tooltips_report_sits_beside_the_controls_own() {
  with_platform(|| {
    let disabled = Signal::new(false);
    let text = Signal::new("Saves".to_owned());
    let (button, plain) = (NodeHandle::new(), NodeHandle::new());

    let _ = terminal::mount(move || {
      Column::new()
        .set_children((
          Tooltip::derive(move || text.get())
            .set_child(
              BasicButton::new(Activate::new())
                .bind_handle(button)
                .derive_disabled(move || disabled.get())
                .set_label("Save"),
            )
            .into_any(),
          Tooltip::view(Text::new("rich"))
            .set_child(
              Container::new()
                .bind_handle(plain)
                .set_child(Text::new("plain")),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(24, 4);
    let node = |handle: NodeHandle| handle.node().expect("a bound view");
    let disabled_state = || {
      let id = node(button).access_id();

      testing::access_tree()
        .nodes
        .into_iter()
        .find(|(held, _)| *held == id)
        .is_some_and(|(_, access)| access.is_disabled())
    };
    let (described_by, said) = description(node(button));

    assert_eq!(described_by.len(), 1);
    assert_eq!(said.as_deref(), Some("Saves"));

    disabled.set(true);

    assert!(disabled_state(), "the control's own report");
    assert_eq!(
      description(node(button)),
      (described_by.clone(), Some("Saves".to_owned())),
      "the description survives the control's rewrite"
    );

    text.set("Saves the document".to_owned());

    assert_eq!(
      description(node(button)),
      (described_by, Some("Saves the document".to_owned()))
    );
    assert!(disabled_state());

    let (view_described_by, view_said) = description(node(plain));

    assert_eq!(
      view_described_by.len(),
      1,
      "a view tooltip names its bubble"
    );
    assert_eq!(view_said, None, "and says no text of its own");
  });
}

/// A control's own description and a tooltip's fold into one list, the control's
/// first, through a rewrite of the control's report.
#[test]
fn a_controls_description_and_a_tooltips_fold_into_one_list() {
  with_platform(|| {
    let disabled = Signal::new(false);
    let button = NodeHandle::new();
    let mut hint = None;

    let _ = terminal::mount(|| {
      let required = Text::new("Required").build();

      hint = Some(required);

      Column::new()
        .set_children((
          AnyView::node(required),
          Tooltip::new("Saves")
            .set_child(
              BasicButton::new(Activate::new())
                .bind_handle(button)
                .derive_disabled(move || disabled.get())
                .set_label("Save")
                .set_described_by(required),
            )
            .into_any(),
        ))
        .into_any()
        .build()
    });
    let _ = testing::render_to_text(24, 4);
    let hint = hint.expect("the hint").access_id();
    let described_by = || description(button.node().expect("the button")).0;
    let folded = described_by();

    assert_eq!(folded.len(), 2, "{folded:?}");
    assert_eq!(folded[0], hint, "the control's own first");

    disabled.set(true);

    assert_eq!(described_by(), folded, "through the control's rewrite");

    testing::trigger_key("Tab");
    disabled.set(false);
    testing::trigger_key("Tab");

    assert_eq!(open_tooltips(), folded[1..], "the second is the bubble");
  });
}
