//! Hydration: the client adopts a server-rendered DOM instead of rebuilding it.
//!
//! The server's markup is hand-written here with the `data-h` markers and anchor
//! comments a real `render_to_string` emits, so the client can match each node it
//! builds to the server node carrying its marker.
//!
//! Run in a headless browser:
//! `cargo test -p skene-web --target wasm32-unknown-unknown --test browser_hydrate`.
#![cfg(skene_web_platform = "browser")]

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

use skene_web::{
  AsyncView,
  AsyncViewBase,
  BasicListboxOption,
  BasicPicker,
  BasicPickerBase,
  Column,
  ColumnBase,
  Container,
  ContainerBase,
  ControlBase,
  DecorationBase,
  Enhance,
  EnhanceBase,
  HasChild,
  HasChildren,
  Label,
  ListboxOption,
  Resource,
  Text,
  TextBase,
  Tooltip,
  TooltipBase,
  platform::{self, hydrate},
  reactive::Get,
  testing::{document, only_child, tick},
  wasm_bindgen::JsCast,
};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use web_sys::{Element, HtmlElement};

wasm_bindgen_test_configure!(run_in_browser);

/// A detached root holding `markup` as its server-rendered DOM.
fn server_root(markup: &str) -> Element {
  let root = document().create_element("div").expect("create root div");

  root.set_inner_html(markup);

  root
}

/// The number of descendant elements under `root`.
fn element_count(root: &Element) -> u32 {
  root
    .query_selector_all("*")
    .expect("query descendants")
    .length()
}

/// The markup and the stylesheet the server leaf would have produced for `app`:
/// built once in the browser, every element marked in document order, then torn
/// down.
fn served(app: impl FnOnce() -> skene_web::AnyView + 'static) -> (String, String) {
  let root = server_root("");

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");
  platform::install(root.clone());

  let _ = platform::mount(|| skene_web::View::build(app()));

  let elements = root.query_selector_all("*").expect("query the elements");

  for index in 0..elements.length() {
    if let Some(element) = elements
      .item(index)
      .and_then(|node| node.dyn_into::<Element>().ok())
    {
      let _ = element.set_attribute("data-h", &(index + 1).to_string());
    }
  }

  let styles = document()
    .query_selector_all("style[data-skene-style]")
    .expect("query the styles");
  let styles: String = (0..styles.length())
    .filter_map(|index| styles.item(index))
    .filter_map(|style| style.text_content())
    .collect();
  let markup = root.inner_html();

  platform::uninstall();
  root.remove();

  (markup, styles)
}

/// A scrollable box the reader scrolled and a box the reader focused before the bundle
/// arrived keep their offset and focus through hydration.
#[wasm_bindgen_test]
fn hydrate_keeps_the_readers_scroll_and_focus() {
  let app = || {
    Container::new()
      .set_height(40.)
      .set_overflow(skene_foundation::Overflow::ScrollIfNeeded)
      .set_child(
        skene_web::Column::new().set_children((
          Container::new()
            .set_min_height(20.)
            .set_focus_order(skene_web::FocusOrder::Auto),
          Container::new().set_min_height(200.),
        )),
      )
  };
  let (markup, styles) = served(move || skene_web::View::into_any(app()));
  let block = document().create_element("style").expect("a style");

  let _ = block.set_attribute("id", "skene-styles");
  block.set_text_content(Some(&styles));
  document()
    .head()
    .expect("a head")
    .append_child(&block)
    .expect("append the block");

  let root = server_root(&markup);

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");

  let scroller = only_child(&root);
  let focusable = scroller
    .query_selector("[tabindex]")
    .expect("query")
    .expect("the focusable box");

  // Focus first: focusing scrolls the box into view, which would undo the offset.
  focusable
    .dyn_ref::<HtmlElement>()
    .expect("an HTML element")
    .focus()
    .expect("focus the box");
  scroller.set_scroll_top(30);

  assert_eq!(scroller.scroll_top(), 30);

  hydrate(root.clone(), "", app);

  assert_eq!(platform::hydration_mismatches(), 0);
  assert!(only_child(&root) == scroller, "the scroller was adopted");
  assert_eq!(scroller.scroll_top(), 30, "the offset survived hydration");
  assert_eq!(
    document().active_element(),
    Some(focusable),
    "the focus survived hydration",
  );

  platform::uninstall();
  root.remove();
  block.remove();
}

