//! Headless view tests: the GUI facade's views realized against the headless platform,
//! built and driven without a browser.

#![cfg(skene_web_platform = "headless")]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_foundation::{
  Axis,
  Color,
  ColumnBase,
  ContainerBase,
  ControlBase,
  DecorationBase,
  Dimension,
  FlexBase,
  FlexChild,
  Layout,
  OverlayBase,
  PaintedBase,
  Property,
  PropertyKey,
  RowBase,
  TextBase,
};
use skene_web::{
  AsyncViewBase,
  BasicRadio,
  BasicRadioGroup,
  BasicTextField,
  Bubbling,
  Column,
  Container,
  DeferredBase,
  HasChild,
  HasChildren,
  LayoutBox,
  LoadState,
  Overlay,
  Radio,
  RadioGroup,
  Resource,
  Role,
  Row,
  Text,
  TextEntry,
  View,
  WebProperty,
  headless::{
    add_head_element,
    box_props,
    children,
    is_open,
    is_tab_stop,
    outline,
    pump_tasks,
    reset,
    resolve_color,
    resolve_length,
    text,
    trigger_change,
    trigger_click,
    trigger_dismiss,
  },
  prelude::{Get, Owner, Set, Signal},
};

const WIDTH: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Width);
const HEIGHT: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Height);
const COLOR: PropertyKey<WebProperty> = PropertyKey::Shared(Property::Background);

/// The nodes between a container's anchors: its content, in order.
fn content(node: NodeId) -> Vec<NodeId> {
  let nodes = children(node);

  nodes[1..nodes.len() - 1].to_vec()
}

use skene_web::NodeId;

/// A static tree records its structure: kinds, order, and text content; a wrapper's
/// anchors take no place in the layout's outline.
#[test]
fn tree_structure_is_recorded() {
  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Container::new()
      .set_role(Role::Navigation)
      .set_child(Column::new().set_children((
        Text::new("first"),
        Row::new().set_children((Text::new("second"),)),
      )))
      .build()
  });

  assert_eq!(
    outline(root),
    "Box\n  Box\n    Run \"first\"\n    Box\n      Run \"second\"\n"
  );
  assert_eq!(
    box_props(root).and_then(|props| props.role),
    Some(Role::Navigation)
  );

  owner.dispose();
}

/// A static property lands in the compiled props; a derived one writes its slot and
/// updates it on change.
#[test]
fn derived_property_updates_the_resolved_style() {
  reset();

  let owner = Owner::new();
  let width = Signal::new(100.0f32);
  let root = owner.enter(|| {
    Container::new()
      .set_height(40.)
      .derive_width(move || Dimension::Length(width.get()))
      .build()
  });

  assert_eq!(resolve_length(root, HEIGHT), Some(40.));
  assert_eq!(resolve_length(root, WIDTH), Some(100.));

  width.set(160.);

  assert_eq!(resolve_length(root, WIDTH), Some(160.));

  owner.dispose();
}

/// Derived text rewrites the run in place on change.
#[test]
fn derived_text_updates_the_run() {
  reset();

  let owner = Owner::new();
  let count = Signal::new(1);
  let root = owner.enter(|| {
    Container::new()
      .set_child(Text::derive(move || format!("count: {}", count.get())))
      .build()
  });
  let run = content(root)[0];

  assert_eq!(text(run).as_deref(), Some("count: 1"));

  count.set(2);

  assert_eq!(text(run).as_deref(), Some("count: 2"));
  assert_eq!(content(root)[0], run, "the same run, rewritten");

  owner.dispose();
}

