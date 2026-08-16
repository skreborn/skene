//! Headless view tests: the GUI facade's views realized against the recording driver,
//! built and driven without a browser.

#![cfg(not(target_arch = "wasm32"))]

use std::{cell::Cell, rc::Rc};

use skene_gui::{
  Column,
  Container,
  HasChildren,
  LayoutBox,
  Overlay,
  RawTextField,
  Row,
  Text,
  View,
  core::{Bubbling, Color, Dimension, Role},
  prelude::{Get, Set, Signal},
  record,
};

/// A static tree records its structure: kinds, order, and text content.
#[test]
fn tree_structure_is_recorded() {
  let root = Container::new()
    .set_role(Role::Navigation)
    .set_child(Column::new().set_children((
      Text::new("first"),
      Row::new().set_children((Text::new("second"),)),
    )))
    .build();

  assert_eq!(
    record::outline(root),
    "Box\n  Box\n    Run \"first\"\n    Box\n      Run \"second\"\n"
  );
  assert_eq!(record::role(root), Some(Role::Navigation));
}

/// A static style lands in the recorded props; a derived property updates it on
/// change.
#[test]
fn derived_property_updates_the_recorded_style() {
  let width = Signal::new(100.0f32);
  let root = Container::new()
    .set_height(40.)
    .derive_width(move || width.get())
    .build();
  let style = record::box_style(root).expect("a box style");

  assert_eq!(style.height, Some(Dimension::Length(40.)));
  assert_eq!(style.width, Some(Dimension::Length(100.)));

  width.set(160.);

  let style = record::box_style(root).expect("a box style");

  assert_eq!(style.width, Some(Dimension::Length(160.)));
}

/// Derived text rewrites the recorded run on change.
#[test]
fn derived_text_updates_the_recorded_run() {
  let count = Signal::new(1);
  let root = Container::new()
    .set_child(Text::derive(move || format!("count: {}", count.get())))
    .build();
  let run = record::children(root)[0];

  assert_eq!(record::text(run).as_deref(), Some("count: 1"));

  count.set(2);

  assert_eq!(record::text(run).as_deref(), Some("count: 2"));
}

/// A click trigger runs the view's handler; a box that handled the event keeps it.
#[test]
fn click_trigger_runs_the_handler() {
  let clicks = Rc::new(Cell::new(0));
  let root = Container::new()
    .handle_click({
      let clicks = Rc::clone(&clicks);

      move || {
        clicks.set(clicks.get() + 1);

        Bubbling::Stop
      }
    })
    .build();

  let prevention = record::trigger_click(root);

  assert_eq!(clicks.get(), 1);
  assert!(prevention.bubbling, "a box that handled it keeps it");
  assert!(
    !prevention.platform,
    "and states nothing about the platform, which is the driver's"
  );
}

/// A change trigger commits the value into an input's binding.
#[test]
fn change_trigger_commits_into_the_binding() {
  let value = Signal::new(String::new());
  let root = RawTextField::new().bind_value(value).build();

  record::trigger_change(root, "typed");

  assert_eq!(value.get(), "typed");
}

/// An overlay opens and closes with its bound signal, and a dismiss trigger requests
/// closing through the handler.
#[test]
fn overlay_open_follows_its_signal() {
  let open = Signal::new(false);
  let root = Container::new()
    .set_child(
      Overlay::modal()
        .bind_open(open)
        .set_children((Text::new("dialog"),)),
    )
    .build();
  let overlay = record::children(root)[0];

  assert_eq!(record::overlay_open(overlay), Some(false));

  open.set(true);

  assert_eq!(record::overlay_open(overlay), Some(true));

  record::trigger_dismiss(overlay);

  assert!(!open.get());
  assert_eq!(record::overlay_open(overlay), Some(false));
}