/// A container's static child sits between the anchors of a segment of its own: the
/// container builds as element 1, its text as element 2, around the first segment.
#[wasm_bindgen_test]
fn hydrate_adopts_the_marked_elements() {
  let root =
    server_root("<div data-h=\"1\"><!--s:0--><span data-h=\"2\">Hi</span><!--e:0--></div>");
  let server_container = only_child(&root);
  let server_span = only_child(&server_container);

  hydrate(root.clone(), "", || {
    Container::new().set_child(Text::new("Hi"))
  });

  let hydrated_container = only_child(&root);

  assert_eq!(platform::hydration_mismatches(), 0);
  assert!(
    server_container == hydrated_container,
    "the container was adopted, not recreated",
  );

  assert!(
    server_span == only_child(&hydrated_container),
    "the child span was adopted too",
  );

  assert_eq!(
    element_count(&root),
    2,
    "no elements were duplicated by hydration",
  );
  assert_eq!(
    hydrated_container.child_nodes().length(),
    3,
    "the anchors were adopted rather than added",
  );

  platform::uninstall();
}

#[wasm_bindgen_test]
fn a_hydrated_handler_fires_on_the_adopted_node() {
  let root = server_root("<div data-h=\"1\"></div>");

  let clicked = Rc::new(Cell::new(false));

  hydrate(root.clone(), "", {
    let clicked = Rc::clone(&clicked);

    move || Container::new().handle_click(move || clicked.set(true))
  });

  only_child(&root)
    .dyn_into::<HtmlElement>()
    .expect("an HtmlElement")
    .click();

  assert!(
    clicked.get(),
    "the hydrated handler fired on the server-rendered node",
  );

  platform::uninstall();
}

#[wasm_bindgen_test]
fn a_resource_adopts_server_state_without_loading() {
  let root = server_root("<span data-h=\"1\">x</span>");

  let loaded = Rc::new(Cell::new(false));
  let captured: Rc<RefCell<Option<Resource<String>>>> = Rc::new(RefCell::new(None));

  // The server's state: one order-keyed JSON-encoded value, "Ada", and no tagged ones.
  hydrate(root.clone(), r#"[["\"Ada\""],[],[]]"#, {
    let loaded = Rc::clone(&loaded);
    let captured = Rc::clone(&captured);

    move || {
      let data = Resource::new({
        let loaded = Rc::clone(&loaded);

        move || {
          let loaded = Rc::clone(&loaded);

          async move {
            loaded.set(true);

            "loaded".to_owned()
          }
        }
      });

      *captured.borrow_mut() = Some(data);

      Text::new("hydrated")
    }
  });

  let data = (*captured.borrow()).expect("the resource was captured");

  assert!(
    !loaded.get(),
    "the loader did not run; the server value was adopted",
  );

  data.with(|value| {
    assert_eq!(
      value.map(String::as_str),
      Some("Ada"),
      "the adopted value is the server's, not the loader's",
    );
  });

  platform::uninstall();
}

/// An enhancement builds its fallback while hydrating, adopting the server's markup
/// for it with no mismatch, and rebuilds into its child once hydration has finished.
#[wasm_bindgen_test]
fn an_enhancement_adopts_its_fallback_then_builds_its_child() {
  let root = server_root("<!--s:0--><span data-h=\"1\">baseline</span><!--e:0-->");
  let children = Rc::new(Cell::new(0));

  hydrate(root.clone(), "", {
    let children = Rc::clone(&children);

    move || {
      assert!(
        !skene_web::media::hydrated().get(),
        "not hydrated while adopting"
      );

      Enhance::new(skene_web::media::hydrated())
        .set_fallback(|| Text::new("baseline"))
        .set_child_builder(move || {
          children.set(children.get() + 1);

          Text::new("live")
        })
    }
  });

  assert_eq!(platform::hydration_mismatches(), 0);
  assert_eq!(root.text_content().expect("text"), "live");
  assert_eq!(element_count(&root), 1, "the fallback is gone");
  assert_eq!(children.get(), 1);
  assert!(skene_web::media::hydrated().get());

  platform::uninstall();
}