/// A click runs the handlers deepest first; a box that stops bubbling keeps the event
/// from its ancestors, and no box states anything about the platform.
#[test]
fn click_bubbles_until_a_box_stops_it() {
  reset();

  let owner = Owner::new();
  let log = Rc::new(RefCell::new(Vec::new()));
  let note = |name: &'static str| {
    let log = Rc::clone(&log);

    move || {
      log.borrow_mut().push(name);
    }
  };
  let root = owner.enter(|| {
    Container::new()
      .handle_click(note("outer"))
      .set_child(
        Container::new()
          .handle_click({
            let note = note("middle");

            move || {
              note();

              Bubbling::Stop
            }
          })
          .set_child(Container::new().handle_click(note("inner"))),
      )
      .build()
  });
  let middle = content(root)[0];
  let inner = content(middle)[0];
  let verdict = trigger_click(inner);

  assert_eq!(*log.borrow(), ["inner", "middle"]);
  assert!(verdict.bubbling, "the middle box kept it");
  assert!(!verdict.platform, "and stated nothing about the platform");

  owner.dispose();
}

/// A change trigger commits the value into a text field's binding.
#[test]
fn change_trigger_commits_into_the_binding() {
  reset();

  let owner = Owner::new();
  let value = Signal::new(String::new());
  let field = owner.enter(|| BasicTextField::new(TextEntry::new().bind_value(value)).build());

  trigger_change(field, "typed");

  assert_eq!(value.get(), "typed");

  owner.dispose();
}

/// An overlay opens and closes with its bound signal, and a dismiss trigger requests
/// closing through the handler rather than closing on its own.
#[test]
fn overlay_open_follows_its_signal() {
  reset();

  let owner = Owner::new();
  let open = Signal::new(false);
  let root = owner.enter(|| {
    Container::new()
      .set_child(
        Overlay::modal()
          .bind_open(open)
          .set_child(Text::new("dialog")),
      )
      .build()
  });
  let overlay = content(root)[0];

  assert!(!is_open(overlay));

  open.set(true);

  assert!(is_open(overlay));

  trigger_dismiss(overlay);

  assert!(!open.get());
  assert!(!is_open(overlay));

  owner.dispose();
}

/// Keyed children keep the nodes whose keys stay and reorder them, rebuilding nothing.
#[test]
fn keyed_children_reuse_and_reorder() {
  reset();

  let owner = Owner::new();
  let items = Signal::new(vec![1, 2, 3]);
  let column = owner.enter(|| {
    Column::new()
      .derive_children(
        move || items.get(),
        |item: &i32| *item,
        |item| Text::new(item.to_string()),
      )
      .build()
  });
  let before = content(column);

  assert_eq!(before.len(), 3);

  items.set(vec![3, 1]);

  let after = content(column);

  assert_eq!(after.len(), 2);
  assert_eq!(after[0], before[2], "the kept node moved, not rebuilt");
  assert_eq!(after[1], before[0]);
  assert_eq!(text(after[0]).as_deref(), Some("3"));

  owner.dispose();
}

/// A radio group's roster orders its members by the tree, so the one tab stop follows
/// the choice and a member inserted first is first.
#[test]
fn roster_orders_members_in_tree_order() {
  reset();

  let owner = Owner::new();
  let chosen: Signal<Option<String>> = Signal::new(None);
  let items = Signal::transient(vec!["a", "b"]);
  let group = owner.enter(|| {
    BasicRadioGroup::new(RadioGroup::new().bind_chosen(chosen))
      .set_child(Column::new().derive_children(
        move || items.get(),
        |item| *item,
        |item| BasicRadio::new(Radio::new(item)).set_label(item),
      ))
      .build()
  });
  let column = children(group)[0];
  let member = |index: usize| content(column)[index];

  assert!(is_tab_stop(member(0)));
  assert!(!is_tab_stop(member(1)));

  let _ = trigger_click(member(1));

  assert_eq!(chosen.get().as_deref(), Some("b"));
  assert!(!is_tab_stop(member(0)));
  assert!(is_tab_stop(member(1)));

  chosen.set(None);
  items.set(vec!["z", "a", "b"]);

  assert!(is_tab_stop(member(0)));
  assert!(!is_tab_stop(member(1)));
  assert!(!is_tab_stop(member(2)));

  owner.dispose();
}

