//! The realization both web leaves share: one record per node beside the scene, the
//! element choices, attributes, classes, slots and token definitions each node is
//! realized with, and the stylesheet bookkeeping, written once over the [`Sink`] the
//! DOM and the string tree implement.

use std::rc::Rc;

use skene_foundation::{
  AccessProp,
  BoxProps,
  BoxValue,
  CheckState,
  ControlProps,
  ControlState,
  Definitions,
  FocusOrder,
  NodeId,
  NodeMap,
  OverlayProps,
  PickerOption,
  Placement,
  Point,
  RunProps,
  RunValue,
  Scene,
  Slot,
  StaticBoxStyle,
  StaticLeaf,
  web::WebStyle,
};

use crate::{
  LinkPolicy,
  compile::{
    Compiled,
    SlotBinding,
    all_unconditional,
    box_fingerprint,
    compile_box,
    compile_control,
    compile_definitions,
    compile_layout,
    compile_overlay,
    compile_root_definitions,
    compile_run,
    control_fingerprint,
    inline_definitions,
    layout_fingerprint,
    overlay_fingerprint,
    render_box_leaf,
    render_run_leaf,
    run_fingerprint,
    slot_properties,
  },
  condition::{CONTROL_ATTRIBUTE, DRAGS_ATTRIBUTE, NATIVE_CLASS, drags_value},
  declarations::Declarations,
  markup::{
    FormElement,
    NATIVE_ACCESS_ATTRIBUTES,
    SCOPE_ATTRIBUTE,
    access_attributes,
    anchor_text,
    aria_role,
    box_tag,
    drawn_element,
    element_id,
    focus_order_attribute,
    form_element,
    is_form_element,
    native_attributes,
    referenced_nodes,
    replaced_by_access,
    runs_script,
    scrolls,
    takes_disabled,
  },
  platform::web::Web,
  style_book::{StyleBook, StyleSink},
  token::WebTokenValue,
};

/// Where a web leaf realizes the scene: the live DOM in the browser, a string tree on
/// the server.
pub(crate) trait Sink {
  /// An element or a comment of the sink.
  type Element: Clone;
  /// What the leaf keeps per node beside the shared record.
  type Extra: Default;
  /// Where the leaf's stylesheet goes.
  type Sheet: StyleSink;

  /// Whether a plain description is written as `title`, the tooltip of a page that
  /// runs no script.
  const TITLE_TOOLTIPS: bool;