/// A client-only view adopts the empty place a server left for it, so the nodes after
/// it keep their markers, and builds once hydration has finished.
#[wasm_bindgen_test]
fn a_client_only_view_builds_after_hydrating_without_shifting_markers() {
  let root = server_root(
    "<div data-h=\"1\"><!--s:0--><!--s:1--><!--s:2--><!--e:2--><!--e:1-->\
     <span data-h=\"2\">after</span><!--e:0--></div>",
  );
  let server_after = only_child(&only_child(&root));

  hydrate(root.clone(), "", || {
    Column::new().set_children((
      skene_web::client_only(|| skene_web::View::build(Text::new("map"))),
      Text::new("after"),
    ))
  });

  let column = only_child(&root);

  assert_eq!(platform::hydration_mismatches(), 0);
  assert_eq!(column.text_content().expect("text"), "mapafter");
  assert!(
    column.last_element_child() == Some(server_after),
    "the later text was adopted"
  );

  platform::uninstall();
}

/// A picker hydrates over the server's `<select>`, carrying the choice the reader
/// made before the bundle arrived into the signal and onto the drawn face that takes
/// the select's place.
#[wasm_bindgen_test]
fn a_picker_carries_the_readers_choice_from_the_select() {
  let options = || {
    ["a", "b"].map(|value| {
      let name = value.to_uppercase();

      BasicListboxOption::new(ListboxOption::new(value))
        .set_label(Label::new(Text::new(&name)).set_name(name))
    })
  };
  // The option views build before the select and attach nowhere, so the select's
  // marker follows the elements they make.
  let marker = {
    let scratch = server_root("");

    document()
      .body()
      .expect("a body")
      .append_child(&scratch)
      .expect("connect the scratch root");
    platform::install(scratch.clone());

    let _ = platform::mount(|| skene_web::View::build(Column::new().set_children(options())));

    let count = element_count(&scratch);

    platform::uninstall();
    scratch.remove();

    count + 1
  };
  let root = server_root(&format!(
    "<select data-h=\"{marker}\"><option value=\"a\" selected>A</option><option value=\"b\">B</option></select>",
  ));
  let select: web_sys::HtmlSelectElement = root
    .query_selector("select")
    .expect("query")
    .expect("the select")
    .unchecked_into();

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");
  select.set_value("b");

  let value = skene_web::reactive::Signal::new("a".to_owned());

  hydrate(root.clone(), "", move || {
    BasicPicker::new()
      .bind_chosen(value)
      .set_options(options())
      .set_face(|shown| Text::derive(move || shown.get()))
  });

  assert_eq!(platform::hydration_mismatches(), 0);
  assert_eq!(value.get(), "b", "the reader's choice reached the signal");
  assert!(
    root.query_selector("select").expect("query").is_none(),
    "the select is gone"
  );

  let face = root
    .query_selector("[role=combobox]")
    .expect("query")
    .expect("the drawn face");

  let shown = face
    .query_selector("span")
    .expect("query")
    .expect("the face's text");

  assert_eq!(shown.text_content().as_deref(), Some("b"));

  platform::uninstall();
  root.remove();
}

