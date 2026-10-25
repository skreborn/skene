//! The realization both web leaves share: one record per node beside the scene, the
//! element choices, attributes, classes, slots and token definitions each node is
//! realized with, and the stylesheet bookkeeping, written once over the [`Sink`] the
//! DOM and the string tree implement.

use std::rc::Rc;

use skene_foundation::{
  AccessProp,
  BoxFacts,
  CheckState,
  Choices,
  Content,
  ContentValue,
  ControlFacts,
  ControlState,
  Definitions,
  DragAxes,
  Edit,
  FocusOrder,
  Keyboard,
  Membership,
  NodeFacts,
  NodeId,
  NodeMap,
  NodeSpec,
  OverlayFacts,
  OverlayPlacement,
  Placement,
  RunFacts,
  Scene,
  Slot,
  SlotLeaf,
  TextSelection,
  TokenScope,
  web::WebStyle,
};

use crate::{
  LinkPolicy,
  compile::{
    Compiled,
    KeptStyle,
    SlotBinding,
    Template,
    all_unconditional,
    compile_definitions,
    compile_root_definitions,
    inline_definitions,
    render_slot_leaf,
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
    keyboard_attributes,
    native_attributes,
    range_attributes,
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

  /// Sets the value of the text entry or range input `element`.
  fn set_value(&mut self, element: &Self::Element, value: &str);

  /// Places the selection of the text entry `element`, in grapheme clusters of its
  /// value.
  fn select(&mut self, element: &Self::Element, selection: TextSelection);

  /// What the text entry `element` shows: its value, with its selection where the sink
  /// keeps one.
  fn shown(&self, element: &Self::Element) -> Option<Edit>;

  /// Whether what the native `element` shows is the reader's until it is reported; see
  /// [`Realization::composing`](skene_foundation::Realization::composing).
  fn composing(&self, element: &Self::Element) -> bool;

  /// Shows `choices` on the `<select>` `element`: its options, and the chosen one
  /// selected, or, where the chosen key names none of them, an empty hidden option first
  /// and selected, keyed by the empty string.
  fn set_choices(&mut self, element: &Self::Element, choices: &Choices);

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
    /// The group a radio belongs to and its key there, as its last membership said.
    membership: Option<Membership>,
    /// The key of the member a group has chosen, as its last chosen content said.
    chosen: Option<Rc<str>>,
    /// The radios whose membership named the control as their group, in the order they
    /// joined, a removed one until the next chosen key is written.
    members: Vec<NodeId>,
    /// Whether a text entry is secure, as its last secure content said.
    secure: bool,
    /// The keyboard a text entry asks for, as its last keyboard content said.
    keyboard: Keyboard,
  },
  /// An overlay.
  Overlay {
    /// Whether it is modal.
    #[cfg_attr(
      skene_server,
      expect(dead_code, reason = "the server renders every overlay closed")
    )]
    modal: bool,
    /// Where it sits, or the point it is pinned at, as its last content said.
    placement: OverlayPlacement,
  },
  /// A text run.
  Run {
    /// How it opens as a link and what it discloses.
    policy: LinkPolicy,
    /// The path it navigates to, where it is a link and one was written.
    destination: Option<Rc<str>>,
  },
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
  /// The facts of its construction its rule template is compiled from, until the
  /// shared half applies its style; `None` for a node that takes none.
  pub template: Option<Template>,
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
        ..
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
      Realized::Control { native: Some(native), .. } if native.is_bare()
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
        template: None,
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

  /// Realizes `node` as `spec` describes it, making a node of every kind, an anchor a
  /// comment and a scope a layout-invisible element; always `true`.
  pub(crate) fn create(&mut self, node: NodeId, spec: &NodeSpec) -> bool {
    match &spec.facts {
      NodeFacts::Box(boxed) => self.make_box(node, spec, boxed),
      NodeFacts::Control(control) => self.make_control(node, spec, control),
      NodeFacts::Overlay(overlay) => self.make_overlay(node, spec, overlay),
      NodeFacts::Run(run) => self.make_run(node, run),
      NodeFacts::Anchor => self.make_anchor(node),
      NodeFacts::Scope => self.make_scope(node),
    }

    true
  }

  /// Realizes the box `node`: an anchor where it is a link, else the element its role
  /// names.
  fn make_box(&mut self, node: NodeId, spec: &NodeSpec, boxed: &BoxFacts) {
    let tag = box_tag(spec.role, spec.level, boxed.link);
    let (element, _) = self.create_element(node, tag, Realized::Box);

    if let Some(role) = spec.role.and_then(aria_role) {
      self.sink.set_attribute(&element, "role", role);
    }

    self.finish_box(node, spec.drag, Template::boxed(boxed));
  }

  /// Realizes the control `node`: a form element bare or inside its label where it
  /// stands in one, else the element it is drawn as.
  fn make_control(&mut self, node: NodeId, spec: &NodeSpec, control: &ControlFacts) {
    let native = form_element(control.native);
    let realized = Realized::Control {
      native,
      membership: None,
      chosen: None,
      members: Vec::new(),
      secure: false,
      keyboard: Keyboard::TEXT,
    };
    let root = match native {
      Some(native) if native.is_bare() => {
        let (root, adopted) = self.create_element(node, native.tag, realized);

        self.write_native_attributes(&root, native, control);

        if adopted {
          self.sink.note_live_state(node, &root, native);
        }

        root
      }
      Some(native) => {
        let (root, _) = self.create_element(node, "label", realized);
        let (input, adopted) = self.sink.create_element(native.tag);

        self.write_native_attributes(&input, native, control);
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
        let (tag, attribute) = drawn_element(control.native, spec.role);
        let (root, _) = self.create_element(node, tag, realized);

        if let Some((name, value)) = attribute {
          self.sink.set_attribute(&root, name, value);
        }

        if let Some(label) = &control.label {
          self.sink.set_attribute(&root, "aria-label", label);
        }

        root
      }
    };

    self.sink.set_attribute(&root, CONTROL_ATTRIBUTE, "");

    self.finish_box(node, spec.drag, Template::control(control));
  }

  /// Realizes the overlay `node`: a `<dialog>` when modal, else a popover.
  fn make_overlay(&mut self, node: NodeId, spec: &NodeSpec, overlay: &OverlayFacts) {
    let realized = Realized::Overlay {
      modal: overlay.modal,
      placement: OverlayPlacement::new(Placement::Center, None),
    };

    if overlay.modal {
      self.create_element(node, "dialog", realized);
    }
    else {
      let (element, _) = self.create_element(node, "div", realized);

      self.sink.set_attribute(
        &element,
        "popover",
        if overlay.hint { "hint" } else { "manual" },
      );

      if let Some(role) = spec.role.and_then(aria_role) {
        self.sink.set_attribute(&element, "role", role);
      }
    }

    self.finish_box(node, spec.drag, Template::overlay(overlay));
  }

  /// Realizes the text run `node`: an anchor where it is a link, else a span.
  fn make_run(&mut self, node: NodeId, run: &RunFacts) {
    let realized = Realized::Run {
      policy: LinkPolicy::default(),
      destination: None,
    };

    self.create_element(node, if run.link { "a" } else { "span" }, realized);

    if let Some(record) = self.records.get_mut(node) {
      record.template = Some(Template::run(run));
    }
  }

  /// Realizes `node`, which a low-level view made, as a new element of `tag`, its style
  /// the per-child layout a wrapper annotated it with.
  pub(crate) fn create_low_level(&mut self, node: NodeId, tag: &'static str) {
    self.create_element(node, tag, Realized::Box);

    if let Some(record) = self.records.get_mut(node) {
      record.template = Some(Template::Layout);
    }
  }

  /// Realizes `node` as `element`, the platform's own view adopted through the native
  /// door: a box stating the role `spec` gives it, which takes the layout a wrapper gives
  /// it and keeps its own look; always `true`.
  #[cfg(not(skene_server))]
  pub(crate) fn adopt(&mut self, node: NodeId, element: &S::Element, spec: &NodeSpec) -> bool {
    self.insert(node, element.clone(), "", Realized::Box, false);

    if let Some(role) = spec.role.and_then(aria_role) {
      self.sink.set_attribute(element, "role", role);
    }

    if let Some(record) = self.records.get_mut(node) {
      record.template = Some(Template::Layout);
    }

    true
  }

  /// Gives `node` the class `style` compiles to under the facts of its construction,
  /// noting whether it scrolls its content; a node that takes no style ignores it.
  pub(crate) fn apply_style(&mut self, node: NodeId, style: &KeptStyle) {
    let Some(template) = self
      .records
      .get_mut(node)
      .and_then(|record| record.template.take())
    else {
      return;
    };
    let compiled = self.book.compiled(style, &template);

    self.assign_style(node, &compiled);

    if let Some(record) = self.records.get_mut(node) {
      record.scrolls = scrolls(style);
    }
  }

  /// Realizes the anchor `node` as the next anchor's comment.
  fn make_anchor(&mut self, node: NodeId) {
    let text = anchor_text(self.anchors);

    self.anchors += 1;

    let (comment, adopted) = self.sink.create_comment(text);

    self.insert(node, comment, "", Realized::Anchor, adopted);
  }

  /// Realizes the scope `node` as a layout-invisible element.
  fn make_scope(&mut self, node: NodeId) {
    let (element, adopted) = self.sink.create_element("div");

    self.sink.set_attribute(&element, SCOPE_ATTRIBUTE, "");
    self.sink.set_property(&element, "display", "contents");
    self.insert(node, element, "div", Realized::Scope, adopted);
  }

  /// Writes the leaf of `node`'s slot `slot` as the custom properties its class reads.
  pub(crate) fn apply_slot(&mut self, node: NodeId, slot: Slot, leaf: &SlotLeaf<f32, WebStyle>) {
    let Some(record) = self.records.get(node)
    else {
      return;
    };

    if let Some(binding) = record.slots.iter().find(|binding| binding.slot == slot) {
      let properties = slot_properties(binding.property, binding.position);

      for (property, text) in properties
        .iter()
        .zip(render_slot_leaf(leaf, properties.len()))
      {
        self.sink.set_property(&record.element, property, &text);
      }
    }
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
  /// has one, whose checked state is its content's, else as attributes of its own
  /// element.
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

  /// Defines tokens on `scope`, replacing what it defined before: a node's as its token
  /// class or inline over it, the root's as the root rule's class.
  pub(crate) fn define_tokens(
    &mut self,
    scope: TokenScope,
    definitions: &Definitions<WebTokenValue>,
  ) {
    match scope {
      TokenScope::Node(node) => self.define_node_tokens(node, definitions),
      TokenScope::Root => {
        self.book.register_tokens(definitions);

        let previous = self.root_class.take();
        let class = self
          .book
          .swap_class(&compile_root_definitions(definitions), previous.as_deref());

        self.root_class = Some(class);
      }
    }
  }

  /// Defines `node`'s tokens: inline over its token class while every
  /// definition is unconditional and the class exists, else as a fresh class.
  ///
  /// The first definition of a node adopted from the server's markup gives up the
  /// definitions the server wrote inline on it: every custom property of a scope, and
  /// those naming one of `definitions` on any other node.
  fn define_node_tokens(&mut self, node: NodeId, definitions: &Definitions<WebTokenValue>) {
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

  /// Shows `value` as `node`'s content, whole: a run's text as its own, a link's
  /// destination as its `href` under its policy, an overlay's placement where it opens,
  /// a group's chosen key as the checked state of its members, and a control's on the
  /// form element carrying it, a range's bounds and step before its value and a radio's
  /// membership as its `name`, an entry's secure flag and keyboard as its `type` and its
  /// input method's attributes; a control drawn without one shows its content through
  /// its parts, and a door's content is its sink's, shown by nothing here.
  pub(crate) fn write(&mut self, node: NodeId, value: ContentValue<'_>) {
    match value {
      ContentValue::Secure(_) | ContentValue::Keyboard(_) => return self.write_entry(node, value),
      ContentValue::Text(text) => return self.set_text(node, text),
      ContentValue::Destination(path) => return self.write_destination(node, path),
      ContentValue::Placement(placement) => return self.place_overlay(node, placement),
      ContentValue::Chosen(chosen) => return self.write_chosen(node, chosen),
      ContentValue::Membership(membership) => return self.write_membership(node, membership),
      ContentValue::Door { .. } => return,
      _ => {}
    }

    let Some(record) = self.records.get(node)
    else {
      return;
    };
    let Realized::Control {
      native: Some(_), ..
    } = record.realized
    else {
      return;
    };
    let carrier = record.carrier();

    match value {
      ContentValue::Text(_)
      | ContentValue::Destination(_)
      | ContentValue::Placement(_)
      | ContentValue::Chosen(_)
      | ContentValue::Membership(_)
      | ContentValue::Secure(_)
      | ContentValue::Keyboard(_)
      | ContentValue::Door { .. } => {}
      ContentValue::Edit(edit) => {
        self.sink.set_value(carrier, &edit.text);

        if let Some(selection) = edit.selection {
          self.sink.select(carrier, selection);
        }
      }
      ContentValue::Checked(checked) => {
        self.sink.set_checked(carrier, checked == CheckState::On);
        self
          .sink
          .set_indeterminate(carrier, checked == CheckState::Mixed);
      }
      ContentValue::Range(range) => {
        for (name, value) in range_attributes(range) {
          self.sink.set_attribute(carrier, name, &value);

          if name == "value" {
            self.sink.set_value(carrier, &value);
          }
        }
      }
      ContentValue::Choice(choices) => self.sink.set_choices(carrier, choices),
      ContentValue::Placeholder(hint) => self.sink.set_attribute(carrier, "placeholder", hint),
    }
  }

  /// Runs `read` with what `node` shows of `content`: a text entry's text with its
  /// selection; `None` for any other content, and for a node showing none.
  pub(crate) fn read<R>(
    &self,
    node: NodeId,
    content: Content,
    read: impl FnOnce(ContentValue<'_>) -> R,
  ) -> Option<R> {
    let record = self.records.get(node)?;
    let Realized::Control {
      native: Some(native),
      ..
    } = record.realized
    else {
      return None;
    };

    if content != Content::Edit || !native.is_text_entry() {
      return None;
    }

    let shown = self.sink.shown(record.carrier())?;

    Some(read(ContentValue::Edit(&shown)))
  }

  /// Whether what the control `node` shows is the reader's until it is reported.
  pub(crate) fn composing(&self, node: NodeId) -> bool {
    self
      .records
      .get(node)
      .is_some_and(|record| self.sink.composing(record.carrier()))
  }

  /// Notes `chosen` as the key the group `group` has chosen and checks the member whose
  /// key it is, unchecking every other, but a member showing what the reader changed
  /// until it is reported.
  fn write_chosen(&mut self, group: NodeId, chosen: Option<&str>) {
    let members = self.members(group);

    if let Some(Record {
      realized:
        Realized::Control {
          chosen: held,
          members: joined,
          ..
        },
      ..
    }) = self.records.get_mut(group)
    {
      *held = chosen.map(Rc::from);
      joined.clone_from(&members);
    }

    for member in members {
      if let Some(record) = self.records.get(member)
        && let Realized::Control {
          membership: Some(membership),
          ..
        } = &record.realized
        && !self.sink.composing(record.carrier())
      {
        self
          .sink
          .set_checked(record.carrier(), chosen == Some(&*membership.key));
      }
    }
  }

  /// The radios whose membership names `group` as their group, in the order they
  /// joined it.
  pub(crate) fn members(&self, group: NodeId) -> Vec<NodeId> {
    let Some(Record {
      realized: Realized::Control { members, .. },
      ..
    }) = self.records.get(group)
    else {
      return Vec::new();
    };

    members
      .iter()
      .copied()
      .filter(|member| {
        matches!(self.records.get(*member), Some(Record {
          realized: Realized::Control {
            membership: Some(membership),
            ..
          },
          ..
        }) if membership.group == group)
      })
      .collect()
  }

  /// Gives the radio `node` its group's `name` and its key there as its `value`, and
  /// checks it where it is the member its group has chosen, unless it shows what the
  /// reader changed until it is reported.
  fn write_membership(&mut self, node: NodeId, membership: &Membership) {
    let chosen = match self.records.get_mut(membership.group) {
      Some(Record {
        realized: Realized::Control {
          chosen, members, ..
        },
        ..
      }) => {
        if !members.contains(&node) {
          members.push(node);
        }

        chosen.clone()
      }
      _ => None,
    };
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };

    if let Realized::Control {
      membership: held, ..
    } = &mut record.realized
    {
      *held = Some(membership.clone());
    }

    let carrier = record.carrier().clone();

    self
      .sink
      .set_attribute(&carrier, "name", &element_id(membership.group));
    self.sink.set_attribute(&carrier, "value", &membership.key);

    if !self.sink.composing(&carrier) {
      self
        .sink
        .set_checked(&carrier, chosen.as_deref() == Some(&*membership.key));
    }
  }

  /// Shows the text entry `node`'s secure flag or keyboard, as `value` says, on its form
  /// element: its `type` where it changes, keeping the value and the selection shown, and
  /// the keyboard's attributes beside it.
  fn write_entry(&mut self, node: NodeId, value: ContentValue<'_>) {
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };
    let Realized::Control {
      native: Some(native),
      secure,
      keyboard,
      ..
    } = &mut record.realized
    else {
      return;
    };
    let before = native.entry_type(*secure, *keyboard);

    match value {
      ContentValue::Secure(written) => *secure = written,
      ContentValue::Keyboard(written) => *keyboard = written,
      _ => return,
    }

    let after = native.entry_type(*secure, *keyboard);
    let attributes =
      matches!(value, ContentValue::Keyboard(_)).then(|| keyboard_attributes(*keyboard));
    let carrier = record.carrier().clone();

    if let Some(kind) = after.filter(|_| after != before) {
      let shown = self.sink.shown(&carrier).and_then(|shown| shown.selection);

      self.sink.set_attribute(&carrier, "type", kind);

      if let Some(selection) = shown
        && self.sink.shown(&carrier).and_then(|now| now.selection) != Some(selection)
      {
        self.sink.select(&carrier, selection);
      }
    }

    for (name, value) in attributes.into_iter().flatten() {
      match value {
        Some(value) => self.sink.set_attribute(&carrier, name, value),
        None => self.sink.remove_attribute(&carrier, name),
      }
    }
  }

  /// Gives the link `node` the destination `path`: a run's `href`, `target` and `rel`
  /// as its policy has them, a box's `href`, neither `href` where following it would
  /// run script.
  fn write_destination(&mut self, node: NodeId, path: &str) {
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };

    match &mut record.realized {
      Realized::Run {
        policy,
        destination,
      } => {
        let previous = destination
          .replace(Rc::from(path))
          .map(|previous| policy.attributes(&previous))
          .unwrap_or_default();
        let next = policy.attributes(path);

        Self::rewrite_attributes(&mut self.sink, &record.element, &previous, &next);
      }
      Realized::Box if runs_script(path) => self.sink.remove_attribute(&record.element, "href"),
      Realized::Box => self.sink.set_attribute(&record.element, "href", path),
      _ => {}
    }
  }

  /// Opens the link run `node` as `policy` says from now on, rewriting the attributes
  /// of the destination it holds.
  pub(crate) fn set_link_policy(&mut self, node: NodeId, next: LinkPolicy) {
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };
    let Realized::Run {
      policy,
      destination,
    } = &mut record.realized
    else {
      return;
    };
    let previous = *policy;

    *policy = next;

    if let Some(path) = destination.as_deref() {
      Self::rewrite_attributes(
        &mut self.sink,
        &record.element,
        &previous.attributes(path),
        &next.attributes(path),
      );
    }
  }

  /// Replaces the attributes `previous` of `element` with `next`, removing those `next`
  /// leaves out and writing the rest.
  fn rewrite_attributes(
    sink: &mut S,
    element: &S::Element,
    previous: &[(&'static str, String)],
    next: &[(&'static str, String)],
  ) {
    for (name, _) in previous {
      if next.iter().all(|(kept, _)| kept != name) {
        sink.remove_attribute(element, name);
      }
    }

    for (name, value) in next {
      sink.set_attribute(element, name, value);
    }
  }

  /// Places the overlay `node` as `placement` says when it next opens, its margins
  /// against the viewport compiled from it until its style is applied.
  fn place_overlay(&mut self, node: NodeId, placement: OverlayPlacement) {
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };

    if let Realized::Overlay {
      placement: held, ..
    } = &mut record.realized
    {
      *held = placement;
    }

    if let Some(Template::Overlay {
      placement: compiled,
      ..
    }) = &mut record.template
    {
      *compiled = placement.placement;
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

  /// Realizes what every box-like node shares onto `node`: the axes a press drags it
  /// along, and `template`, kept for its style.
  fn finish_box(&mut self, node: NodeId, drag: Option<DragAxes>, template: Template) {
    let Some(record) = self.records.get_mut(node)
    else {
      return;
    };

    record.template = Some(template);

    if let Some(axes) = drag {
      self
        .sink
        .set_attribute(&record.element, DRAGS_ATTRIBUTE, drags_value(axes));
    }
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

  /// Writes the attributes a control's native `element` carries.
  fn write_native_attributes(
    &mut self,
    element: &S::Element,
    native: FormElement,
    control: &ControlFacts,
  ) {
    for (name, value) in native_attributes(native, control) {
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