  /// A new element of `tag`, taking the next hydration marker, with whether the
  /// hydration in progress adopted it from the server's markup.
  fn create_element(&mut self, tag: &'static str) -> (Self::Element, bool);

  /// A new comment of `text`, with whether the hydration in progress adopted it.
  fn create_comment(&mut self, text: String) -> (Self::Element, bool);

  /// Binds `element` to the scene's `node` it realizes.
  fn bind(&mut self, element: &Self::Element, node: NodeId);

  /// Forgets `element`, which no record holds any longer.
  fn discard(&mut self, element: &Self::Element);

  /// Appends `child` to the children of `parent`, where it is not among them already.
  fn append_child(&mut self, parent: &Self::Element, child: &Self::Element);

  /// Sets the attribute `name` of `element` to `value`.
  fn set_attribute(&mut self, element: &Self::Element, name: &str, value: &str);

  /// Removes the attribute `name` of `element`.
  fn remove_attribute(&mut self, element: &Self::Element, name: &str);

  /// Removes every attribute of `element` whose name `matches`.
  fn remove_attributes(&mut self, element: &Self::Element, matches: impl Fn(&str) -> bool);

  /// Sets the attribute `name` of `element` present and empty when `on`, else absent.
  fn set_flag(&mut self, element: &Self::Element, name: &str, on: bool) {
    if on {
      self.set_attribute(element, name, "");
    }
    else {
      self.remove_attribute(element, name);
    }
  }

  /// Sets the inline declaration of `property` on `element` to `value`.
  fn set_property(&mut self, element: &Self::Element, property: &str, value: &str);

  /// Removes the inline declaration of `property` from `element`.
  fn remove_property(&mut self, element: &Self::Element, property: &str);

  /// The custom properties the inline style of `element` declares.
  fn custom_properties(&self, element: &Self::Element) -> Vec<String>;

  /// The text of `element`.
  fn text(&self, element: &Self::Element) -> Option<String>;

  /// Replaces the content of `element` with `text`.
  fn set_text(&mut self, element: &Self::Element, text: &str);

  /// Replaces `previous` with `class` among the classes of `element`.
  fn swap_class(&mut self, element: &Self::Element, previous: Option<&str>, class: &str);

  /// Sets whether the native toggle `element` is checked.
  fn set_checked(&mut self, element: &Self::Element, checked: bool);

  /// Sets whether the native toggle `element` shows as indeterminate.
  fn set_indeterminate(&mut self, element: &Self::Element, indeterminate: bool);

  /// Sets the value of the form element `element`.
  fn set_value(&mut self, element: &Self::Element, value: &str);

  /// Replaces the options of the `<select>` `element`.
  fn set_options(&mut self, element: &Self::Element, options: &[PickerOption]);

  /// Notes what the reader changed on the adopted native `input` of the control `node`
  /// before the client arrived.
  fn note_live_state(&mut self, node: NodeId, input: &Self::Element, native: FormElement);
}

/// What a node is realized as.
pub(crate) enum Realized {
  /// A box.
  Box,
  /// A control, with the form element it stands in where it has one.
  Control {
    /// The form element, where the control has one.
    native: Option<FormElement>,
  },
  /// An overlay.
  #[cfg_attr(
    skene_server,
    expect(
      dead_code,
      reason = "the server renders every overlay closed, unplaced"
    )
  )]
  Overlay {
    /// Whether it is modal.
    modal: bool,
    /// Where it opens against its anchor.
    placement: Placement,
    /// The point it is pinned at, where it is pinned to one.
    position: Option<Point<f32>>,
  },
  /// A text run.
  Run,
  /// An anchor.
  Anchor,
  /// A scope.
  Scope,
}

/// One node's record beside the scene.
pub(crate) struct Record<S: Sink> {
  /// The element realizing the node, or an anchor's comment.
  pub element: S::Element,
  /// The element's tag; empty for an anchor and for the root the scene mounts on.
  pub tag: &'static str,
  /// The native element carrying a control's state, focus and value, where the control
  /// has one apart from its root.
  pub carrier: Option<S::Element>,
  /// The `<label>` the caption a control keeps beside it lives in.
  pub caption: Option<S::Element>,
  /// The control whose caption it is, while it sits in that control's `<label>`.
  pub captioned: Option<NodeId>,
  /// What the node is realized as.
  pub realized: Realized,
  /// The class of its compiled style.
  pub style_class: Option<Rc<str>>,
  /// The class of the node's token definitions.
  pub token_class: Option<Rc<str>>,
  /// The custom properties the node's token definitions are written inline as, over
  /// its token class.
  pub inline_tokens: Vec<String>,
  /// The slots its class references.
  pub slots: Rc<[SlotBinding]>,
  /// Whether the box scrolls its content, so it takes a place in the focus order.
  pub scrolls: bool,
  /// Whether the hydration in progress adopted its element from the server's markup.
  pub adopted: bool,
  /// What the leaf keeps beside.
  #[cfg_attr(
    skene_server,
    expect(dead_code, reason = "the server keeps nothing beside the record")
  )]
  pub extra: S::Extra,
}

impl<S: Sink> Record<S> {
  /// The element carrying the node's state, focus and value: its native input, else
  /// itself.
  #[inline(always)]
  pub(crate) fn carrier(&self) -> &S::Element {
    self.carrier.as_ref().unwrap_or(&self.element)
  }

