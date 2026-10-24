//! The browser leaf's platform: the portable vocabulary, the markup escape and the DOM
//! escape over the live document, with the popups' placement and the clipboard's
//! reach.

use std::{
  cell::{Cell, RefCell},
  rc::Rc,
  thread::LocalKey,
  time::Duration,
};

use skene_foundation::{
  AccessProp,
  Availability,
  ClipboardAction,
  ColorScheme,
  CompiledStyle,
  ControlState,
  Definitions,
  EventKind,
  FocusOrder,
  Keep,
  Media,
  MediaSignals,
  NodeFacts,
  NodeId,
  NodeSpec,
  Observation,
  OverlayFacts,
  PickerOption,
  Placement,
  Point,
  PointerEvent,
  Realization,
  Scene,
  Size,
  Slot,
  SlotLeaf,
  TextSelection,
  Timer,
  TokenScope,
  Unit,
  notify_dismiss,
  text::TextIndex,
  web::{HydrationSource, WebHydration, WebStyle},
};
use skene_reactive::{Memo, ReadSignal};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{
  Document,
  Element,
  HtmlDialogElement,
  HtmlElement,
  HtmlInputElement,
  HtmlTextAreaElement,
  InputEvent,
  InputEventInit,
  SelectionMode,
};

use super::{
  Dom,
  Node,
  SCENE,
  Timeout,
  document,
  events,
  events::Listener,
  head,
  keep::Kept,
  media,
  try_with_dom,
  with_dom,
};
use crate::{
  declarations::Declarations,
  low_level::{WebDom, WebMarkup},
  platform::web::Web,
  realize::Realized,
  token::WebTokenValue,
};

/// Whether the engine places a box against a CSS anchor (`position-area`), decided
/// once per page.
fn anchors_in_css() -> bool {
  thread_local! {
    static SUPPORTED: Cell<Option<bool>> = const { Cell::new(None) };
  }

  SUPPORTED.with(|cell| {
    if let Some(known) = cell.get() {
      return known;
    }

    let supported =
      web_sys::css::supports_with_value("position-area", "block-end").unwrap_or(false);

    cell.set(Some(supported));

    supported
  })
}

/// The async clipboard, where the page has one: a secure context.
fn clipboard(dom: &Dom) -> Option<web_sys::Clipboard> {
  let navigator = dom.document().default_view()?.navigator();

  js_sys::Reflect::has(&navigator, &JsValue::from_str("clipboard"))
    .ok()
    .filter(|has| *has)
    .map(|_| navigator.clipboard())
}

/// The text entry a clipboard action acts on: the active one, or the one focus left
/// for the open popup it now sits in.
fn clipboard_entry(dom: &Dom) -> Option<Element> {
  let active = dom.document().active_element()?;

  if entry_selection(&active).is_some() {
    return Some(active);
  }

  let active: &web_sys::Node = &active;
  let popups: Vec<NodeId> = Scene::<Web>::with(|scene| {
    scene
      .open_overlays()
      .iter()
      .filter(|overlay| !scene.is_modal(**overlay))
      .copied()
      .collect()
  });
  let in_popup = popups.iter().any(|sink| {
    dom
      .node(*sink)
      .is_some_and(|node| node.element.contains(Some(active)))
  });

  in_popup.then(|| dom.events.last_entry.clone()).flatten()
}

/// The selection of an `<input>` or `<textarea>`: its start and end in UTF-16 units,
/// and the text between them.
fn entry_selection(entry: &Element) -> Option<(u32, u32, String)> {
  let (value, start, end) = if let Some(input) = entry.dyn_ref::<HtmlInputElement>() {
    (
      input.value(),
      input.selection_start().ok()??,
      input.selection_end().ok()??,
    )
  }
  else {
    let editor = entry.dyn_ref::<HtmlTextAreaElement>()?;

    (
      editor.value(),
      editor.selection_start().ok()??,
      editor.selection_end().ok()??,
    )
  };
  let index = TextIndex::new(&value);
  let selected = index.byte_range(Unit::Utf16, start as usize..end as usize);
  let units = index.unit_range(Unit::Utf16, selected.clone());
  let unit = |offset: usize| u32::try_from(offset).unwrap_or(u32::MAX);

  Some((
    unit(units.start),
    unit(units.end),
    value[selected].to_owned(),
  ))
}

/// The document's selection as text, if any.
fn document_selection(dom: &Dom) -> Option<String> {
  let selection = dom.document().default_view()?.get_selection().ok()??;

  Some(String::from(selection.to_string()))
}

