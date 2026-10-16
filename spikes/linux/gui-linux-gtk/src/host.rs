//! The engine's host container: [`SkeneLayout`], the layout manager every Skene container
//! uses, runs the foundation's engine over the container's children, measuring each toolkit
//! widget through GTK's own `measure`; [`SkeneBox`] is a container that does not act (CSS name
//! `skenebox`), [`SkeneButton`] one that does (GTK's button, CSS name `skenebutton`, so the
//! theme's button chrome stays off it). [`SkenePainted`] shows a painted
//! node's pixels (CSS name `skenepainted`).

use gtk::{glib, prelude::*, subclass::prelude::ObjectSubclassIsExt as _};
use skene_foundation::layout::{Axis, LayoutHost, Measurement, Rect};

use crate::slot;

/// GTK as the engine's host.
pub struct Gtk;

impl LayoutHost for Gtk {
  type Child = gtk::Widget;

  fn measure(child: &gtk::Widget, axis: Axis, for_size: Option<f32>) -> Measurement {
    let orientation = match axis {
      Axis::Horizontal => gtk::Orientation::Horizontal,
      Axis::Vertical => gtk::Orientation::Vertical,
    };
    let (minimum, natural, _, baseline) =
      child.measure(orientation, for_size.map_or(-1, |size| size as i32));

    Measurement {
      minimum: minimum as f32,
      natural: natural as f32,
      baseline: (baseline >= 0).then_some(baseline as f32),
    }
  }

  fn constant_height(child: &gtk::Widget) -> bool {
    child.request_mode() == gtk::SizeRequestMode::ConstantSize
  }

  fn is_visible(child: &gtk::Widget) -> bool {
    child.is_visible()
  }

  fn is_container(child: &gtk::Widget) -> bool {
    slot(child).container.get().is_some()
  }

  fn allocate(child: &gtk::Widget, rect: Rect, fractional: Rect, _: Option<f32>) {
    let slot = slot(child);

    if slot.container.get().is_some() {
      slot.hint.set(Some(fractional));
    }

    child.size_allocate(
      &gtk::Allocation::new(
        rect.x as i32,
        rect.y as i32,
        rect.width as i32,
        rect.height as i32,
      ),
      -1,
    );
  }
}

mod imp {
  use std::cell::RefCell;

  use gtk::{glib, prelude::*, subclass::prelude::*};
  use skene_foundation::{
    layout::{Axis, ContainerLayout, Rect, ResolvedLayout},
    seam::node::LayoutKind,
  };

  use super::Gtk;
  use crate::slot;

  /// The layout manager every Skene container uses: the engine over its children.
  #[derive(Default)]
  pub struct SkeneLayout {
    /// The engine, built on first use.
    pub engine: RefCell<Option<ContainerLayout<Gtk>>>,
  }

  #[glib::object_subclass]
  impl ObjectSubclass for SkeneLayout {
    const NAME: &'static str = "SkeneLayout";
    type Type = super::SkeneLayout;
    type ParentType = gtk::LayoutManager;
  }

  impl ObjectImpl for SkeneLayout {}

  impl SkeneLayout {
    fn with_engine<T>(
      &self,
      widget: &gtk::Widget,
      body: impl FnOnce(&mut ContainerLayout<Gtk>) -> T,
    ) -> T {
      let own = slot(widget);
      let kind = own.container.get().unwrap_or(LayoutKind::Box);
      let mut children = Vec::new();
      let mut next = widget.first_child();

      while let Some(child) = next {
        next = child.next_sibling();

        let placement = slot(&child).layout.borrow().clone();

        children.push((child, placement));
      }

      let mut engine = self.engine.borrow_mut();
      let engine =
        engine.get_or_insert_with(|| ContainerLayout::new(kind, ResolvedLayout::default()));

      engine.set_own(own.layout.borrow().clone());
      engine.set_rtl(widget.direction() == gtk::TextDirection::Rtl);
      engine.set_children(children);

      body(engine)
    }
  }

  impl LayoutManagerImpl for SkeneLayout {
    fn request_mode(&self, _: &gtk::Widget) -> gtk::SizeRequestMode {
      gtk::SizeRequestMode::HeightForWidth
    }

    fn measure(
      &self,
      widget: &gtk::Widget,
      orientation: gtk::Orientation,
      for_size: i32,
    ) -> (i32, i32, i32, i32) {
      let axis = match orientation {
        gtk::Orientation::Horizontal => Axis::Horizontal,
        _ => Axis::Vertical,
      };
      let measured = self.with_engine(widget, |engine| {
        engine.measure(axis, (for_size >= 0).then_some(for_size as f32))
      });
      let (minimum, natural) = (measured.minimum as i32, measured.natural as i32);
      let baseline = measured
        .baseline
        .map_or(-1, |baseline| (baseline.round() as i32).min(natural));

      // GTK holds each baseline inside the size it goes with.
      (minimum, natural, baseline.min(minimum), baseline)
    }