#[wasm_bindgen_test]
async fn a_lazy_view_keeps_server_content_until_it_loads() {
  // Client build order: container 1, then the lazy segment inside the container's
  // child segment. The server numbered the lazy content later (9)—after its own
  // synchronous build—so the claim, not the order, must account for it.
  let root = server_root(
    "<div data-h=\"1\"><!--s:0--><!--s:1--><span data-h=\"9\">note</span><!--e:1--><!--e:0--></div>",
  );
  let server_span = root.query_selector("span").expect("query").expect("a span");

  hydrate(root.clone(), "", || {
    Container::new().set_child(AsyncView::new().set_child_loader(|| async { Text::new("note") }))
  });

  // Claimed: the server content still stands, visible, while the module is pending.
  assert_eq!(platform::hydration_mismatches(), 0);
  assert_eq!(element_count(&root), 2, "the server content was kept");
  assert_eq!(root.text_content().expect("text"), "note");

  for _ in 0..4 {
    tick().await;
  }

  // Swapped: the arrived content replaced the server span, nothing duplicated.
  assert_eq!(
    element_count(&root),
    2,
    "the arrived content replaced the claim"
  );
  assert_eq!(root.text_content().expect("text"), "note");

  let hydrated_span = root.query_selector("span").expect("query").expect("a span");

  assert!(
    server_span != hydrated_span,
    "the arrived content is freshly built, not the claim",
  );

  platform::uninstall();
}

/// A lazy segment's content adopts the values the server keyed under the segment's
/// place, and a resource built beside the segment adopts its own, so neither takes the
/// other's however late the segment's content arrives. The DOM shape is not under test.
#[wasm_bindgen_test]
async fn a_lazy_segments_resources_adopt_their_own_values() {
  let root = server_root("");
  let inside: Rc<RefCell<Option<Resource<String>>>> = Rc::new(RefCell::new(None));
  let after: Rc<RefCell<Option<Resource<String>>>> = Rc::new(RefCell::new(None));

  hydrate(
    root.clone(),
    r#"[["\"after\""],[],[[[0],["\"inside\""]]]]"#,
    {
      let inside = Rc::clone(&inside);
      let after = Rc::clone(&after);

      move || {
        let lazy = AsyncView::new().set_child_loader(move || {
          let inside = Rc::clone(&inside);

          async move {
            *inside.borrow_mut() = Some(Resource::new(|| async { "loaded".to_owned() }));

            Text::new("inside")
          }
        });

        *after.borrow_mut() = Some(Resource::new(|| async { "loaded".to_owned() }));

        Column::new().set_children((lazy, Text::new("after")))
      }
    },
  );

  for _ in 0..4 {
    tick().await;
  }

  let inside = (*inside.borrow()).expect("the lazy content's resource was captured");
  let after = (*after.borrow()).expect("the resource beside the segment was captured");

  after.with(|value| {
    assert_eq!(
      value.map(String::as_str),
      Some("after"),
      "the resource beside the segment adopted its own value, not the segment's",
    );
  });
  inside.with(|value| {
    assert_eq!(
      value.map(String::as_str),
      Some("inside"),
      "the segment's content adopted the value keyed under its place",
    );
  });

  platform::uninstall();
}

/// A server-rendered caption and its input are adopted in place: the container builds
/// as element 1, the input as 2, the caption's text as 3 and the label around it as 4,
/// the label standing before the input in the markup.
#[wasm_bindgen_test]
fn hydrate_adopts_a_captioned_text_field() {
  use skene_web::{BasicTextField, TextEntry};

  let root = server_root(
    "<div data-h=\"1\"><!--s:0--><label for=\"x\" data-h=\"4\"><span data-h=\"3\">Name</span></label><input type=\"text\" id=\"x\" data-skene-control=\"\" data-h=\"2\"><!--e:0--></div>",
  );
  let server_label = root
    .query_selector("label")
    .expect("query")
    .expect("a label");
  let server_input = root
    .query_selector("input")
    .expect("query")
    .expect("an input");

  hydrate(root.clone(), "", || {
    Container::new().set_child(BasicTextField::new(TextEntry::new()).set_label("Name"))
  });

  assert_eq!(platform::hydration_mismatches(), 0);
  assert_eq!(
    element_count(&root),
    4,
    "nothing was built beside the server's"
  );

  let label = root
    .query_selector("label")
    .expect("query")
    .expect("a label");
  let input = root
    .query_selector("input")
    .expect("query")
    .expect("an input");

  assert!(label == server_label, "the caption was adopted");
  assert!(input == server_input, "the input was adopted");
  assert_eq!(label.next_element_sibling().as_ref(), Some(&input));
  assert_eq!(label.get_attribute("for"), Some(input.id()));

  platform::uninstall();
}