  /// The tag of the element carrying the node's state.
  const fn carrier_tag(&self) -> &'static str {
    match self.realized {
      Realized::Control {
        native: Some(native),
      } => native.tag,
      _ => self.tag,
    }
  }

  /// Whether the node is a control.
  #[inline(always)]
  pub(crate) const fn is_control(&self) -> bool {
    matches!(self.realized, Realized::Control { .. })
  }

  /// Whether the node is a control whose own element is its `<input>`, `<textarea>` or
  /// `<select>`, which its caption's `<label>` is `for`.
  #[inline(always)]
  pub(crate) fn is_bare(&self) -> bool {
    matches!(
      self.realized,
      Realized::Control { native: Some(native) } if native.is_bare()
    )
  }
}

/// One leaf's realization of the scene: the record of every realized node, the sink
/// they are realized into, and the stylesheet.
pub(crate) struct Realizer<S: Sink> {
  /// The sink the nodes are realized into.
  pub sink: S,
  /// The record of every realized node.
  pub records: NodeMap<Record<S>>,
  /// The stylesheet bookkeeping.
  pub book: StyleBook<S::Sheet>,
  /// The class of the root's token definitions.
  root_class: Option<Rc<str>>,
  /// The ordinal the next anchor's comment takes.
  anchors: u32,
}

impl<S: Sink> Realizer<S> {
  /// A realization into `sink` writing its stylesheet into `sheet`, with nothing
  /// realized yet.
  pub(crate) fn new(sink: S, sheet: S::Sheet) -> Self {
    Self {
      sink,
      records: NodeMap::new(),
      book: StyleBook::new(sheet),
      root_class: None,
      anchors: 0,
    }
  }

  /// Records that `element`, a `tag`, realizes `node` as `realized`.
  pub(crate) fn insert(
    &mut self,
    node: NodeId,
    element: S::Element,
    tag: &'static str,
    realized: Realized,
    adopted: bool,
  ) {
    self.sink.bind(&element, node);

    let _replaced = self.records.insert(
      node,
      Record {
        element,
        tag,
        carrier: None,
        caption: None,
        captioned: None,
        realized,
        style_class: None,
        token_class: None,
        inline_tokens: Vec::new(),
        slots: Rc::from([]),
        scrolls: false,
        adopted,
        extra: S::Extra::default(),
      },
    );
  }

  /// Realizes `node` as a new element of `tag`, returning it with whether it was
  /// adopted.
  pub(crate) fn create_element(
    &mut self,
    node: NodeId,
    tag: &'static str,
    realized: Realized,
  ) -> (S::Element, bool) {
    let (element, adopted) = self.sink.create_element(tag);

    self.insert(node, element.clone(), tag, realized, adopted);

    (element, adopted)
  }

  /// Realizes the box `node`.
  pub(crate) fn create_box(&mut self, node: NodeId, props: &BoxProps<Web>) {
    let tag = box_tag(
      props.role,
      AccessProp::level_of(&props.access),
      props.destination.is_some(),
    );
    let (element, _) = self.create_element(node, tag, Realized::Box);

    if let Some(role) = props.role.and_then(aria_role) {
      self.sink.set_attribute(&element, "role", role);
    }

    if let Some(href) = props
      .destination
      .as_deref()
      .filter(|href| !runs_script(href))
    {
      self.sink.set_attribute(&element, "href", href);
    }

    let compiled = self
      .book
      .compiled(box_fingerprint(props, 0), || compile_box(props));

    self.finish_box(node, props, &compiled);
  }

