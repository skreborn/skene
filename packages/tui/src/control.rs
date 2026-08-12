//! The surface every terminal control shares: styling, conditions, and focus.

use std::rc::Rc;

use skene_core::NodeId;

use crate::{FocusHandle, props::Condition, with_state};

/// The focus state behind a control's shared builder surface: the target of
/// [`HasFocus`](crate::HasFocus).
pub struct FocusCore {
  pub(crate) autofocus: bool,
  pub(crate) focus_handle: Option<FocusHandle>,
  pub(crate) anchor: Option<crate::Anchor>,
  pub(crate) focus_handler: Option<Rc<dyn Fn()>>,
  pub(crate) blur_handler: Option<Rc<dyn Fn()>>,
  pub(crate) key_down_handler: Option<Rc<skene_core::KeyHandler>>,
}

/// The styling state behind a control's shared builder surface: the target of
/// [`HasStyle`](crate::HasStyle); `S` is the style vocabulary the control accepts.
pub struct StyleCore<S> {
  pub(crate) style: S,
  pub(crate) variants: Vec<(Condition, S)>,
}

/// The state behind a control's shared builder surface, embedded by every control;
/// `S` is the style vocabulary the control accepts (a small control's
/// [`ControlStyle`](crate::props::ControlStyle), a container-owning one's
/// [`BoxStyle`](crate::props::BoxStyle)).
pub(crate) struct ControlBase<S> {
  pub(crate) style: StyleCore<S>,
  pub(crate) focus: FocusCore,
}

impl<S: Default> ControlBase<S> {
  pub(crate) fn new() -> Self {
    Self {
      style: StyleCore {
        style: S::default(),
        variants: Vec::new(),
      },
      focus: FocusCore {
        autofocus: false,
        focus_handle: None,
        anchor: None,
        focus_handler: None,
        blur_handler: None,
        key_down_handler: None,
      },
    }
  }

  /// Registers the built control: joins the focus order, wires the base's handlers,
  /// and binds its handles.
  pub(crate) fn register(self, node: NodeId) {
    let focus = self.focus;

    with_state(|state| {
      state.controls.borrow_mut().insert(node);

      if let Some(action) = focus.focus_handler {
        state
          .focus
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = focus.blur_handler {
        state
          .blur
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if let Some(action) = focus.key_down_handler {
        state
          .key_down
          .borrow_mut()
          .entry(node)
          .or_default()
          .push(action);
      }

      if focus.autofocus {
        state.focused.set(Some(node));
        state.dirty.set(true);
      }
    });

    if let Some(handle) = &focus.focus_handle {
      handle.assign(node);
    }

    if let Some(anchor) = &focus.anchor {
      anchor.assign(node);
    }
  }
}