/// A keyed reconcile keeps unkeyed-change nodes and reorders the recorded children.
#[test]
fn reconcile_reuses_and_reorders_recorded_children() {
  let items = Signal::new(vec![1, 2, 3]);
  let parent = record::create_group();

  skene_gui::reconcile(
    parent,
    move || items.get(),
    |item: &i32| *item,
    |item| Text::new(format!("{item}")).build(),
  );

  let before = record::children(parent);

  assert_eq!(before.len(), 3);

  items.set(vec![3, 1]);

  let after = record::children(parent);

  assert_eq!(after.len(), 2);
  assert_eq!(after[0], before[2], "the kept node moved, not rebuilt");
  assert_eq!(after[1], before[0]);
  assert_eq!(record::text(after[0]).as_deref(), Some("3"));
}

/// Colors and backgrounds record as given, uninterpreted.
#[test]
fn decoration_records_verbatim() {
  let color = Color::srgb(0.25, 0.5, 0.75);
  let root = Container::new().set_color(color).build();
  let style = record::box_style(root).expect("a box style");

  assert_eq!(style.color, Some(color));
}

/// The equal-footing gate: a layout view written outside the facade, against its
/// public API alone, matches the built-in it parallels feature for feature—typed
/// children (a [`Flexible`] wrapper included) and its own layout knob—realizing an
/// identical node.
#[test]
fn a_third_party_layout_view_matches_the_built_in() {
  use skene_gui::{Flexible, core::FlexChild};

  #[must_use]
  struct Toolbar {
    base: LayoutBox<FlexChild>,
  }

  impl Toolbar {
    fn new() -> Self {
      Self {
        base: LayoutBox::new(skene_gui::Layout::Flex {
          direction: skene_gui::core::FlexDirection::Row,
        }),
      }
    }

    fn set_spacing(mut self, spacing: f32) -> Self {
      self.base = self.base.set_spacing(skene_gui::core::Spacing {
        row: 0.,
        column: spacing,
      });

      self
    }
  }

  impl HasChildren<skene_gui::GuiOps> for Toolbar {
    type Kind = FlexChild;

    fn set_children_to(mut self, children: skene_gui::Children) -> Self {
      self.base = self.base.set_children_to(children);

      self
    }
  }

  impl View for Toolbar {
    fn build(self) -> skene_gui::core::NodeId {
      self.base.build()
    }
  }

  let children = || {
    (
      Text::new("ab"),
      Flexible::new().set_child(Text::new("cd")),
      Text::new("ef"),
    )
  };
  let third_party = Toolbar::new()
    .set_spacing(4.)
    .set_children(children())
    .build();
  let built_in = Row::new().set_spacing(4.).set_children(children()).build();

  assert_eq!(record::layout(third_party), record::layout(built_in));
  assert_eq!(record::outline(third_party), record::outline(built_in));
  assert!(
    record::box_style(third_party) == record::box_style(built_in),
    "the layout knob lands the same on both"
  );

  let wrapped = |node| record::box_style(record::children(node)[1]);

  assert!(
    wrapped(third_party) == wrapped(built_in),
    "the `Flexible` wrapper lands the same on both"
  );
}

/// A view implementing the control surfaces over a raw control forwards through them,
/// the way the Simple controls wrap their raw counterparts.
#[test]
fn has_focus_forwards_through_a_raw_control() {
  use skene_gui::{HasEvents, HasFocus};

  #[must_use]
  struct Wrapped {
    raw: RawTextField,
  }

  impl HasFocus for Wrapped {
    fn focus_core(&mut self) -> &mut skene_gui::FocusCore {
      self.raw.focus_core()
    }
  }

  impl skene_gui::HasEvents for Wrapped {
    fn event_core(&mut self) -> &mut skene_gui::EventCore {
      self.raw.event_core()
    }
  }

  impl View for Wrapped {
    fn build(self) -> skene_gui::core::NodeId {
      self.raw.build()
    }
  }

  let pressed = Rc::new(Cell::new(false));
  let seen = Rc::clone(&pressed);
  let root = Wrapped {
    raw: RawTextField::new(),
  }
  .set_autofocus()
  .handle_key_down(move |_| seen.set(true))
  .build();

  let _ = record::trigger_key_down(
    root,
    &skene_gui::core::KeyEvent {
      key: "a".into(),
      modifiers: skene_gui::core::Modifiers::default(),
    },
  );

  assert!(pressed.get(), "the key handler forwards to the control");
}