/// Decoration resolves as given.
#[test]
fn decoration_resolves_verbatim() {
  reset();

  let owner = Owner::new();
  let color = Color::srgb(0.25, 0.5, 0.75);
  let root = owner.enter(|| Container::new().set_background(color).build());

  assert_eq!(resolve_color(root, COLOR), Some(color));

  owner.dispose();
}

/// The equal-footing gate: a layout view written outside the facade, against its
/// public API alone, realizes the same box the built-in it parallels does.
#[test]
fn a_third_party_layout_view_matches_the_built_in() {
  #[must_use]
  struct Toolbar {
    base: LayoutBox<FlexChild>,
  }

  impl Toolbar {
    fn new() -> Self {
      Self {
        base: LayoutBox::new(Layout::Flex {
          axis: Axis::Horizontal,
          reversed: false,
        }),
      }
    }

    fn set_spacing(mut self, spacing: f32) -> Self {
      self.base.style_mut().spacing = Some(skene_web::Spacing::from(spacing).into());

      self
    }
  }

  impl skene_web::AnyKind for Toolbar {}

  impl skene_foundation::View<skene_web::Active> for Toolbar {
    fn build(self) -> NodeId {
      skene_web::View::build(self.base)
    }
  }

  reset();

  let owner = Owner::new();
  let clicks = Rc::new(Cell::new(0));
  let (toolbar, row) = owner.enter(|| {
    let _ = &clicks;

    (
      Toolbar::new().set_spacing(8.).build(),
      Row::new().set_spacing(8.).build(),
    )
  });
  let (toolbar, row) = (box_props(toolbar).unwrap(), box_props(row).unwrap());

  assert_eq!(toolbar.layout, row.layout);
  assert_eq!(toolbar.style, row.style);

  owner.dispose();
}

/// A resource loads through the headless executor: loading until the test pumps the
/// tasks, ready with its value once it has.
#[test]
fn a_resource_loads_when_the_tasks_are_pumped() {
  reset();

  let owner = Owner::new();
  let resource = owner.enter(|| Resource::new(|| async { 42u32 }));

  assert_eq!(resource.state(), LoadState::Loading);
  assert_eq!(resource.get(), None);
  assert!(pump_tasks());
  assert_eq!(resource.state(), LoadState::Ready);
  assert_eq!(resource.get(), Some(42));
  assert!(!pump_tasks());

  owner.dispose();
}

/// A resource installs the headless executor itself, as `spawn` does: on a thread
/// nothing reset, it loads once the tasks are pumped.
#[test]
fn a_resource_installs_the_executor_on_the_host() {
  let owner = Owner::new();
  let resource = owner.enter(|| Resource::new(|| async { 7u32 }));

  assert!(pump_tasks());
  assert_eq!(resource.get(), Some(7));

  owner.dispose();
}

/// A page's routes, for the host-side link and router tests.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Page {
  Home,
  About,
}

impl skene_web::router::Route for Page {
  fn parse(path: &str) -> Option<Self> {
    match path {
      "/" => Some(Self::Home),
      "/about" => Some(Self::About),
      _ => None,
    }
  }

  fn to_path(&self) -> String {
    match self {
      Self::Home => "/".to_owned(),
      Self::About => "/about".to_owned(),
    }
  }
}