/// Replaces the selection of an `<input>` or `<textarea>` with `text`, the caret after
/// it, and reports the edit as the platform would: a bubbling `input` of `input_type`.
fn replace_entry_selection(entry: &Element, text: &str, input_type: &str) {
  let Some((start, end, _)) = entry_selection(entry)
  else {
    return;
  };
  let replaced = if let Some(input) = entry.dyn_ref::<HtmlInputElement>() {
    input
      .set_range_text_with_start_and_end_and_selection_mode(text, start, end, SelectionMode::End)
      .is_ok()
  }
  else if let Some(editor) = entry.dyn_ref::<HtmlTextAreaElement>() {
    editor
      .set_range_text_with_start_and_end_and_mode(text, start, end, "end")
      .is_ok()
  }
  else {
    false
  };

  if !replaced {
    return;
  }

  let init = InputEventInit::new();

  init.set_bubbles(true);
  init.set_input_type(input_type);
  init.set_data((!text.is_empty()).then_some(text));

  if let Ok(event) = InputEvent::new_with_event_init_dict("input", &init) {
    let _ = entry.dispatch_event(&event);
  }
}

/// Pins an open popup's top-left corner to `point` in viewport pixels, shifted back into
/// the viewport where it would overflow.
fn position_at_point(popup: &HtmlElement, point: Point<f32>) {
  let style = popup.style();

  let _ = style.set_property("inset", "auto");
  let _ = style.set_property("margin", "0");
  let _ = style.set_property("position", "fixed");
  let _ = style.set_property("left", &format!("{}px", point.x));
  let _ = style.set_property("top", &format!("{}px", point.y));

  let Some(window) = web_sys::window()
  else {
    return;
  };
  let extent = |value: Result<JsValue, JsValue>| value.ok().and_then(|value| value.as_f64());
  let (Some(viewport_width), Some(viewport_height)) =
    (extent(window.inner_width()), extent(window.inner_height()))
  else {
    return;
  };
  let width = f64::from(popup.offset_width());
  let height = f64::from(popup.offset_height());
  let x = f64::from(point.x).min(viewport_width - width).max(0.);
  let y = f64::from(point.y).min(viewport_height - height).max(0.);

  let _ = style.set_property("left", &format!("{x}px"));
  let _ = style.set_property("top", &format!("{y}px"));
}

/// Positions an open popup against its anchor: below it for `Bottom`, above it for
/// `Top`, start-aligned either way; other placements keep their viewport position.
///
/// Where the engine has CSS anchor positioning the anchor is named after its sink and
/// the popup follows it through `position-area`; elsewhere the popup is placed at the
/// anchor's measured rectangle as it opens.
fn position_at_anchor(popup: &HtmlElement, anchor: &Element, sink: NodeId, placement: Placement) {
  let area = match placement {
    Placement::Top => "block-start span-inline-end",
    Placement::Bottom => "block-end span-inline-end",
    _ => return,
  };
  let style = popup.style();

  let _ = style.set_property("inset", "auto");
  let _ = style.set_property("margin", "0");
  let _ = style.set_property("position", "fixed");

  if anchors_in_css()
    && let Some(anchor) = anchor.dyn_ref::<HtmlElement>()
  {
    let name = format!("--skene-anchor-{}-{}", sink.index(), sink.generation());

    let _ = anchor.style().set_property("anchor-name", &name);
    let _ = style.set_property("position-anchor", &name);
    let _ = style.set_property("position-area", area);

    return;
  }

  let rect = anchor.get_bounding_client_rect();
  let top = match placement {
    Placement::Top => rect.top() - f64::from(popup.offset_height()),
    _ => rect.bottom(),
  };

  let _ = style.set_property("left", &format!("{}px", rect.left()));
  let _ = style.set_property("top", &format!("{top}px"));
}

impl Realization for Web {
  const FONT_SIZE: f32 = 16.;
  const KEEP: Keep = Keep::Everything;

  type Scalar = f32;
  type StyleExt = WebStyle;
  type TokenValue = WebTokenValue;

