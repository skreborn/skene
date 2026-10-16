//! [`Linux`] as the seam's realization: every method a GTK call.

use std::{cell::Cell, process::ExitCode, rc::Rc, time::Duration};

use gtk::{glib, prelude::*};
use skene_foundation::{
  NodeId, Scheme, State,
  layout::ResolvedLayout,
  seam::{
    LayoutOwner, Realization,
    access::AccessFacts,
    content::{Content, Edit, Graphemes, Pixels, PropValue, Range, TextSelection, Unit},
    node::{LayoutKind, NodeKind, NodeSpec},
    report::{Interest, Report},
    runtime::{Cancel, Task},
    style::{CompiledStyle, Keep, KeptDefinition, KeptLeaf, ScopeId},
  },
};

use crate::{
  Linux, Node, SkeneBox, SkeneButton, SkenePainted, access, app,
  controls::{
    buffer_text, caret_anchor, choice, configure, control_widget, markup, number_range,
    splice_entry, splice_view, write_choice,
  },
  environment, events, handled, node_of, report, sheet, slot,
};

impl Realization for Linux {
  type Node = Node;
  type Widget = gtk::Widget;

  const LAYOUT: LayoutOwner = LayoutOwner::Engine;
  const KEEP: Keep = Keep::ColorsAndStates;
  const UNIT: Unit = Unit::CodePoint;
  const SCOPE_NODES: bool = false;

  fn create(id: NodeId, spec: &NodeSpec) -> Node {
    let role = access::accessible_role(spec.role);
    let widget: gtk::Widget = match spec.kind {
      NodeKind::Layout(_) if spec.acts => {
        crate::sheet::install();

        let button: SkeneButton = glib::Object::builder()
          .property("accessible-role", role)
          .build();

        button.connect_clicked(move |button| {
          let destination = slot(button.upcast_ref())
            .extra
            .borrow()
            .get(&Content::Destination)
            .cloned();

          match destination {
            Some(PropValue::Text(destination)) => {
              if !handled(&report(id, Report::Follow(Rc::clone(&destination)))) {
                gtk::UriLauncher::new(&destination).launch(
                  button.root().and_downcast_ref::<gtk::Window>(),
                  None::<&gtk::gio::Cancellable>,
                  |_| {},
                );
              }
            }
            _ => {
              report(id, Report::Activate);
            }
          }
        });

        button.upcast()
      }
      NodeKind::Layout(LayoutKind::Scroll {
        horizontal,
        vertical,
      }) => {
        let policy = |scrolls: bool| {
          if scrolls {
            gtk::PolicyType::Automatic
          } else {
            gtk::PolicyType::Never
          }
        };
        let scrolled: gtk::ScrolledWindow = glib::Object::builder()
          .property("accessible-role", role)
          .build();

        scrolled.set_policy(policy(horizontal), policy(vertical));
        scrolled.set_propagate_natural_width(true);
        scrolled.set_propagate_natural_height(true);
        scrolled.upcast()
      }
      NodeKind::Layout(_) | NodeKind::Scope | NodeKind::Segment | NodeKind::Native => {
        glib::Object::builder::<SkeneBox>()
          .property("accessible-role", role)
          .build()
          .upcast()
      }
      NodeKind::Text => {
        let label = gtk::Label::new(None);

        label.set_wrap(true);
        label.set_xalign(0.);
        label.connect_activate_link(move |_, uri| {
          if handled(&report(id, Report::Follow(uri.into()))) {
            glib::Propagation::Stop
          } else {
            glib::Propagation::Proceed
          }
        });

        label.upcast()
      }
      NodeKind::Image => gtk::Picture::new().upcast(),
      NodeKind::Painted => glib::Object::builder::<SkenePainted>()
        .property("accessible-role", role)
        .build()
        .upcast(),
      NodeKind::Control(control) => control_widget(id, control),
    };
    let slot = slot(&widget);

    slot.id.set(Some(id));

    match spec.kind {
      NodeKind::Layout(LayoutKind::Scroll { .. }) => neutralize(&widget),
      NodeKind::Layout(kind) => slot.container.set(Some(kind)),
      _ => neutralize(&widget),
    }

    Node { widget }
  }