    fn allocate(&self, widget: &gtk::Widget, width: i32, height: i32, _: i32) {
      let hint = slot(widget).hint.take().map(|hint| {
        let Some(bounds) = widget.compute_bounds(widget) else {
          return hint;
        };
        let (left, top) = (-bounds.x(), -bounds.y());

        Rect {
          x: hint.x + left,
          y: hint.y + top,
          width: hint.width - (bounds.width() - width as f32),
          height: hint.height - (bounds.height() - height as f32),
        }
      });

      self.with_engine(widget, |engine| {
        engine.allocate((width as f32, height as f32), hint);
      });

      crate::report_size(widget, width, height);
    }
  }

  /// A Skene container that does not act: a plain widget laid out by `SkeneLayout`.
  #[derive(Default)]
  pub struct SkeneBox;

  #[glib::object_subclass]
  impl ObjectSubclass for SkeneBox {
    const NAME: &'static str = "SkeneBox";
    type Type = super::SkeneBox;
    type ParentType = gtk::Widget;

    fn class_init(class: &mut Self::Class) {
      class.set_layout_manager_type::<super::SkeneLayout>();
      class.set_css_name("skenebox");
    }
  }

  impl ObjectImpl for SkeneBox {
    fn dispose(&self) {
      while let Some(child) = self.obj().first_child() {
        child.unparent();
      }
    }
  }

  impl WidgetImpl for SkeneBox {}

  /// A Skene container that acts: GTK's button, any number of children laid out by
  /// `SkeneLayout`.
  #[derive(Default)]
  pub struct SkeneButton;

  #[glib::object_subclass]
  impl ObjectSubclass for SkeneButton {
    const NAME: &'static str = "SkeneButton";
    type Type = super::SkeneButton;
    type ParentType = gtk::Button;

    fn class_init(class: &mut Self::Class) {
      // A box that acts is drawn as its view decorates it, not as the theme's button.
      class.set_css_name("skenebutton");
    }
  }

  impl ObjectImpl for SkeneButton {
    fn constructed(&self) {
      self.parent_constructed();
      self
        .obj()
        .set_layout_manager(Some(super::SkeneLayout::new()));
    }

    fn dispose(&self) {
      while let Some(child) = self.obj().first_child() {
        child.unparent();
      }
    }
  }

  impl WidgetImpl for SkeneButton {}

  impl ButtonImpl for SkeneButton {}

  /// A painted node: its pixels, drawn over its whole box.
  #[derive(Default)]
  pub struct SkenePainted {
    /// The pixels shown, as a texture.
    pub texture: RefCell<Option<gtk::gdk::Texture>>,
    /// The scale the pixels were drawn at.
    pub scale: std::cell::Cell<f64>,
  }

  #[glib::object_subclass]
  impl ObjectSubclass for SkenePainted {
    const NAME: &'static str = "SkenePainted";
    type Type = super::SkenePainted;
    type ParentType = gtk::Widget;

    fn class_init(class: &mut Self::Class) {
      class.set_css_name("skenepainted");
    }
  }

  impl ObjectImpl for SkenePainted {}

  impl WidgetImpl for SkenePainted {
    fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
      self.parent_size_allocate(width, height, baseline);
      crate::report_size(self.obj().upcast_ref(), width, height);
    }

    fn snapshot(&self, snapshot: &gtk::Snapshot) {
      let widget = self.obj();

      if let Some(texture) = self.texture.borrow().as_ref() {
        snapshot.append_texture(
          texture,
          &gtk::graphene::Rect::new(0., 0., widget.width() as f32, widget.height() as f32),
        );
      }
    }
  }
}

glib::wrapper! {
  /// The layout manager every Skene container uses.
  pub struct SkeneLayout(ObjectSubclass<imp::SkeneLayout>) @extends gtk::LayoutManager;
}

glib::wrapper! {
  /// A Skene container that does not act.
  pub struct SkeneBox(ObjectSubclass<imp::SkeneBox>)
    @extends gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

glib::wrapper! {
  /// A Skene container that acts.
  pub struct SkeneButton(ObjectSubclass<imp::SkeneButton>)
    @extends gtk::Button, gtk::Widget,
    @implements gtk::Accessible, gtk::Actionable, gtk::Buildable, gtk::ConstraintTarget;
}

glib::wrapper! {
  /// A painted node's widget.
  pub struct SkenePainted(ObjectSubclass<imp::SkenePainted>)
    @extends gtk::Widget,
    @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl SkenePainted {
  /// Shows `pixels`, drawn at `scale`, over the whole box.
  pub fn show(&self, texture: Option<gtk::gdk::Texture>, scale: f64) {
    let inner = self.imp();

    *inner.texture.borrow_mut() = texture;
    inner.scale.set(scale);
    self.queue_draw();
  }

  /// The pixels shown and the scale they were drawn at.
  #[must_use]
  pub fn shown(&self) -> (Option<gtk::gdk::Texture>, f64) {
    let inner = self.imp();

    (inner.texture.borrow().clone(), inner.scale.get())
  }
}

impl SkeneLayout {
  /// A layout manager running the engine.
  #[must_use]
  pub fn new() -> Self {
    glib::Object::new()
  }
}

impl Default for SkeneLayout {
  fn default() -> Self {
    Self::new()
  }
}
