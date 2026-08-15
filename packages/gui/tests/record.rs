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
  core::{Color, Dimension, Prevent, Role},
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

/// A click trigger runs the view's handler and returns its verdict.
#[test]
fn click_trigger_runs_the_handler() {
  let clicks = Rc::new(Cell::new(0));
  let root = Container::new()
    .handle_click({
      let clicks = Rc::clone(&clicks);

      move || {
        clicks.set(clicks.get() + 1);

        Prevent::Platform
      }
    })
    .build();

  let prevention = record::trigger_click(root);

  assert_eq!(clicks.get(), 1);
  assert!(prevention.platform);
  assert!(!prevention.bubbling);
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

/// A view implementing `HasFocus` over a raw control forwards through it, the way the
/// Simple controls wrap their raw counterparts.
#[test]
fn has_focus_forwards_through_a_raw_control() {
  use skene_gui::HasFocus;

  #[must_use]
  struct Wrapped {
    raw: RawTextField,
  }

  impl HasFocus for Wrapped {
    fn focus_core(&mut self) -> &mut skene_gui::FocusCore {
      self.raw.focus_core()
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

/// A handler installed on a drawn control runs ahead of the control's own behavior:
/// breaking takes the event outright and carries its own verdict, continuing leaves the
/// control to act as though nothing were installed.
#[test]
fn an_installed_handler_runs_ahead_of_a_control() {
  use std::ops::ControlFlow;

  use skene_gui::{
    RawCheckbox,
    core::{KeyEvent, Modifiers},
  };

  let checked = Signal::new(false);
  let seen = Rc::new(Cell::new(0_usize));
  let counted = Rc::clone(&seen);
  let root = RawCheckbox::new()
    .bind_checked(checked)
    .handle_key_down(move |event: &KeyEvent| {
      counted.set(counted.get() + 1);

      if event.key == "Escape" {
        return ControlFlow::Break(Prevent::Bubbling);
      }

      ControlFlow::Continue(())
    })
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

  assert_eq!(seen.get(), 1, "the installed handler runs first");
  assert!(!checked.get(), "breaking takes the event from the control");
  assert!(prevention.bubbling, "the verdict is the handler's");
  assert!(
    !prevention.platform,
    "the control's own verdict is not reached"
  );

  let prevention = press(" ");

  assert_eq!(seen.get(), 2, "the installed handler sees every key");
  assert!(checked.get(), "continuing leaves the toggle to run");
  assert!(prevention.platform, "the verdict is now the control's");
}

/// A drawn slider reports its value and bounds, and the keys move it the way a
/// browser's range input does: arrows by a step, page keys by ten, Home and End to the
/// bounds, everything else left alone.
// Every expected value is a step multiple or a bound, so it is exactly representable
// and an exact comparison is the assertion rather than an approximation of it.
#[expect(clippy::float_cmp)]
#[test]
fn a_drawn_slider_reports_and_moves_on_keys() {
  use skene_gui::{
    RawSlider,
    core::{AccessProp, KeyEvent, Modifiers, Role},
  };

  let value = Signal::new(40.0f32);
  let root = RawSlider::new(0., 100.)
    .set_step(5.)
    .bind_value(value)
    .set_label("Volume")
    .set_child(Text::new("track"))
    .build();

  assert_eq!(record::role(root), Some(Role::Slider));
  assert_eq!(
    record::access(root),
    vec![
      AccessProp::Value(40.),
      AccessProp::ValueMin(0.),
      AccessProp::ValueMax(100.),
      AccessProp::Label("Volume".to_owned()),
    ],
  );

  let press = |key: &str| {
    record::trigger_key_down(
      root,
      &KeyEvent {
        key: key.into(),
        modifiers: Modifiers::default(),
      },
    )
  };

  let _ = press("ArrowRight");

  assert_eq!(value.get(), 45., "an arrow moves by one step");

  let _ = press("ArrowDown");

  assert_eq!(value.get(), 40., "down falls, as on the web");

  let _ = press("PageUp");

  assert_eq!(value.get(), 90., "a page key moves by ten steps");

  let _ = press("End");

  assert_eq!(value.get(), 100.);

  let _ = press("Home");

  assert_eq!(value.get(), 0.);

  let prevention = press("a");

  assert_eq!(value.get(), 0., "an unrelated key leaves it alone");
  assert!(!prevention.platform, "and does not swallow the default");
}

/// A radio group owns the selection, the arrow keys, and the single tab stop; members
/// find it through context, so wrapping one in a container does not detach it.
#[test]
fn a_drawn_radio_group_owns_its_members() {
  use skene_gui::{
    Container,
    RawRadio,
    RawRadioGroup,
    core::{AccessProp, FocusOrder, KeyEvent, Modifiers, Role, Toggle},
  };

  let chosen = Signal::new("mint".to_owned());
  let root = RawRadioGroup::new()
    .bind_chosen(chosen)
    .set_label("Flavor")
    .set_child(Column::new().set_children((
      RawRadio::new("mint").set_child(Text::new("Mint")),
      // Wrapped for layout: context reaches it all the same.
      Container::new().set_child(RawRadio::new("plum").set_child(Text::new("Plum"))),
      RawRadio::new("fig").set_child(Text::new("Fig")),
    )))
    .build();

  assert_eq!(record::role(root), Some(Role::RadioGroup));

  let column = record::children(root)[0];
  let mint = record::children(column)[0];
  let plum = record::children(record::children(column)[1])[0];
  let fig = record::children(column)[2];

  assert_eq!(
    record::role(plum),
    Some(Role::Radio),
    "the wrapped one still joined"
  );
  assert_eq!(
    record::access(mint),
    vec![AccessProp::Checked(Toggle::On)],
    "the bound member reports checked",
  );
  assert_eq!(record::access(plum), vec![AccessProp::Checked(Toggle::Off)]);

  // One tab stop: the chosen member, and nothing else.
  assert_eq!(record::focus_order(mint), Some(FocusOrder::Auto));
  assert_eq!(record::focus_order(plum), Some(FocusOrder::Skip));
  assert_eq!(record::focus_order(fig), Some(FocusOrder::Skip));

  let _ = record::trigger_key_down(
    mint,
    &KeyEvent {
      key: "ArrowDown".into(),
      modifiers: Modifiers::default(),
    },
  );

  assert_eq!(
    chosen.get(),
    "plum",
    "an arrow moves to the next member and chooses it"
  );
  assert_eq!(record::focus_order(mint), Some(FocusOrder::Skip));
  assert_eq!(
    record::focus_order(plum),
    Some(FocusOrder::Auto),
    "the tab stop follows"
  );

  let _ = record::trigger_key_down(
    fig,
    &KeyEvent {
      key: "ArrowDown".into(),
      modifiers: Modifiers::default(),
    },
  );

  assert_eq!(chosen.get(), "mint", "and wraps at the end");
}