  fn adopt(id: NodeId, widget: gtk::Widget, _: &NodeSpec) -> Node {
    neutralize(&widget);
    slot(&widget).id.set(Some(id));

    Node { widget }
  }

  fn widget(node: &Node) -> gtk::Widget {
    node.widget.clone()
  }

  fn insert(parent: &Node, child: &Node, index: usize) {
    place(&parent.widget, &child.widget, index);
  }

  fn relocate(parent: &Node, child: &Node, index: usize) {
    place(&parent.widget, &child.widget, index);
  }

  fn set_hidden(node: &Node, hidden: bool) {
    node.widget.set_visible(!hidden);
  }

  fn remove(node: &Node) {
    sheet::release(&node.widget);
    detach(&node.widget);
  }

  fn apply_style(node: &Node, style: &CompiledStyle) {
    sheet::apply(&node.widget, style);
  }

  fn apply_slots(node: &Node, slots: &[KeptLeaf]) {
    sheet::write_slots(&node.widget, slots);
  }

  fn apply_layout(node: &Node, layout: &ResolvedLayout) {
    *slot(&node.widget).layout.borrow_mut() = layout.clone();
    node.widget.queue_resize();
  }

  fn define_tokens(scope: ScopeId, _: Option<&Node>, tokens: &[KeptDefinition]) {
    sheet::define_tokens(scope, tokens);
  }