/// A link is a tab stop carrying its route's path as destination; a plain click or
/// `Enter` navigates the router and suppresses the platform's own navigation, a
/// modified click, from a pointer or a key, leaves both to the platform.
#[test]
fn a_link_carries_its_destination_and_navigates_the_router() {
  use skene_foundation::{ClickEvent, KeyEvent, Modifiers};
  use skene_web::{
    Link,
    LinkBase,
    headless::{is_tab_stop, trigger_click_event, trigger_key_down},
  };

  reset();

  let owner = Owner::new();
  let (router, link) = owner.enter(|| {
    let router = skene_web::install_router(Page::Home);
    let link = Link::new(Page::About).set_child(Text::new("about")).build();

    (router, link)
  });

  assert_eq!(
    box_props(link).and_then(|props| props.destination),
    Some("/about".to_owned())
  );
  assert_eq!(outline(link), "Box\n  Run \"about\"\n");

  let verdict = trigger_click(link);

  assert!(verdict.platform, "a plain click is the router's");
  assert_eq!(router.route(), Page::About);

  router.navigate(Page::Home);

  for count in [1, 0] {
    let verdict = trigger_click_event(
      link,
      &ClickEvent {
        modifiers: Modifiers::CTRL,
        count,
      },
    );

    assert!(!verdict.platform, "a modified click is the platform's");
    assert_eq!(router.route(), Page::Home);
  }

  assert!(is_tab_stop(link));

  let verdict = trigger_key_down(link, &KeyEvent::new("Enter"));

  assert!(verdict.platform, "Enter on a link is the router's");
  assert_eq!(router.route(), Page::About);

  owner.dispose();
}

/// Off the server leaf a deferred subtree builds its content directly, between anchors
/// of its own.
#[test]
fn a_deferred_subtree_builds_its_content_on_the_host() {
  use skene_web::Deferred;

  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Container::new()
      .set_child(Deferred::new().set_child_builder(|| Text::new("late")))
      .build()
  });

  assert_eq!(outline(root), "Box\n  Run \"late\"\n");

  owner.dispose();
}

/// A lazy subtree mounts its content once its future completes on the headless queue.
#[test]
fn a_lazy_subtree_mounts_when_the_tasks_are_pumped() {
  use skene_web::AsyncView;

  reset();

  let owner = Owner::new();
  let root = owner.enter(|| {
    Container::new()
      .set_child(AsyncView::new().set_child_loader(|| async { Text::new("loaded") }))
      .build()
  });

  assert_eq!(outline(root), "Box\n");
  assert!(pump_tasks());
  assert_eq!(outline(root), "Box\n  Run \"loaded\"\n");

  owner.dispose();
}

/// A spawned task runs on the headless queue and ends with its owner.
#[test]
fn a_spawned_task_runs_when_pumped_and_ends_with_its_owner() {
  reset();

  let ran = Rc::new(Cell::new(0));
  let owner = Owner::new();

  owner.enter(|| {
    let ran = Rc::clone(&ran);

    skene_web::spawn(async move { ran.set(ran.get() + 1) });
  });

  assert_eq!(ran.get(), 0);
  assert!(pump_tasks());
  assert_eq!(ran.get(), 1);

  let disposed = Owner::new();

  disposed.enter(|| {
    let ran = Rc::clone(&ran);

    skene_web::spawn(async move { ran.set(ran.get() + 1) });
  });
  disposed.dispose();

  let _ = pump_tasks();

  assert_eq!(ran.get(), 1, "a disposed owner's task never runs");

  owner.dispose();
}

/// The head registrations reach the headless platform for a test to read back.
#[test]
fn head_registrations_are_recorded_on_the_host() {
  use skene_web::{
    head,
    headless::{HeadElement, document_language, document_title, head_elements},
    window,
  };

  reset();

  let owner = Owner::new();
  let title = Signal::new("Home".to_owned());

  owner.enter(|| {
    window::derive_title(move || format!("{} — skene", title.get()));
    head::add_meta("description", "A tiny page");
    head::add_stylesheet("/site.css");
    head::set_language("en");
  });

  assert_eq!(document_title().as_deref(), Some("Home — skene"));
  assert_eq!(document_language().as_deref(), Some("en"));

  title.set("About".to_owned());

  assert_eq!(document_title().as_deref(), Some("About — skene"));
  assert_eq!(
    head_elements(),
    vec![
      HeadElement::new(
        "meta",
        &[("name", "description"), ("content", "A tiny page")]
      ),
      HeadElement::new("link", &[("rel", "stylesheet"), ("href", "/site.css")]),
    ]
  );

  owner.dispose();

  assert_eq!(document_title(), None);
  assert_eq!(head_elements(), []);
}