#[wasm_bindgen_test]
fn a_structural_mismatch_is_caught_not_silently_adopted() {
  // The server sent a <span> for marker 1, but the client builds a <div> there.
  let root = server_root("<span data-h=\"1\">x</span>");

  hydrate(root.clone(), "", Container::new);

  // A fresh div is created rather than the span adopted, the divergence is counted,
  // and the span is removed once hydration ends, so the page is the client's tree.
  assert_eq!(
    element_count(&root),
    1,
    "the mismatch produced a fresh node and dropped the wrong one",
  );
  assert_eq!(only_child(&root).tag_name().to_lowercase(), "div");
  assert!(platform::hydration_mismatches() > 0);

  platform::uninstall();
}

/// A checkbox whose label the client builds afresh keeps the input the server sent,
/// moved into the client's label rather than removed with the server's.
#[wasm_bindgen_test]
fn an_adopted_input_moves_into_a_rebuilt_label() {
  use skene_web::{BasicCheckbox, Toggle};

  let app = || BasicCheckbox::new(Toggle::new());
  let (markup, _) = served(move || skene_web::View::into_any(app()));
  let mismatched = markup
    .replacen("<label", "<span", 1)
    .replacen("</label>", "</span>", 1);
  let root = server_root(&mismatched);
  let server_input = root
    .query_selector("input")
    .expect("query")
    .expect("an input");

  hydrate(root.clone(), "", app);

  let label = only_child(&root);

  assert_eq!(label.tag_name().to_lowercase(), "label");
  assert!(
    label.query_selector("input").expect("query").as_ref() == Some(&server_input),
    "the adopted input sits in the client's label: {}",
    root.inner_html()
  );
  assert_eq!(platform::hydration_mismatches(), 1);

  platform::uninstall();
}

/// Hydration converges on the client's tree: a server element and anchor the client
/// never built are removed, and an adopted element keeps exactly the client's classes.
#[wasm_bindgen_test]
fn hydration_converges_on_the_clients_tree() {
  let app = || Container::new().set_child(Text::new("kept"));
  let (markup, _) = served(move || skene_web::View::into_any(app()));
  let stale = markup.replacen("class=\"", "class=\"stale ", 1)
    + "<div data-h=\"99\">gone</div><!--s:99--><!--e:99-->";
  let root = server_root(&stale);

  hydrate(root.clone(), "", app);

  let html = root.inner_html();

  assert!(
    !html.contains("gone"),
    "the unbuilt element was removed: {html}"
  );
  assert!(
    !html.contains("s:99"),
    "the unbuilt anchors were removed: {html}"
  );
  assert!(
    !html.contains("stale"),
    "the server's extra class was dropped: {html}"
  );
  assert_eq!(platform::hydration_mismatches(), 3);
  assert_eq!(element_count(&root), 2);

  platform::uninstall();
}

/// A resource built after hydration loads rather than adopting a value served for a
/// resource the client never built.
#[wasm_bindgen_test]
async fn a_resource_built_after_hydration_loads() {
  let root = server_root("<span data-h=\"1\">x</span>");
  let later: Rc<RefCell<Option<Resource<String>>>> = Rc::new(RefCell::new(None));

  // The server's state holds a second order-keyed value no resource adopts while
  // hydrating.
  hydrate(root.clone(), r#"[["\"Ada\"","\"stale\""],[],[]]"#, || {
    let _first = Resource::new(|| async { "loaded".to_owned() });

    Text::new("hydrated")
  });

  let created =
    skene_web::reactive::Owner::new().enter(|| Resource::new(|| async { "fresh".to_owned() }));

  *later.borrow_mut() = Some(created);

  for _ in 0..4 {
    tick().await;
  }

  later.borrow().expect("the resource").with(|value| {
    assert_eq!(
      value.map(String::as_str),
      Some("fresh"),
      "it loaded instead of adopting"
    );
  });

  platform::uninstall();
}