  fn write(node: &Node, content: Content, value: &PropValue) {
    let widget = &node.widget;

    match (content, value) {
      (Content::Text, PropValue::Text(text)) => {
        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
          label.set_use_markup(false);
          label.set_label(text);
        }
      }
      (Content::Runs, PropValue::Runs(runs)) => {
        if let Some(label) = widget.downcast_ref::<gtk::Label>() {
          label.set_markup(&markup(runs));
        }
      }
      (Content::Caption, PropValue::Text(text)) => {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
          button.set_label(text);
        } else if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
          check.set_label(Some(text));
        }
      }
      (Content::On, PropValue::Bool(on)) => {
        if let Some(switch) = widget.downcast_ref::<gtk::Switch>() {
          switch.set_active(*on);
          switch.set_state(*on);
        } else if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
          check.set_active(*on);
        }
      }
      (Content::Mixed, PropValue::Bool(mixed)) => {
        if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
          check.set_inconsistent(*mixed);
        }
      }
      (Content::Range, PropValue::Range(range)) => {
        if let Some(scale) = widget.downcast_ref::<gtk::Range>() {
          configure(&scale.adjustment(), range);
        } else if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
          configure(&spin.adjustment(), range);
        } else if let Some(progress) = widget.downcast_ref::<gtk::ProgressBar>() {
          let span = range.maximum - range.minimum;

          progress.set_fraction(if span > 0. {
            (range.value - range.minimum) / span
          } else {
            0.
          });
          slot(widget)
            .extra
            .borrow_mut()
            .insert(content, value.clone());
        }
      }
      (Content::Choice, PropValue::Choice(truth)) => {
        if let Some(dropdown) = widget.downcast_ref::<gtk::DropDown>() {
          write_choice(dropdown, truth);
        }
      }
      (Content::Edit, PropValue::Edit(truth)) => {
        if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
          splice_entry(entry, truth);
        } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
          splice_view(view, truth);
        }
      }
      (Content::Placeholder, PropValue::Text(text)) => {
        if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
          entry.set_placeholder_text(Some(text));
        }
      }
      (Content::Enabled, PropValue::Bool(enabled)) => widget.set_sensitive(*enabled),
      (Content::ReadOnly, PropValue::Bool(read_only)) => {
        if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
          entry.set_editable(!read_only);
        } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
          view.set_editable(!read_only);
        }
      }
      (Content::Secure, PropValue::Bool(secure)) => {
        if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
          entry.set_visibility(!secure);
        }
      }
      (Content::Source, PropValue::Text(path)) => {
        if let Some(picture) = widget.downcast_ref::<gtk::Picture>() {
          picture.set_filename(Some(path.as_ref()));
        }
      }
      (Content::Pixels, PropValue::Pixels(pixels)) => {
        if let Some(painted) = widget.downcast_ref::<SkenePainted>() {
          let texture = (pixels.width > 0 && pixels.height > 0).then(|| {
            gtk::gdk::MemoryTexture::new(
              pixels.width as i32,
              pixels.height as i32,
              gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied,
              &glib::Bytes::from(&pixels.data[..]),
              pixels.width as usize * 4,
            )
            .upcast()
          });

          painted.show(texture, pixels.scale);
        }
      }
      (Content::Destination, PropValue::Text(_)) => {
        slot(widget)
          .extra
          .borrow_mut()
          .insert(content, value.clone());
      }
      _ => debug_assert!(
        false,
        "{content:?} with {value:?} on {}",
        widget.type_().name()
      ),
    }
  }

  fn read(node: &Node, content: Content) -> Option<PropValue> {
    let widget = &node.widget;
    let text = |text: &str| Some(PropValue::Text(text.into()));

    match content {
      Content::Text => widget
        .downcast_ref::<gtk::Label>()
        .and_then(|label| text(&label.label())),
      Content::Runs => None,
      Content::Caption => widget
        .downcast_ref::<gtk::Button>()
        .and_then(|button| button.label())
        .or_else(|| {
          widget
            .downcast_ref::<gtk::CheckButton>()
            .and_then(|check| check.label())
        })
        .and_then(|label| text(&label)),
      Content::On => widget
        .downcast_ref::<gtk::Switch>()
        .map(|switch| PropValue::Bool(switch.is_active()))
        .or_else(|| {
          widget
            .downcast_ref::<gtk::CheckButton>()
            .map(|check| PropValue::Bool(check.is_active()))
        }),
      Content::Mixed => widget
        .downcast_ref::<gtk::CheckButton>()
        .map(|check| PropValue::Bool(check.is_inconsistent())),
      Content::Range => {
        if let Some(range) = widget.downcast_ref::<gtk::Range>() {
          Some(PropValue::Range(number_range(
            &range.adjustment(),
            range.value(),
          )))
        } else if let Some(spin) = widget.downcast_ref::<gtk::SpinButton>() {
          Some(PropValue::Range(number_range(
            &spin.adjustment(),
            spin.value(),
          )))
        } else if let Some(progress) = widget.downcast_ref::<gtk::ProgressBar>() {
          match slot(widget).extra.borrow().get(&content) {
            Some(PropValue::Range(range)) => Some(PropValue::Range(Range {
              value: range.minimum + progress.fraction() * (range.maximum - range.minimum),
              ..*range
            })),
            _ => None,
          }
        } else {
          None
        }
      }
      Content::Choice => widget
        .downcast_ref::<gtk::DropDown>()
        .map(|dropdown| PropValue::Choice(choice(dropdown))),
      Content::Edit => {
        if let Some(entry) = widget.downcast_ref::<gtk::Entry>() {
          let shown: String = entry.text().into();
          let (caret, anchor) = caret_anchor(entry);

          Some(PropValue::Edit(Edit {
            selection: Some(TextSelection {
              start: Graphemes::cluster(&shown, Unit::CodePoint, anchor),
              end: Graphemes::cluster(&shown, Unit::CodePoint, caret),
            }),
            text: shown.into(),
          }))
        } else if let Some(view) = widget.downcast_ref::<gtk::TextView>() {
          let buffer = view.buffer();
          let shown = buffer_text(&buffer);
          let offset = |mark: &gtk::TextMark| buffer.iter_at_mark(mark).offset().max(0) as usize;

          Some(PropValue::Edit(Edit {
            selection: Some(TextSelection {
              start: Graphemes::cluster(&shown, Unit::CodePoint, offset(&buffer.selection_bound())),
              end: Graphemes::cluster(&shown, Unit::CodePoint, offset(&buffer.get_insert())),
            }),
            text: shown.into(),
          }))
        } else {
          None
        }
      }
      Content::Placeholder => widget
        .downcast_ref::<gtk::Entry>()
        .and_then(|entry| entry.placeholder_text())
        .and_then(|placeholder| text(&placeholder)),
      Content::Enabled => Some(PropValue::Bool(widget.is_sensitive())),
      Content::ReadOnly => widget
        .downcast_ref::<gtk::Entry>()
        .map(|entry| PropValue::Bool(!entry.is_editable()))
        .or_else(|| {
          widget
            .downcast_ref::<gtk::TextView>()
            .map(|view| PropValue::Bool(!view.is_editable()))
        }),
      Content::Secure => widget
        .downcast_ref::<gtk::Entry>()
        .map(|entry| PropValue::Bool(!entry.property::<bool>("visibility"))),
      Content::Source => widget
        .downcast_ref::<gtk::Picture>()
        .and_then(|picture| picture.file())
        .and_then(|file| file.path())
        .and_then(|path| text(&path.to_string_lossy())),
      Content::Destination => slot(widget).extra.borrow().get(&content).cloned(),
      Content::Pixels => {
        let (texture, scale) = widget.downcast_ref::<SkenePainted>()?.shown();
        let Some(texture) = texture else {
          return Some(PropValue::Pixels(Pixels {
            width: 0,
            height: 0,
            scale,
            data: Rc::from([].as_slice()),
          }));
        };
        let mut downloader = gtk::gdk::TextureDownloader::new(&texture);

        downloader.set_format(gtk::gdk::MemoryFormat::R8g8b8a8Premultiplied);

        let (bytes, stride) = downloader.download_bytes();
        let (width, height) = (texture.width() as usize, texture.height() as usize);
        let data: Vec<u8> = (0..height)
          .flat_map(|row| {
            bytes[row * stride..row * stride + width * 4]
              .iter()
              .copied()
          })
          .collect();

        Some(PropValue::Pixels(Pixels {
          width: width as u32,
          height: height as u32,
          scale,
          data: data.into(),
        }))
      }
      Content::Door(_) => None,
    }
  }

  fn composing(node: &Node) -> bool {
    slot(&node.widget).composing.get()
  }

  fn apply_access(
    node: &Node,
    previous: &AccessFacts,
    next: &AccessFacts,
    related: &dyn Fn(NodeId) -> Option<Node>,
  ) {
    access::apply_access(node, previous, next, related);
  }

  fn watch(node: &Node, interest: Interest) {
    let Some(id) = node_of(&node.widget) else {
      return;
    };
    let slot = slot(&node.widget);
    let installed = slot.watched.get();
    let wanted = [
      (interest.states.contains(State::Hovered), WATCH_HOVER),
      (interest.pointer, WATCH_POINTER),
      (interest.keys, WATCH_KEYS),
      (interest.focus, WATCH_FOCUS),
      (interest.size, WATCH_SIZE),
    ];

    for (wanted, bit) in wanted {
      if !wanted || installed & bit != 0 {
        continue;
      }

      match bit {
        WATCH_HOVER => events::watch_hover(&node.widget, id),
        WATCH_POINTER => events::watch_pointer(&node.widget),
        WATCH_KEYS => events::watch_keys(&node.widget),
        WATCH_FOCUS => events::watch_focus(&node.widget, id),
        _ => {
          node.widget.connect_scale_factor_notify(|widget| {
            crate::report_size(widget, widget.width(), widget.height());
          });
          node.widget.connect_realize(follow_surface_scale);

          if node.widget.is_realized() {
            follow_surface_scale(&node.widget);
          }
        }
      }

      slot.watched.set(slot.watched.get() | bit);

      if bit == WATCH_SIZE && node.widget.width() > 0 {
        crate::report_size(&node.widget, node.widget.width(), node.widget.height());
      }
    }
  }

  fn focus(node: &Node) {
    node.widget.grab_focus();
  }

  fn set_scheme(choice: Option<Scheme>) {
    environment::set_scheme(choice);
  }

  fn spawn_local(task: Task) -> Cancel {
    let handle = glib::MainContext::default().spawn_local(task);

    Cancel(Some(Box::new(move || handle.abort())))
  }

  fn schedule(delay: Duration, action: Box<dyn FnOnce()>) -> Cancel {
    let fired = Rc::new(Cell::new(false));
    let flag = Rc::clone(&fired);
    let source = glib::timeout_add_local_once(delay, move || {
      flag.set(true);
      action();
    });

    Cancel(Some(Box::new(move || {
      if !fired.get() {
        source.remove();
      }
    })))
  }

  fn start(app_id: &str, root: Box<dyn FnOnce() -> NodeId>) -> ExitCode {
    app::start(app_id, root)
  }
}