  fn scene() -> &'static LocalKey<RefCell<Scene<Self>>> {
    &SCENE
  }

  fn create(sink: NodeId, spec: &NodeSpec) {
    with_dom(|dom| {
      dom.realizer.create(sink, spec);

      if let NodeFacts::Overlay(OverlayFacts { modal: true, .. }) = spec.facts
        && let Some(node) = dom.node_mut(sink)
      {
        node.extra.cancel = Some(Listener::new(
          node.element.clone().into(),
          "cancel",
          false,
          move |event| {
            event.prevent_default();

            if with_dom(|dom| dom.node(sink).is_some()) {
              notify_dismiss::<Self>(sink);
            }
          },
        ));
      }
    });
  }

  fn apply_style(sink: NodeId, style: CompiledStyle<f32, WebStyle>) {
    with_dom(|dom| dom.realizer.apply_style(sink, &style));
  }

  fn apply_slot(sink: NodeId, slot: Slot, leaf: &SlotLeaf<f32, WebStyle>) {
    with_dom(|dom| dom.realizer.apply_slot(sink, slot, leaf));
  }

  fn update_focus_order(sink: NodeId, focus_order: Option<FocusOrder>) {
    with_dom(|dom| dom.realizer.update_focus_order(sink, focus_order));
  }

  fn update_access(sink: NodeId, access: &[AccessProp]) {
    with_dom(|dom| dom.realizer.update_access(sink, access));
  }

  fn set_control_state(sink: NodeId, state: &ControlState) {
    with_dom(|dom| dom.realizer.set_control_state(sink, *state));
  }

  fn set_overlay_open(sink: NodeId, open: bool, anchor: Option<NodeId>) {
    let Some((element, modal, placement, position, anchor)) = with_dom(|dom| {
      let node = dom.node(sink)?;
      let Realized::Overlay {
        modal,
        placement,
        position,
      } = node.realized
      else {
        return None;
      };
      let element = node.dom_element()?.clone();
      let anchor_element = anchor.and_then(|anchor| {
        let element = dom.node(anchor)?.dom_element().cloned()?;

        Some((anchor, element))
      });

      if open && !element.is_connected() {
        dom.pending_open.insert(sink, anchor);

        return None;
      }

      dom.pending_open.remove(&sink);

      Some((element, modal, placement, position, anchor_element))
    })
    else {
      return;
    };

    if modal {
      if let Some(dialog) = element.dyn_ref::<HtmlDialogElement>() {
        if open {
          let _ = dialog.show_modal();
        }
        else {
          dialog.close();
        }
      }

      return;
    }

    let Some(html) = element.dyn_ref::<HtmlElement>()
    else {
      return;
    };

    if open {
      let _ = html.show_popover();

      if let Some(position) = position {
        position_at_point(html, position);
      }
      else if let Some((sink, anchor)) = anchor {
        position_at_anchor(html, &anchor, sink, placement);
      }
    }
    else {
      let _ = html.hide_popover();
    }
  }

  fn set_overlay_position(sink: NodeId, position: Option<Point<f32>>) {
    with_dom(|dom| {
      if let Some(node) = dom.node_mut(sink)
        && let Realized::Overlay { position: held, .. } = &mut node.realized
      {
        *held = position;
      }
    });
  }

  fn set_overlay_placement(sink: NodeId, placement: Placement) {
    with_dom(|dom| {
      if let Some(node) = dom.node_mut(sink)
        && let Realized::Overlay {
          placement: held, ..
        } = &mut node.realized
      {
        *held = placement;
      }
    });
  }

  fn set_text(sink: NodeId, text: &str) {
    with_dom(|dom| dom.realizer.set_text(sink, text));
  }

  fn attach(parent: NodeId, child: NodeId, before: Option<NodeId>) {
    let Some((parent, child, caption, before, saved, kept, document)) = with_dom(|dom| {
      if dom.place_caption(parent, child) {
        return None;
      }

      let parent = dom.node(parent)?.element.clone();
      let child_node = dom.node(child)?;

      if child_node.adopted && child_node.element.parent_node().as_ref() == Some(&parent) {
        return None;
      }

      let child_dom = child_node.element.clone();
      let caption = child_node.caption.clone();
      let saved = before.and_then(|before| dom.saved.remove(&before));
      let before = before
        .and_then(|before| dom.node(before))
        .map(|node| node.element.clone());

      Some((
        parent,
        child_dom,
        caption,
        before,
        saved,
        dom.kept.remove(&child),
        dom.document().clone(),
      ))
    })
    else {
      return;
    };

    let _ = parent.insert_before(&child, before.as_ref());

    if let Some(caption) = caption
      && caption.parent_node().as_ref() != Some(&parent)
    {
      let _ = parent.insert_before(&caption, Some(&child));
    }

    if let Some(saved) = saved {
      saved.restore(&child);
    }

    if let Some(kept) = kept {
      kept.restore(&document);
    }

    for (overlay, anchor) in with_dom(Dom::take_connected_opens) {
      Self::set_overlay_open(overlay, true, anchor);
    }
  }

  fn relocate(parent: NodeId, child: NodeId, before: Option<NodeId>) {
    if with_dom(|dom| dom.lift_caption(child)) {
      Self::attach(parent, child, before);

      return;
    }

    let Some((parent, child, caption, before, mover)) = with_dom(|dom| {
      if dom.place_caption(parent, child) {
        return None;
      }

      let parent = dom.node(parent)?.element.clone();
      let child_node = dom.node(child)?;
      let child_dom = child_node.element.clone();
      let caption = child_node.caption.clone();
      let before = before
        .and_then(|before| dom.node(before))
        .map(|node| node.element.clone());

      Some((parent, child_dom, caption, before, dom.mover.clone()))
    })
    else {
      return;
    };

    mover.move_node(&parent, &child, before.as_ref());

    if let Some(caption) = caption {
      mover.move_node(&parent, &caption, Some(&child));
    }

    for (overlay, anchor) in with_dom(Dom::take_connected_opens) {
      Self::set_overlay_open(overlay, true, anchor);
    }
  }

  fn hide(parent: NodeId, child: NodeId) {
    let (nodes, overlays) = Scene::<Self>::with(|scene| {
      let mut nodes = vec![child];
      let mut next = 0;

      while let Some(node) = nodes.get(next).copied() {
        nodes.extend(scene.children(node));
        next += 1;
      }

      let overlays: Vec<_> = scene
        .open_overlays()
        .iter()
        .copied()
        .filter(|overlay| scene.ancestry(*overlay).contains(&child))
        .map(|overlay| (overlay, scene.overlay_anchor(overlay)))
        .collect();

      (nodes, overlays)
    });

    with_dom(|dom| {
      if let Some(kept) = dom
        .node(child)
        .map(|node| node.element.clone())
        .and_then(|root| Kept::capture(dom, &root, &nodes))
      {
        dom.kept.insert(child, kept);
      }

      dom.pending_open.extend(overlays);
    });
    Self::remove(parent, child);
  }

  fn remove(parent: NodeId, child: NodeId) {
    let Some((parent, child, caption)) = with_dom(|dom| {
      if dom.lift_caption(child) {
        return None;
      }

      Some((
        dom.node(parent)?.element.clone(),
        dom.node(child)?.element.clone(),
        dom.node(child)?.caption.clone(),
      ))
    })
    else {
      return;
    };

    if child.parent_node().as_ref() == Some(&parent) {
      let _ = parent.remove_child(&child);
    }

    if let Some(caption) = caption
      && caption.parent_node().as_ref() == Some(&parent)
    {
      let _ = parent.remove_child(&caption);
    }
  }

  fn destroy(sink: NodeId) {
    with_dom(|dom| {
      let Some(node) = dom.realizer.destroy(sink)
      else {
        return;
      };

      if !node.extra.observers.is_empty()
        && let (Some((observer, _)), Some(element)) = (&dom.resize, node.dom_element())
      {
        observer.unobserve(element);
      }

      if let (Some(pixels), Some(canvas)) = (&dom.pixels, &node.extra.canvas) {
        pixels.unobserve(&canvas.element);
      }

      dom.saved.remove(&sink);
      dom.kept.remove(&sink);
      dom.pending_open.remove(&sink);
    });
  }

  fn define_tokens(scope: TokenScope, definitions: &Definitions<WebTokenValue>) {
    with_dom(|dom| dom.realizer.define_tokens(scope, definitions));
  }

  fn listen(sink: NodeId, kind: EventKind) {
    with_dom(|dom| {
      dom.events.ensure(kind);

      if kind == EventKind::DragStart
        && let Some(element) = dom.node(sink).and_then(Node::dom_element)
      {
        let _ = element.set_attribute("draggable", "true");
      }
    });
  }

  fn localize_pointer(sink: NodeId, event: &PointerEvent) -> PointerEvent {
    let element = with_dom(|dom| dom.node(sink).and_then(Node::dom_element).cloned());

    element.map_or(*event, |element| events::localized(event, &element))
  }

  fn focus(sink: NodeId) {
    let carrier = with_dom(|dom| dom.node(sink).and_then(Node::carrier_element).cloned());

    if let Some(html) = carrier
      .as_ref()
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.focus();
    }
  }

  fn blur(sink: NodeId) {
    let carrier = with_dom(|dom| dom.node(sink).and_then(Node::carrier_element).cloned());

    if let Some(html) = carrier
      .as_ref()
      .and_then(|carrier| carrier.dyn_ref::<HtmlElement>())
    {
      let _ = html.blur();
    }
  }

  fn focus_visible() -> bool {
    web_sys::window()
      .and_then(|window| window.document())
      .and_then(|document| document.active_element())
      .is_some_and(|element| element.matches(":focus-visible").unwrap_or(false))
  }

  fn set_value(sink: NodeId, value: &str) {
    with_dom(|dom| dom.realizer.set_value(sink, value));
  }

  fn set_options(sink: NodeId, options: &[PickerOption]) {
    with_dom(|dom| dom.realizer.set_options(sink, options));
  }

  fn set_selection(sink: NodeId, selection: TextSelection) {
    with_dom(|dom| {
      let Some(carrier) = dom.node(sink).and_then(Node::carrier_element)
      else {
        return;
      };

      if dom.realizer.sink.holds_reader_text(carrier) {
        return;
      }

      events::select(carrier, selection);
    });
  }

  fn set_checked(sink: NodeId, checked: bool) {
    with_dom(|dom| dom.realizer.set_checked(sink, checked));
  }

  fn observe_size(sink: NodeId, report: Rc<dyn Fn(Size<f32>)>) -> Observation {
    let kept = Rc::clone(&report);

    with_dom(|dom| {
      let observer = dom.resize_observer();
      let Some(node) = dom.node_mut(sink)
      else {
        return;
      };

      if let Some(element) = node.dom_element() {
        if node.extra.observers.is_empty() {
          observer.observe(element);
        }

        node.extra.observers.push(report);
      }
    });

    Observation::new(move || {
      let _ = try_with_dom(|dom| {
        let observer = dom.resize_observer();
        let Some(node) = dom.node_mut(sink)
        else {
          return;
        };

        node
          .extra
          .observers
          .retain(|observer| !Rc::ptr_eq(observer, &kept));

        if node.extra.observers.is_empty()
          && let Some(element) = node.dom_element()
        {
          observer.unobserve(element);
        }
      });
    })
  }

  fn media() -> MediaSignals<f32> {
    media::signals()
  }

  fn hydration() -> Option<ReadSignal<bool>> {
    Some(media::live())
  }

  fn set_scheme(scheme: Option<ColorScheme>) {
    media::set_scheme(scheme);
  }

  fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Timer {
    with_dom(|dom| {
      dom.timers.retain(|_, timeout| !timeout.fired.get());

      let timer = Timer(dom.next_timer);
      let fired = Rc::new(Cell::new(false));
      let action = Cell::new(Some(action));
      let callback = Closure::<dyn FnMut()>::new({
        let fired = Rc::clone(&fired);

        move || {
          fired.set(true);

          if let Some(action) = action.take() {
            action();
          }
        }
      });
      let handle = dom
        .document()
        .default_view()
        .and_then(|window| {
          window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
              callback.as_ref().unchecked_ref(),
              i32::try_from(delay.as_millis()).unwrap_or(i32::MAX),
            )
            .ok()
        })
        .unwrap_or(0);

      dom.next_timer += 1;
      dom.timers.insert(
        timer,
        Timeout {
          handle,
          _callback: callback,
          fired,
        },
      );

      timer
    })
  }

  fn cancel(timer: Timer) {
    with_dom(|dom| {
      if let Some(timeout) = dom.timers.remove(&timer)
        && !timeout.fired.get()
        && let Some(window) = dom.document().default_view()
      {
        window.clear_timeout_with_handle(timeout.handle);
      }
    });
  }

  fn clipboard_availability(action: ClipboardAction) -> Availability {
    with_dom(|dom| {
      if clipboard(dom).is_none() {
        return Availability::Absent;
      }

      // Outside an entry, only the document's selection is copyable, and nothing is
      // cut or pasted; inside one, every action is listed and applies to its selection.
      match (clipboard_entry(dom), action) {
        (Some(entry), ClipboardAction::Copy | ClipboardAction::Cut) => {
          if entry_selection(&entry).is_some_and(|(start, end, _)| start != end) {
            Availability::Enabled
          }
          else {
            Availability::Disabled
          }
        }
        (Some(_), ClipboardAction::Paste) => Availability::Enabled,
        (None, ClipboardAction::Copy) => {
          if document_selection(dom).is_some_and(|text| !text.is_empty()) {
            Availability::Enabled
          }
          else {
            Availability::Absent
          }
        }
        (None, ClipboardAction::Cut | ClipboardAction::Paste) => Availability::Absent,
      }
    })
  }

  fn clipboard(action: ClipboardAction) {
    let Some((clipboard, entry, text)) = with_dom(|dom| {
      let clipboard = clipboard(dom)?;
      let entry = clipboard_entry(dom);
      let text = match &entry {
        Some(entry) => entry_selection(entry).map(|(_, _, text)| text),
        None => document_selection(dom),
      };

      Some((clipboard, entry, text))
    })
    else {
      return;
    };

    match action {
      ClipboardAction::Copy => {
        if let Some(text) = text {
          let _ = clipboard.write_text(&text);
        }
      }
      ClipboardAction::Cut => {
        if let (Some(text), Some(entry)) = (text, entry) {
          let _ = clipboard.write_text(&text);
          replace_entry_selection(&entry, "", "deleteByCut");
        }
      }
      ClipboardAction::Paste => {
        if let Some(entry) = entry {
          let read = wasm_bindgen_futures::JsFuture::from(clipboard.read_text());

          wasm_bindgen_futures::spawn_local(async move {
            if let Some(text) = read.await.ok().and_then(|text| text.as_string()) {
              replace_entry_selection(&entry, &text, "insertFromPaste");
            }
          });
        }
      }
    }
  }

  fn chosen_scheme() -> Option<ColorScheme> {
    media::chosen_scheme()
  }

  fn set_window_title(title: Option<&str>) {
    head::set_window_title(title);
  }

  fn current_path() -> Option<String> {
    Some(super::current_path())
  }

  fn push_path(path: &str) {
    super::push_path(path);
  }

  fn on_pop(on_pop: impl Fn(String) + 'static) {
    super::on_pop(on_pop);
  }

  fn commit_navigation(update: Box<dyn FnOnce()>) {
    super::commit_navigation(update);
  }

  fn watch_media(media: &Media<f32>) -> Memo<bool> {
    media::watch_media(media)
  }
}