/// A streamed page leaves its style deltas in the body; hydration adopts them with the
/// head's block and moves them into the head, so no rule is added twice, the delta
/// stays the one copy, and no change to the content takes it along.
#[wasm_bindgen_test]
fn a_streamed_delta_is_adopted_with_the_stylesheet() {
  use skene_web::Color;

  let head = document().head().expect("a head");
  let block = document().create_element("style").expect("a style");

  let _ = block.set_attribute("id", "skene-styles");
  block.set_text_content(Some(""));
  let _ = head.append_child(&block);

  // The markup a streamed render leaves: the delta right before the content it styles.
  let app = || {
    Container::new()
      .set_background(Color::srgb(0.1, 0.2, 0.3))
      .set_child(Text::new("late"))
  };
  let (markup, rule) = {
    let staging = document().create_element("div").expect("a staging root");

    platform::install(staging.clone());

    let _ = platform::mount(|| skene_web::View::build(app()));

    let container = only_child(&staging);
    let class = container.get_attribute("class").expect("a class");
    let rule = document()
      .query_selector_all("style[data-skene-style]")
      .expect("the styles")
      .item(1)
      .and_then(|style| style.text_content())
      .expect("the class's rule");

    let _ = container.set_attribute("data-h", "1");
    let _ = only_child(&container).set_attribute("data-h", "2");

    let markup = format!(
      "<style data-skene-style>{rule}</style>{}",
      staging.inner_html()
    );

    platform::uninstall();

    assert!(rule.contains(&class));

    (markup, rule)
  };
  let root = server_root(&markup);

  hydrate(root.clone(), "", app);

  let styles = document()
    .query_selector_all("style[data-skene-style]")
    .expect("the styles");
  let copies = (0..styles.length())
    .filter_map(|index| styles.item(index))
    .filter_map(|style| style.text_content())
    .filter(|text| text.contains(&rule))
    .count();

  let delta = (0..styles.length())
    .filter_map(|index| styles.item(index))
    .filter_map(|style| style.dyn_into::<Element>().ok())
    .find(|style| {
      style
        .text_content()
        .is_some_and(|text| text.contains(&rule))
    })
    .expect("the delta");

  assert_eq!(platform::hydration_mismatches(), 0);
  assert_eq!(copies, 1, "the delta is the one copy of its rule");
  assert_eq!(delta.parent_element(), document().head().map(Into::into));

  platform::uninstall();

  assert!(
    !block.is_connected(),
    "the adopted block goes with the platform"
  );
  assert!(
    document()
      .query_selector("style[data-skene-style]")
      .expect("query")
      .is_none(),
    "and so does the delta",
  );
}

/// The server's `title` is the scriptless tooltip; hydration removes it as it wires
/// the bubble, so the browser's own tooltip never shows beside ours.
#[wasm_bindgen_test]
fn hydrate_removes_the_servers_title_from_a_tooltips_child() {
  let app =
    || Tooltip::new("Saves your work").set_child(Container::new().set_child(Text::new("Save")));
  // The client never writes `title`; the server does, so the served markup carries it
  // by hand where the server would have put it.
  let (markup, _) = served(move || skene_web::View::into_any(app()));
  let markup = markup.replacen(
    "aria-describedby=",
    "title=\"Saves your work\" aria-describedby=",
    1,
  );

  assert!(markup.contains("title=\"Saves your work\""), "{markup}");

  let root = server_root(&markup);

  document()
    .body()
    .expect("a body")
    .append_child(&root)
    .expect("connect the root");

  hydrate(root.clone(), "", app);

  assert_eq!(platform::hydration_mismatches(), 0);

  let described = root
    .query_selector("[aria-describedby]")
    .expect("query")
    .expect("the described box");

  assert_eq!(
    described.get_attribute("title"),
    None,
    "{}",
    root.inner_html()
  );
  assert_eq!(
    described.get_attribute("aria-describedby"),
    root
      .query_selector("[popover]")
      .expect("query")
      .expect("the bubble")
      .get_attribute("id")
  );

  platform::uninstall();
  root.remove();
}