/// A registration holds while its owner lives: a later owner's title and description
/// replace the earlier ones until it is disposed, and an identical stylesheet stays
/// while either registrant lives.
#[test]
fn head_registrations_leave_with_their_owner() {
  use skene_web::{
    head,
    headless::{HeadElement, document_title, head_elements},
    window,
  };

  reset();

  let outer = Owner::new();
  let inner = Owner::new();
  let description =
    |content| HeadElement::new("meta", &[("name", "description"), ("content", content)]);
  let stylesheet = HeadElement::new("link", &[("rel", "stylesheet"), ("href", "/site.css")]);

  outer.enter(|| {
    window::set_title("Outer");
    head::add_meta("description", "outer");
    head::add_stylesheet("/site.css");
  });
  inner.enter(|| {
    window::set_title("Inner");
    head::add_meta("description", "inner");
    head::add_stylesheet("/site.css");
  });

  assert_eq!(document_title().as_deref(), Some("Inner"));
  assert_eq!(head_elements(), [description("inner"), stylesheet.clone()]);

  inner.dispose();

  assert_eq!(document_title().as_deref(), Some("Outer"));
  assert_eq!(head_elements(), [description("outer"), stylesheet]);

  outer.dispose();

  assert_eq!(head_elements(), []);
}

/// A handle asked for its size only after its box built measures the box from then.
#[test]
fn a_handle_asked_for_its_size_late_measures_from_then() {
  use skene_foundation::Size;
  use skene_web::{NodeHandle, headless::trigger_size};

  reset();

  let owner = Owner::new();
  let handle = NodeHandle::new();
  let node = owner.enter(|| Container::new().bind_handle(handle).build());

  assert_eq!(handle.node(), Some(node));
  assert_eq!(handle.size(), None);

  trigger_size(node, Size::new(64., 32.));

  assert_eq!(handle.size(), Some(Size::new(64., 32.)));

  owner.dispose();
}

/// A handle bound to a box reports the box's measured size, tracked, and nothing
/// before the first measurement.
#[test]
fn a_size_handle_reports_the_measured_size() {
  use skene_foundation::Size;
  use skene_web::{NodeHandle, headless::trigger_size};

  reset();

  let owner = Owner::new();
  let handle = NodeHandle::new();
  let seen = Rc::new(Cell::new(0));
  let node = owner.enter(|| {
    let seen = Rc::clone(&seen);

    skene_web::reactive::Effect::new(move |_: Option<()>| {
      let _ = handle.size();

      seen.set(seen.get() + 1);
    });

    Container::new().bind_handle(handle).build()
  });

  assert_eq!(handle.size(), None);
  assert_eq!(seen.get(), 1);

  trigger_size(node, Size::new(320., 200.));

  assert_eq!(handle.size(), Some(Size::new(320., 200.)));
  assert_eq!(seen.get(), 2, "the measurement notified the effect");

  owner.dispose();
}