  /// Realizes the control `node`: a form element bare or inside its label where it
  /// stands in one, else the element it is drawn as.
  pub(crate) fn create_control(&mut self, node: NodeId, props: &ControlProps<Web>) {
    let native = form_element(props.native);
    let realized = Realized::Control { native };
    let root = match native {
      Some(native) if native.is_bare() => {
        let (root, adopted) = self.create_element(node, native.tag, realized);

        self.write_native_attributes(&root, native, props);

        if adopted {
          self.sink.note_live_state(node, &root, native);
        }

        root
      }
      Some(native) => {
        let (root, _) = self.create_element(node, "label", realized);
        let (input, adopted) = self.sink.create_element(native.tag);

        self.write_native_attributes(&input, native, props);
        self.sink.set_attribute(&input, "class", NATIVE_CLASS);

        if adopted {
          self.sink.note_live_state(node, &input, native);
        }

        self.sink.append_child(&root, &input);

        if let Some(record) = self.records.get_mut(node) {
          record.carrier = Some(input);
        }

        root
      }
      None => {
        let (tag, attribute) = drawn_element(props.native, props.box_props.role);
        let (root, _) = self.create_element(node, tag, realized);

        if let Some((name, value)) = attribute {
          self.sink.set_attribute(&root, name, value);
        }

        if let Some(label) = &props.label {
          self.sink.set_attribute(&root, "aria-label", label);
        }

        root
      }
    };

    self.sink.set_attribute(&root, CONTROL_ATTRIBUTE, "");

    let compiled = self
      .book
      .compiled(control_fingerprint(props), || compile_control(props));

    self.finish_box(node, &props.box_props, &compiled);
  }

  /// Realizes the overlay `node`: a `<dialog>` when modal, else a popover.
  pub(crate) fn create_overlay(&mut self, node: NodeId, props: &OverlayProps<Web>) {
    let realized = Realized::Overlay {
      modal: props.modal,
      placement: props.placement,
      position: None,
    };

    if props.modal {
      self.create_element(node, "dialog", realized);
    }
    else {
      let (element, _) = self.create_element(node, "div", realized);

      self.sink.set_attribute(
        &element,
        "popover",
        if props.hint { "hint" } else { "manual" },
      );

      if let Some(role) = props.role().and_then(aria_role) {
        self.sink.set_attribute(&element, "role", role);
      }
    }

    let compiled = self
      .book
      .compiled(overlay_fingerprint(props), || compile_overlay(props));

    self.finish_box(node, &props.box_props, &compiled);
  }

  /// Realizes the text run `node`: a link where it has one, else a span.
  pub(crate) fn create_run(&mut self, node: NodeId, props: &RunProps<Web>) {
    match props.link.as_deref() {
      Some(href) => {
        let (element, _) = self.create_element(node, "a", Realized::Run);

        for (name, value) in LinkPolicy::default().attributes(href) {
          self.sink.set_attribute(&element, name, &value);
        }
      }
      None => {
        self.create_element(node, "span", Realized::Run);
      }
    }

    let compiled = self
      .book
      .compiled(run_fingerprint(props), || compile_run(props));

    self.assign_style(node, &compiled);
  }

  /// Realizes the anchor `node` as the next anchor's comment.
  pub(crate) fn create_anchor(&mut self, node: NodeId) {
    let text = anchor_text(self.anchors);

    self.anchors += 1;

    let (comment, adopted) = self.sink.create_comment(text);

    self.insert(node, comment, "", Realized::Anchor, adopted);
  }

  /// Realizes the scope `node` as a layout-invisible element.
  pub(crate) fn create_scope(&mut self, node: NodeId) {
    let (element, adopted) = self.sink.create_element("div");

    self.sink.set_attribute(&element, SCOPE_ATTRIBUTE, "");
    self.sink.set_property(&element, "display", "contents");
    self.insert(node, element, "div", Realized::Scope, adopted);
  }

  /// Writes the box slot `slot` of `node` as the custom property its class reads.
  pub(crate) fn update_box(
    &mut self,
    node: NodeId,
    slot: Slot,
    leaf: &StaticLeaf<BoxValue<f32, WebStyle>>,
  ) {
    self.write_slot(node, slot, |count| render_box_leaf(leaf, count));
  }

  /// Writes the run slot `slot` of `node` as the custom property its class reads.
  pub(crate) fn update_run(
    &mut self,
    node: NodeId,
    slot: Slot,
    leaf: &StaticLeaf<RunValue<WebStyle>>,
  ) {
    self.write_slot(node, slot, |count| render_run_leaf(leaf, count));
  }