/// Reports `widget`'s size again whenever its surface's fractional scale changes, which may
/// leave its integer scale factor as it was.
fn follow_surface_scale(widget: &gtk::Widget) {
  let Some(surface) = widget.native().and_then(|native| native.surface()) else {
    return;
  };
  let weak = widget.downgrade();

  surface.connect_scale_notify(move |_| {
    if let Some(widget) = weak.upgrade() {
      crate::report_size(&widget, widget.width(), widget.height());
    }
  });
}

/// Makes a toolkit widget inside a Skene container a leaf the engine places: its own alignment
/// and margins neutral.
fn neutralize(widget: &gtk::Widget) {
  widget.set_halign(gtk::Align::Fill);
  widget.set_valign(gtk::Align::Fill);
  widget.set_margin_top(0);
  widget.set_margin_bottom(0);
  widget.set_margin_start(0);
  widget.set_margin_end(0);
}

fn place(parent: &gtk::Widget, child: &gtk::Widget, index: usize) {
  if let Some(scrolled) = parent.downcast_ref::<gtk::ScrolledWindow>() {
    scrolled.set_child(Some(child));

    return;
  }

  let siblings: Vec<gtk::Widget> = crate::children(parent)
    .into_iter()
    .filter(|sibling| sibling != child)
    .collect();
  let previous = index.checked_sub(1).and_then(|index| siblings.get(index));

  if child.parent().is_some() && child.parent().as_ref() != Some(parent) {
    detach(child);
  }

  child.insert_after(parent, previous);
  parent.queue_resize();
}

fn detach(widget: &gtk::Widget) {
  match widget.parent() {
    Some(parent) if parent.is::<gtk::Viewport>() => {
      if let Some(scrolled) = parent.parent().and_downcast::<gtk::ScrolledWindow>() {
        scrolled.set_child(None::<&gtk::Widget>);
      }
    }
    Some(parent) if parent.is::<gtk::ScrolledWindow>() => {
      if let Some(scrolled) = parent.downcast_ref::<gtk::ScrolledWindow>() {
        scrolled.set_child(None::<&gtk::Widget>);
      }
    }
    Some(parent) if parent.is::<gtk::Window>() => {
      if let Some(window) = parent.downcast_ref::<gtk::Window>() {
        window.set_child(None::<&gtk::Widget>);
      }
    }
    Some(_) => widget.unparent(),
    None => {}
  }
}

const WATCH_HOVER: u8 = 1;
const WATCH_POINTER: u8 = 2;
const WATCH_KEYS: u8 = 4;
const WATCH_FOCUS: u8 = 8;
/// The interest bit of a node's size ([`Interest::size`]).
pub(crate) const WATCH_SIZE: u8 = 16;