/// The browser hydrates: it adopts the state and claims the markup a server render
/// carried, and streams nothing.
impl WebHydration for Web {
  type LazyRegion = crate::resource_state::LazyRegion;

  fn install_executor() {
    super::install_executor();
  }

  fn is_server() -> bool {
    false
  }

  fn adopt_next() -> Option<String> {
    crate::resource_state::adopt_next()
  }

  fn adopt_tagged(tag: &str) -> Option<String> {
    crate::resource_state::adopt_tagged(tag)
  }

  fn register_source(source: HydrationSource) {
    crate::resource_state::register_source(source);
  }

  fn register_tagged_source(tag: &str, source: HydrationSource) {
    crate::resource_state::register_tagged_source(tag, source);
  }

  fn enter_lazy() -> Self::LazyRegion {
    crate::resource_state::enter_lazy()
  }

  fn within_lazy<R>(region: &Self::LazyRegion, work: impl FnOnce() -> R) -> R {
    crate::resource_state::within_lazy(region, work)
  }

  fn claim_lazy(start: NodeId) -> bool {
    super::claim_lazy(start)
  }

  fn discard_lazy(start: NodeId) {
    super::discard_lazy(start);
  }

  fn register_boundary(_start: NodeId, _settled: Box<dyn Fn() -> bool>) {}
}

impl WebMarkup for Web {
  fn create_element(node: NodeId, tag: &'static str) {
    with_dom(|dom| {
      dom.realizer.create_low_level(node, tag);
    });
  }

  fn set_attribute(sink: NodeId, name: &str, value: &str) {
    with_dom(|dom| dom.realizer.set_attribute(sink, name, value));
  }

  fn remove_attribute(sink: NodeId, name: &str) {
    with_dom(|dom| dom.realizer.remove_attribute(sink, name));
  }

  fn set_declarations(sink: NodeId, declarations: &Declarations) {
    with_dom(|dom| dom.realizer.set_declarations(sink, declarations));
  }

  fn set_text(sink: NodeId, text: &str) {
    <Self as Realization>::set_text(sink, text);
  }
}

impl WebDom for Web {
  fn element(sink: NodeId) -> Option<Element> {
    with_dom(|dom| dom.node(sink).and_then(Node::dom_element).cloned())
  }

  fn document() -> Document {
    document()
  }
}