/// A painted box hands its painter to the platform and draws through vello: the pixels
/// the headless platform renders carry the fill.
/// A platform's renderer takes the canvas's steps in the order they were drawn.
#[test]
fn a_canvas_replays_its_scene_into_a_renderer() {
  use skene_foundation::web::Renderer;
  use skene_web::{
    Canvas,
    kurbo::{Affine, BezPath, Rect, Stroke},
    peniko::Brush,
  };

  #[derive(Default)]
  struct Log(Vec<String>);

  impl Renderer for Log {
    fn set_transform(&mut self, transform: Affine) {
      self
        .0
        .push(format!("transform {}", transform.as_coeffs()[0]));
    }

    fn fill(&mut self, path: &BezPath, brush: &Brush) {
      self.0.push(format!(
        "fill {} {}",
        path.elements().len(),
        matches!(brush, Brush::Solid(_))
      ));
    }

    fn stroke(&mut self, _: &BezPath, stroke: &Stroke, _: &Brush) {
      self.0.push(format!("stroke {}", stroke.width));
    }

    fn push_clip(&mut self, _: &BezPath) {
      self.0.push("clip".into());
    }

    fn pop(&mut self) {
      self.0.push("pop".into());
    }
  }

  let mut canvas = Canvas::new(4, 4);
  let rect = Rect::new(0., 0., 4., 4.);

  canvas.set_transform(Affine::scale(2.));
  canvas.push_clip(&rect);
  canvas.fill(&rect, Color::srgb(1., 0., 0.));
  canvas.stroke(&rect, &Stroke::new(3.), Color::srgb(0., 1., 0.));
  canvas.pop();

  let mut log = Log::default();

  canvas.replay(&mut log);

  assert_eq!(
    log.0,
    ["transform 2", "clip", "fill 5 true", "stroke 3", "pop"]
  );
}

#[test]
fn a_painted_box_draws_through_vello() {
  use skene_foundation::Size;
  use skene_web::{
    Painted,
    headless::{is_painted, render_to_pixels},
    kurbo::Rect,
  };

  reset();

  let owner = Owner::new();
  let node = owner.enter(|| {
    Painted::new(|canvas, size| {
      canvas.fill(
        &Rect::new(0., 0., f64::from(size.width), f64::from(size.height)),
        Color::srgb(1., 0., 0.),
      );
    })
    .set_width(Dimension::Length(8.))
    .set_height(Dimension::Length(4.))
    .build()
  });

  assert!(is_painted(node));

  let pixels = render_to_pixels(node, Size::new(8., 4.)).expect("a painted box");

  assert_eq!(pixels.len(), 8 * 4 * 4);
  assert_eq!(
    &pixels[0..4],
    &[255, 0, 0, 255],
    "the top-left pixel is the fill"
  );
  assert_eq!(
    &pixels[pixels.len() - 4..],
    &[255, 0, 0, 255],
    "so is the last"
  );

  owner.dispose();
}

/// A painted box with a fallback builds the fallback where nothing is drawn as pixels
/// and the painted box once something is.
#[test]
fn a_painted_box_falls_back_where_nothing_is_drawn_as_pixels() {
  use skene_foundation::Scene;
  use skene_web::{
    Painted,
    headless::{is_painted, trigger_graphics},
  };

  reset();
  trigger_graphics(false);

  let owner = Owner::new();
  let start = owner.enter(|| {
    Painted::new(|_, _| {})
      .set_fallback_builder(|| Text::new("a red box"))
      .build()
  });
  let nodes = || Scene::<skene_web::Active>::with(|scene| scene.segment_nodes(start));
  let shown = nodes();

  assert_eq!(shown.len(), 1);
  assert_eq!(text(shown[0]).as_deref(), Some("a red box"));
  assert!(!is_painted(shown[0]));

  trigger_graphics(true);

  let shown = nodes();

  assert_eq!(shown.len(), 1);
  assert!(
    is_painted(shown[0]),
    "the painted box took the fallback's place"
  );

  owner.dispose();
}

/// A head registration is an element a head holds, with real attribute names, as on
/// the web.
#[test]
#[should_panic(expected = "a head element is one of")]
fn a_head_registration_names_a_head_element() {
  reset();

  let _ = add_head_element("img", &[("src", "x")]);
}

/// An attribute name that is not one is refused, as on the web.
#[test]
#[should_panic(expected = "is not an attribute name")]
fn a_head_registration_refuses_an_attribute_that_is_not_one() {
  reset();

  let _ = add_head_element("meta", &[("onload=alert(1) x", "y")]);
}