  /// Writes `focus_order` on the element carrying `node`'s focus.
  pub(crate) fn update_focus_order(&mut self, node: NodeId, focus_order: Option<FocusOrder>) {
    let Some(record) = self.records.get(node)
    else {
      return;
    };

    match focus_order_attribute(focus_order, record.is_control(), record.scrolls) {
      Some(tabindex) => self
        .sink
        .set_attribute(record.carrier(), "tabindex", &tabindex),
      None => self.sink.remove_attribute(record.carrier(), "tabindex"),
    }
  }

  /// Replaces the accessibility attributes of the element carrying `node`'s state with
  /// those `access` maps to, and gives every node it refers to the id the reference
  /// names.
  pub(crate) fn update_access(&mut self, node: NodeId, access: &[AccessProp]) {
    let Some(record) = self.records.get(node)
    else {
      return;
    };
    let carrier = record.carrier();
    let form = is_form_element(record.carrier_tag());

    self.sink.remove_attributes(carrier, |name| {
      replaced_by_access(name) || (form && NATIVE_ACCESS_ATTRIBUTES.contains(&name))
    });

    for (name, value) in access_attributes(access, form) {
      if name == "title" && !S::TITLE_TOOLTIPS {
        continue;
      }

      self.sink.set_attribute(carrier, name, &value);

      if name == "value" {
        self.sink.set_value(carrier, &value);
      }
    }

    for referenced in referenced_nodes(access) {
      if let Some(target) = self.records.get(referenced) {
        self
          .sink
          .set_attribute(target.carrier(), "id", &element_id(referenced));
      }
    }
  }

  /// Writes `state` on the control `node`: on the native element carrying it where it
  /// has one, else as attributes of its own element.
  pub(crate) fn set_control_state(&mut self, node: NodeId, state: ControlState) {
    let Some(record) = self.records.get(node)
    else {
      return;
    };
    let root = &record.element;
    let mixed = state.checked == CheckState::Mixed;

    match &record.carrier {
      None => {
        self
          .sink
          .set_flag(root, "data-skene-checked", state.checked == CheckState::On);
        self.sink.set_flag(root, "data-skene-indeterminate", mixed);
        self
          .sink
          .set_flag(root, "data-skene-disabled", state.disabled);
        self
          .sink
          .set_flag(root, "data-skene-invalid", state.invalid);

        if takes_disabled(record.tag) {
          self.sink.set_flag(root, "disabled", state.disabled);
        }
      }
      Some(carrier) => {
        self
          .sink
          .set_checked(carrier, state.checked == CheckState::On);
        self.sink.set_indeterminate(carrier, mixed);
        self
          .sink
          .set_flag(carrier, "data-skene-indeterminate", mixed);
        self.sink.set_flag(carrier, "disabled", state.disabled);

        if state.invalid {
          self.sink.set_attribute(carrier, "aria-invalid", "true");
        }
        else {
          self.sink.remove_attribute(carrier, "aria-invalid");
        }
      }
    }

    self.sink.set_flag(root, "data-skene-open", state.open);
  }

  /// Gives `node` the class of its compiled layout.
  pub(crate) fn apply_layout(&mut self, node: NodeId, layout: &StaticBoxStyle<Web>) {
    let compiled = self.book.compiled(layout_fingerprint::<Web>(layout), || {
      compile_layout::<Web>(layout)
    });

    self.assign_style(node, &compiled);
  }

  /// Replaces the text of the run `node`, leaving adopted text that already reads
  /// `text`.
  pub(crate) fn set_text(&mut self, node: NodeId, text: &str) {
    let Some(record) = self.records.get(node)
    else {
      return;
    };

    if record.adopted && self.sink.text(&record.element).as_deref() == Some(text) {
      return;
    }

    self.sink.set_text(&record.element, text);
  }