/// A widget role and the state it reports land on the node together: the role says what
/// the control is, the accessibility props say what it currently reports.
#[test]
fn a_widget_reports_its_role_and_state() {
  use skene_gui::core::{AccessProp, Role, Toggle};

  let checked = Signal::new(false);
  let root = Container::new()
    .set_role(Role::Checkbox)
    .derive_accessibility(move || vec![AccessProp::Checked(checked.get().into())])
    .build();

  assert_eq!(record::role(root), Some(Role::Checkbox));
  assert_eq!(record::access(root), vec![AccessProp::Checked(Toggle::Off)]);

  checked.set(true);

  assert_eq!(
    record::access(root),
    vec![AccessProp::Checked(Toggle::On)],
    "the reported state follows the signal, and does not accumulate",
  );
}

/// A drawn checkbox supplies behavior and reports state; the child supplies the pixels.
/// Its node is a plain box carrying the checkbox role, not a native control.
#[test]
fn a_drawn_checkbox_reports_and_toggles() {
  use skene_gui::{
    RawCheckbox,
    core::{AccessProp, Role, Toggle},
  };

  let checked = Signal::new(false);
  let root = RawCheckbox::new()
    .bind_checked(checked)
    .set_label("Subscribed")
    .set_child(Text::new("mark"))
    .build();

  assert_eq!(record::role(root), Some(Role::Checkbox));
  assert_eq!(record::outline(root), "Box\n  Run \"mark\"\n");
  assert_eq!(
    record::access(root),
    vec![
      AccessProp::Checked(Toggle::Off),
      AccessProp::Label("Subscribed".to_owned()),
    ],
  );

  let _ = record::trigger_click(root);

  assert!(checked.get(), "a click writes the bound signal back");
  assert_eq!(
    record::access(root),
    vec![
      AccessProp::Checked(Toggle::On),
      AccessProp::Label("Subscribed".to_owned()),
    ],
    "and the reported state follows",
  );
}