  /// Defines `node`'s tokens: inline over its token class while every
  /// definition is unconditional and the class exists, else as a fresh class.
  ///
  /// The first definition of a node adopted from the server's markup gives up the
  /// definitions the server wrote inline on it: every custom property of a scope, and
  /// those naming one of `definitions` on any other node.
  pub(crate) fn define_tokens(&mut self, node: NodeId, definitions: &Definitions<WebTokenValue>) {
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };

    self.book.register_tokens(definitions);

    let previous = record.token_class.clone();

    if previous.is_none() && record.adopted {
      let scope = matches!(record.realized, Realized::Scope);

      record.inline_tokens = self
        .sink
        .custom_properties(&record.element)
        .into_iter()
        .filter(|property| {
          scope
            || definitions
              .iter()
              .any(|(name, _)| property.strip_prefix("--") == Some(name.as_str()))
        })
        .collect();
    }

    if previous.is_some() && all_unconditional(definitions) {
      Self::write_inline_definitions(&mut self.sink, record, inline_definitions(definitions));

      return;
    }

    let class = self
      .book
      .swap_class(&compile_definitions(definitions), previous.as_deref());

    self
      .sink
      .swap_class(&record.element, previous.as_deref(), &class);
    Self::write_inline_definitions(&mut self.sink, record, Vec::new());
    record.token_class = Some(class);
  }

  /// Defines the root's tokens as the root rule's class.
  pub(crate) fn define_root_tokens(&mut self, definitions: &Definitions<WebTokenValue>) {
    self.book.register_tokens(definitions);

    let previous = self.root_class.take();
    let class = self
      .book
      .swap_class(&compile_root_definitions(definitions), previous.as_deref());

    self.root_class = Some(class);
  }

  /// Sets the value of the form element carrying `node`'s value.
  pub(crate) fn set_value(&mut self, node: NodeId, value: &str) {
    if let Some(record) = self.records.get(node) {
      self.sink.set_value(record.carrier(), value);
    }
  }

  /// Replaces the options of the `<select>` carrying `node`'s value.
  pub(crate) fn set_options(&mut self, node: NodeId, options: &[PickerOption]) {
    if let Some(record) = self.records.get(node) {
      self.sink.set_options(record.carrier(), options);
    }
  }

  /// Sets whether the native toggle carrying `node`'s state is checked.
  pub(crate) fn set_checked(&mut self, node: NodeId, checked: bool) {
    if let Some(record) = self.records.get(node) {
      self.sink.set_checked(record.carrier(), checked);
    }
  }

  /// Sets the attribute `name` of the element realizing `node`.
  pub(crate) fn set_attribute(&mut self, node: NodeId, name: &str, value: &str) {
    if let Some(record) = self.records.get(node) {
      self.sink.set_attribute(&record.element, name, value);
    }
  }

  /// Removes the attribute `name` of the element realizing `node`.
  pub(crate) fn remove_attribute(&mut self, node: NodeId, name: &str) {
    if let Some(record) = self.records.get(node) {
      self.sink.remove_attribute(&record.element, name);
    }
  }

  /// Sets `declarations` inline on the element realizing `node`.
  pub(crate) fn set_declarations(&mut self, node: NodeId, declarations: &Declarations) {
    if let Some(record) = self.records.get(node) {
      for (property, value) in declarations.iter() {
        self.sink.set_property(&record.element, property, value);
      }
    }
  }

  /// Places `child` in the caption `<label>` of `parent` where it is the caption the
  /// control `parent` keeps beside it, making the label, `for` the control where it
  /// stands bare, on the first; the label, or `None` where `child` is no such caption.
  pub(crate) fn enter_caption(&mut self, parent: NodeId, child: NodeId) -> Option<S::Element> {
    let labels = self
      .records
      .get(parent)
      .filter(|record| record.is_control())
      .map(Record::is_bare)?;

    if !self.records.contains(child)
      || Scene::<Web>::read(|scene| scene.caption_beside(parent)) != Some(child)
    {
      return None;
    }

    let label = if let Some(label) = self
      .records
      .get(parent)
      .and_then(|record| record.caption.clone())
    {
      label
    }
    else {
      let (label, _) = self.sink.create_element("label");
      let id = element_id(parent);

      if labels {
        self.sink.set_attribute(&label, "for", &id);
      }

      if let Some(record) = self.records.get_mut(parent) {
        if labels {
          self.sink.set_attribute(&record.element, "id", &id);
        }

        record.caption = Some(label.clone());
      }

      label
    };

    if let Some(record) = self.records.get_mut(child) {
      record.captioned = Some(parent);
    }

    Some(label)
  }

  /// Takes `child` out of the caption `<label>` of the control it captions, dropping
  /// the label; the control and the label, or `None` where it sits in none.
  pub(crate) fn leave_caption(&mut self, child: NodeId) -> Option<(NodeId, S::Element)> {
    let control = self.records.get_mut(child)?.captioned.take()?;
    let label = self.records.get_mut(control)?.caption.take()?;

    self.sink.discard(&label);

    Some((control, label))
  }

  /// Forgets `node`: releases its classes and discards the elements realizing it,
  /// handing its record back.
  pub(crate) fn destroy(&mut self, node: NodeId) -> Option<Record<S>> {
    let record = self.records.remove(node)?;

    for class in record.style_class.iter().chain(&record.token_class) {
      self.book.release(class);
    }

    for element in [
      Some(&record.element),
      record.carrier.as_ref(),
      record.caption.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
      self.sink.discard(element);
    }

    Some(record)
  }

  /// Realizes the props every box-like node shares onto `node`.
  fn finish_box(&mut self, node: NodeId, props: &BoxProps<Web>, compiled: &Rc<Compiled>) {
    self.assign_style(node, compiled);

    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };

    record.scrolls = scrolls::<Web>(&props.style);

    if let Some(axes) = props.drag_axes {
      self
        .sink
        .set_attribute(&record.element, DRAGS_ATTRIBUTE, drags_value(axes));
    }

    self.update_focus_order(node, props.focus_order);
    self.update_access(node, &props.access);
  }

  /// Gives `node` the class of `compiled` in place of its previous one, with the slots
  /// the class references.
  fn assign_style(&mut self, node: NodeId, compiled: &Rc<Compiled>) {
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };

    self.book.register_slots(&compiled.slots);

    let previous = record.style_class.take();
    let class = self
      .book
      .swap_class(&compiled.template, previous.as_deref());

    self
      .sink
      .swap_class(&record.element, previous.as_deref(), &class);
    record.style_class = Some(class);
    record.slots = Rc::from(compiled.slots.as_slice());
  }

  /// Writes the leaf of `node`'s slot `slot`, rendered by `render` into as many texts as
  /// it is given, as the custom properties its class reads.
  fn write_slot(&mut self, node: NodeId, slot: Slot, render: impl FnOnce(usize) -> Vec<String>) {
    let Some(record) = self.records.get(node)
    else {
      return;
    };

    if let Some(binding) = record.slots.iter().find(|binding| binding.slot == slot) {
      let properties = slot_properties(binding.property, binding.position);

      for (property, text) in properties.iter().zip(render(properties.len())) {
        self.sink.set_property(&record.element, property, &text);
      }
    }
  }

  /// Writes the attributes a control's native `element` carries.
  fn write_native_attributes(
    &mut self,
    element: &S::Element,
    native: FormElement,
    props: &ControlProps<Web>,
  ) {
    for (name, value) in native_attributes(native, props) {
      self.sink.set_attribute(element, name, &value);
    }
  }

  /// Replaces a node's inline definitions with `declarations`.
  fn write_inline_definitions(
    sink: &mut S,
    record: &mut Record<S>,
    declarations: Vec<(String, String)>,
  ) {
    for property in record.inline_tokens.drain(..) {
      sink.remove_property(&record.element, &property);
    }

    for (property, value) in declarations {
      sink.set_property(&record.element, &property, &value);
      record.inline_tokens.push(property);
    }
  }
}