/// Space toggles a drawn checkbox and Enter does not, matching the browser's own.
#[test]
fn a_drawn_checkbox_toggles_on_space_only() {
  use skene_gui::{RawCheckbox, core::Modifiers};

  let checked = Signal::new(false);
  let root = RawCheckbox::new()
    .bind_checked(checked)
    .set_child(Text::new("mark"))
    .build();
  let press = |key: &str| {
    record::trigger_key_down(
      root,
      &skene_gui::core::KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };

  let prevention = press(" ");

  assert!(checked.get());
  assert!(prevention.platform, "the page must not scroll under it");

  let _ = press("Enter");

  assert!(checked.get(), "Enter leaves a checkbox alone");
}

/// A handler on a drawn control observes without displacing it: the control still
/// acts, and what the dispatch is told to prevent stays the control's decision.
#[test]
fn a_handler_observes_without_displacing_the_control() {
  use skene_gui::{
    HasEvents,
    RawCheckbox,
    core::{KeyEvent, Modifiers},
  };

  let checked = Signal::new(false);
  let seen = Rc::new(Cell::new(0_usize));
  let counted = Rc::clone(&seen);
  let root = RawCheckbox::new()
    .bind_checked(checked)
    .handle_key_down(move |_| counted.set(counted.get() + 1))
    .set_child(Text::new("mark"))
    .build();
  let press = |key: &str| {
    record::trigger_key_down(
      root,
      &KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };

  let prevention = press("Escape");

  assert_eq!(seen.get(), 1, "the handler sees a key the control ignores");
  assert!(!checked.get());
  assert!(
    !prevention.platform,
    "an ignored key prevents nothing, handler or no handler"
  );

  let prevention = press(" ");

  assert_eq!(seen.get(), 2, "and sees the ones it acts on");
  assert!(checked.get(), "which still toggle");
  assert!(prevention.platform, "under the control's own verdict");
}

/// A control carries a handler for every event, including the ones it does not act on,
/// so a handler always goes on the control rather than on a wrapper around it.
#[test]
fn a_control_carries_handlers_for_events_it_ignores() {
  use skene_gui::{
    HasEvents,
    RawCheckbox,
    core::{Modifiers, PointerEvent},
  };

  let seen = Rc::new(Cell::new(false));
  let noticed = Rc::clone(&seen);
  let root = RawCheckbox::new()
    .handle_pointer_down(move |_| noticed.set(true))
    .set_child(Text::new("mark"))
    .build();

  let _ = record::trigger_pointer_down(
    root,
    &PointerEvent {
      x: 0.,
      y: 0.,
      local_x: 0.,
      local_y: 0.,
      button: None,
      modifiers: Modifiers::default(),
    },
  );

  assert!(
    seen.get(),
    "a checkbox acts on no pointer press, yet carries"
  );
}

/// A drawn picker opens on a click, moves its highlight and its single tab stop with
/// the arrows, and commits only on request—so a dismissal leaves the value alone.
#[test]
fn a_drawn_picker_opens_moves_and_commits() {
  use skene_gui::{
    RawOption,
    RawPicker,
    core::{AccessProp, FocusOrder, KeyEvent, Modifiers},
  };

  let flavor = Signal::new(String::from("mint"));
  let root = RawPicker::new()
    .bind_value(flavor)
    .set_label("Flavor")
    .set_face(Text::new("choose"))
    .set_options((
      RawOption::new("mint").set_child(Text::new("Mint")),
      Text::new("— seasonal —"),
      Container::new().set_child(RawOption::new("plum").set_child(Text::new("Plum"))),
      RawOption::new("fig").set_child(Text::new("Fig")),
    ))
    .build();
  let face = record::children(root)[0];
  let overlay = record::children(root)[1];
  let listbox = record::children(overlay)[0];
  let column = record::children(listbox)[0];
  let listed = record::children(column);
  // The second option sits inside a wrapper and the plain text is no option at all, so
  // the list is reached by structure rather than by position.
  let rows = [listed[0], record::children(listed[2])[0], listed[3]];
  let press = |node, key: &str| {
    record::trigger_key_down(
      node,
      &KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };
  let tab_stops = || {
    rows
      .iter()
      .filter(|node| record::focus_order(**node) == Some(FocusOrder::Auto))
      .count()
  };

  assert_eq!(record::role(face), Some(Role::Combobox));
  assert_eq!(record::role(listbox), Some(Role::Listbox));
  assert!(
    rows
      .iter()
      .all(|node| record::role(*node) == Some(Role::ListboxOption)),
    "the wrapped option is an option like any other"
  );
  assert_eq!(
    record::role(listed[1]),
    None,
    "the interleaved text is no option"
  );
  assert!(
    record::access(face).contains(&AccessProp::Expanded(false)),
    "a picker reports itself closed"
  );
  assert_eq!(tab_stops(), 0, "a closed list holds no tab stop");

  let _ = record::trigger_click(face);

  assert_eq!(record::overlay_open(overlay), Some(true));
  assert!(record::access(face).contains(&AccessProp::Expanded(true)));
  assert_eq!(tab_stops(), 1, "the open list holds exactly one");
  assert_eq!(
    record::focus_order(rows[0]),
    Some(FocusOrder::Auto),
    "opening highlights the chosen option"
  );

  let prevention = press(rows[0], "ArrowDown");

  assert!(prevention.platform, "the page must not scroll under it");
  assert_eq!(
    record::focus_order(rows[1]),
    Some(FocusOrder::Auto),
    "the tab stop follows the highlight"
  );
  assert_eq!(flavor.get(), "mint", "moving the highlight commits nothing");

  let _ = press(rows[1], "ArrowDown");
  let _ = press(rows[2], "ArrowDown");

  assert_eq!(
    record::focus_order(rows[2]),
    Some(FocusOrder::Auto),
    "the highlight clamps at the end rather than wrapping"
  );

  let _ = press(rows[2], "Enter");

  assert_eq!(flavor.get(), "fig", "Enter commits the highlight");
  assert_eq!(record::overlay_open(overlay), Some(false));
  assert!(record::access(face).contains(&AccessProp::Expanded(false)));
  assert!(
    record::access(rows[2]).contains(&AccessProp::Selected(true)),
    "the committed option reports itself selected"
  );

  let _ = record::trigger_click(face);
  let _ = press(rows[2], "ArrowUp");
  record::trigger_dismiss(overlay);

  assert_eq!(flavor.get(), "fig", "a dismissal leaves the value alone");
  assert_eq!(record::overlay_open(overlay), Some(false));
}

/// A closed picker answers the arrows by changing the value outright, without opening,
/// the way a closed native select does.
#[test]
fn a_closed_picker_cycles_without_opening() {
  use skene_gui::{
    RawOption,
    RawPicker,
    core::{KeyEvent, Modifiers},
  };

  let flavor = Signal::new(String::from("mint"));
  let root = RawPicker::new()
    .bind_value(flavor)
    .set_face(Text::new("choose"))
    .set_options((
      RawOption::new("mint").set_child(Text::new("Mint")),
      RawOption::new("plum").set_child(Text::new("Plum")),
      RawOption::new("fig").set_child(Text::new("Fig")),
    ))
    .build();
  let face = record::children(root)[0];
  let overlay = record::children(root)[1];
  let press = |key: &str| {
    record::trigger_key_down(
      face,
      &KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };

  let _ = press("ArrowDown");

  assert_eq!(flavor.get(), "plum");
  assert_eq!(record::overlay_open(overlay), Some(false), "still closed");

  let _ = press("End");

  assert_eq!(flavor.get(), "fig");

  let _ = press("ArrowDown");

  assert_eq!(flavor.get(), "fig", "the value clamps at the last option");

  let _ = press("Home");

  assert_eq!(flavor.get(), "mint");
}

/// A member that reconciles away leaves its group, so the arrows never reach a
/// destroyed node and the wrap lands on one that still exists.
#[test]
fn a_reconciled_away_member_leaves_its_group() {
  use skene_gui::{
    RawRadio,
    RawRadioGroup,
    core::{KeyEvent, Modifiers},
  };

  let flavors = Signal::new(vec![
    String::from("mint"),
    String::from("plum"),
    String::from("fig"),
  ]);
  let chosen = Signal::new(String::from("mint"));
  let root = RawRadioGroup::new()
    .bind_chosen(chosen)
    .set_child(Column::new().derive_children(
      move || flavors.get(),
      |flavor: &String| flavor.clone(),
      |flavor| RawRadio::new(flavor.clone()).set_child(Text::new(flavor)),
    ))
    .build();
  let column = record::children(root)[0];
  let press = |node, key: &str| {
    record::trigger_key_down(
      node,
      &KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };

  assert_eq!(record::children(column).len(), 3);

  let _ = press(record::children(column)[0], "ArrowDown");

  assert_eq!(chosen.get(), "plum", "the arrows walk the built members");

  chosen.set(String::from("mint"));
  flavors.set(vec![String::from("mint"), String::from("fig")]);

  let rows = record::children(column);

  assert_eq!(rows.len(), 2, "the middle member is gone from the tree");

  let _ = press(rows[0], "ArrowDown");

  assert_eq!(
    chosen.get(),
    "fig",
    "and gone from traversal—a stale roster would choose the destroyed member"
  );

  let _ = press(rows[1], "ArrowDown");

  assert_eq!(chosen.get(), "mint", "the ring wraps over what remains");
}

/// A filtered-away option leaves the picker, so the highlight clamps at the end of what
/// is left rather than running past it onto a destroyed row.
#[test]
fn a_filtered_away_option_leaves_its_picker() {
  use skene_gui::{
    RawOption,
    RawPicker,
    core::{FocusOrder, KeyEvent, Modifiers},
  };

  let shown = Signal::new(vec![
    String::from("mint"),
    String::from("plum"),
    String::from("fig"),
  ]);
  let flavor = Signal::new(String::from("mint"));
  let root = RawPicker::new()
    .bind_value(flavor)
    .set_face(Text::new("choose"))
    .set_options((Column::new().derive_children(
      move || shown.get(),
      |flavor: &String| flavor.clone(),
      |flavor| RawOption::new(flavor.clone()).set_child(Text::new(flavor)),
    ),))
    .build();
  let face = record::children(root)[0];
  let overlay = record::children(root)[1];
  let listbox = record::children(overlay)[0];
  let rows = || record::children(record::children(record::children(listbox)[0])[0]);
  let press = |node, key: &str| {
    record::trigger_key_down(
      node,
      &KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };
  let highlighted = || {
    rows()
      .into_iter()
      .position(|node| record::focus_order(node) == Some(FocusOrder::Auto))
  };

  let _ = record::trigger_click(face);

  assert_eq!(rows().len(), 3);
  assert_eq!(
    highlighted(),
    Some(0),
    "opening highlights the chosen option"
  );

  shown.set(vec![String::from("mint"), String::from("plum")]);

  assert_eq!(rows().len(), 2, "the last option is gone from the tree");

  let _ = press(rows()[0], "ArrowDown");
  let _ = press(rows()[1], "ArrowDown");

  assert_eq!(
    highlighted(),
    Some(1),
    "the highlight clamps at what is left—a stale roster would run past it"
  );
}

/// A member inserted into the middle of a reconciling set takes its place in traversal,
/// because the order comes from the tree rather than from the order members joined.
#[test]
fn a_member_inserted_in_the_middle_traverses_in_document_order() {
  use skene_gui::{
    RawRadio,
    RawRadioGroup,
    core::{KeyEvent, Modifiers},
  };

  let flavors = Signal::new(vec![String::from("mint"), String::from("fig")]);
  let chosen = Signal::new(String::from("mint"));
  let root = RawRadioGroup::new()
    .bind_chosen(chosen)
    .set_child(Column::new().derive_children(
      move || flavors.get(),
      |flavor: &String| flavor.clone(),
      |flavor| RawRadio::new(flavor.clone()).set_child(Text::new(flavor)),
    ))
    .build();
  let column = record::children(root)[0];
  let press = |node, key: &str| {
    record::trigger_key_down(
      node,
      &KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };

  let _ = press(record::children(column)[0], "ArrowDown");

  assert_eq!(chosen.get(), "fig");

  chosen.set(String::from("mint"));
  flavors.set(vec![
    String::from("mint"),
    String::from("plum"),
    String::from("fig"),
  ]);

  let rows = record::children(column);

  assert_eq!(rows.len(), 3);

  let _ = press(rows[0], "ArrowDown");

  assert_eq!(
    chosen.get(),
    "plum",
    "the newcomer sits where the tree puts it, not where it joined"
  );

  let _ = press(rows[1], "ArrowDown");

  assert_eq!(chosen.get(), "fig");

  let _ = press(rows[2], "ArrowDown");

  assert_eq!(chosen.get(), "mint", "and the ring still wraps");
}

/// An enhancement renders its child once the client owns the page—which, on a driver
/// with no server tree to reproduce, is from the start.
#[test]
fn an_enhancement_renders_its_child_when_the_client_owns_the_page() {
  use skene_gui::{Enhanced, RawCheckbox, WebCheckbox};

  let root = Container::new()
    .set_child(
      Enhanced::new(WebCheckbox::new).set_child(|| RawCheckbox::new().set_child(Text::new("mark"))),
    )
    .build();
  let built = record::children(root);

  assert_eq!(built.len(), 1, "one form is rendered, never both");
  assert_eq!(
    record::role(record::children(built[0])[0]),
    Some(Role::Checkbox),
    "and it is the drawn one, not the browser's own"
  );
}

/// A Simple checkbox is drawn from ordinary views once the client owns the page: the
/// control reports the checkbox role, and the box beside its caption is a box.
#[test]
fn a_simple_checkbox_is_drawn_from_ordinary_views() {
  use skene_gui::{SimpleCheckbox, core::AccessProp};

  let checked = Signal::new(true);
  let root = Container::new()
    .set_child(
      SimpleCheckbox::new()
        .bind_checked(checked)
        .set_label("Ship"),
    )
    .build();
  let control = record::children(record::children(root)[0])[0];

  assert_eq!(record::role(control), Some(Role::Checkbox));
  assert!(record::access(control).contains(&AccessProp::Checked(true.into())));
  assert!(
    record::outline(control).contains("Run \"Ship\""),
    "the caption is drawn beside the box: {}",
    record::outline(control)
  );
}

/// A Simple radio group draws its members and owns their selection, wherever they sit.
#[test]
fn a_simple_radio_group_draws_and_owns_its_members() {
  use skene_gui::{SimpleRadio, SimpleRadioGroup, core::AccessProp};

  let flavor = Signal::new(String::from("mint"));
  let root = SimpleRadioGroup::new("flavor")
    .bind_chosen(flavor)
    .set_child(Column::new().set_children((
      SimpleRadio::new("mint").set_label("Mint"),
      Container::new().set_child(SimpleRadio::new("plum").set_label("Plum")),
    )))
    .build();

  assert_eq!(record::role(root), Some(Role::RadioGroup));

  let column = record::children(root)[0];
  let mint = record::children(record::children(column)[0])[0];
  let plum = record::children(record::children(record::children(column)[1])[0])[0];

  assert_eq!(record::role(mint), Some(Role::Radio));
  assert_eq!(
    record::role(plum),
    Some(Role::Radio),
    "a wrapped member is a member"
  );
  assert!(record::access(mint).contains(&AccessProp::Checked(true.into())));
  assert!(record::access(plum).contains(&AccessProp::Checked(false.into())));
}

/// A Simple slider is drawn from ordinary views: the control reports the slider role and
/// its bounds, and the fill follows the value with no platform pseudo-element involved.
#[test]
fn a_simple_slider_is_drawn_from_ordinary_views() {
  use skene_gui::{SimpleSlider, core::AccessProp};

  let volume = Signal::new(25.0f32);
  let root = Container::new()
    .set_child(SimpleSlider::new(0., 100.).bind_value(volume))
    .build();
  let control = record::children(record::children(root)[0])[0];

  assert_eq!(record::role(control), Some(Role::Slider));
  assert!(record::access(control).contains(&AccessProp::ValueMax(100.)));

  let stack = record::children(record::children(control)[0])[0];
  let fill = record::children(record::children(stack)[0])[0];

  assert_eq!(
    record::box_style(fill).and_then(|style| style.width),
    Some(Dimension::Fraction(0.25)),
    "the filled run is a quarter of the track"
  );

  volume.set(75.);

  assert_eq!(
    record::box_style(fill).and_then(|style| style.width),
    Some(Dimension::Fraction(0.75)),
    "and follows the value"
  );
}
