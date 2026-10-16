//! A closed face that opens a list of options: a picker.

use crate::{
  behavior::{Behavior, Collection, Orientation, Response},
  seam::{
    access::{AccessFacts, Action, Role},
    report::KeyEvent,
  },
};

/// What a popup reports.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PopupAct {
  /// Open the list, its chosen option highlighted.
  Open,
  /// Close the list, choosing nothing.
  Close,
  /// Highlight an option of the open list.
  Highlight(usize),
  /// Choose an option, the list staying as it is (a closed face's arrows).
  Choose(usize),
  /// Choose an option and close the list.
  ChooseAndClose(usize),
}

/// A popup's behavior: its closed face and its open list.
///
/// The closed face opens on `Space`, `Alt` with an arrow down or up, and `F4`; its arrows,
/// `Home` and `End` choose without opening, as a Chromium `<select>` on Linux and Windows
/// does. The open list moves its highlight with the arrows, the page keys, `Home` and `End`;
/// `Enter` and `Space` choose the highlighted option and close, as `Tab` does while leaving
/// the key to move focus; `Escape` closes, choosing nothing. A click on the face opens or
/// closes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub struct Popup {
  /// How many options.
  pub count: usize,
  /// The chosen option.
  pub chosen: Option<usize>,
  /// Whether the list is open.
  pub open: bool,
  /// The highlighted option of the open list.
  pub highlighted: Option<usize>,
  /// The options a page key moves by in the open list.
  pub page: usize,
}

impl Popup {
  /// A popup of `count` options with `chosen`, closed.
  #[inline(always)]
  pub const fn new(count: usize, chosen: Option<usize>) -> Self {
    Self {
      count,
      chosen,
      open: false,
      highlighted: None,
      page: 10,
    }
  }

  /// This popup, open with `highlighted` where `open`.
  #[inline(always)]
  pub const fn with_open(mut self, open: bool, highlighted: Option<usize>) -> Self {
    self.open = open;
    self.highlighted = highlighted;

    self
  }

  fn list(&self) -> Collection<'static> {
    Collection::new(
      self.count,
      if self.open {
        self.highlighted
      } else {
        self.chosen
      },
      Orientation::Vertical,
    )
    .with_page(self.page)
  }
}

impl Behavior for Popup {
  type Act = PopupAct;

  fn role(&self) -> Role {
    Role::ComboBox
  }

  fn key(&self, event: &KeyEvent) -> Response<PopupAct> {
    let modifiers = event.modifiers;

    if !event.down || modifiers.control || modifiers.meta {
      return Response::IGNORED;
    }

    let key = event.key.as_str();
    let toggles = (modifiers.alt && matches!(key, "ArrowDown" | "ArrowUp")) || key == "F4";

    if modifiers.alt && !toggles {
      return Response::IGNORED;
    }

    if !self.open {
      return match key {
        " " => Response::act(PopupAct::Open),
        _ if toggles => Response::act(PopupAct::Open),
        "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "PageUp"
        | "PageDown" => {
          let list = Collection::new(self.count, self.chosen, Orientation::Both).with_page(1);

          list.key(event).map(PopupAct::Choose)
        }
        _ => Response::IGNORED,
      };
    }

    match key {
      "Escape" => Response::act(PopupAct::Close),
      "Enter" | " " => Response::act(
        self
          .highlighted
          .map_or(PopupAct::Close, PopupAct::ChooseAndClose),
      ),
      "Tab" => Response {
        consumed: false,
        pressed: None,
        act: Some(
          self
            .highlighted
            .map_or(PopupAct::Close, PopupAct::ChooseAndClose),
        ),
      },
      _ if toggles => Response::act(
        self
          .highlighted
          .map_or(PopupAct::Close, PopupAct::ChooseAndClose),
      ),
      _ => {
        let response = self.list().key(event).map(PopupAct::Highlight);

        if response.consumed {
          response
        } else if matches!(key, "ArrowLeft" | "ArrowRight") {
          Response::CONSUMED
        } else {
          Response::IGNORED
        }
      }
    }
  }

  fn click(&self) -> Option<PopupAct> {
    Some(if self.open {
      PopupAct::Close
    } else {
      PopupAct::Open
    })
  }

  fn action(&self, action: Action) -> Option<PopupAct> {
    match action {
      Action::Activate => self.click(),
      Action::Expand if !self.open => Some(PopupAct::Open),
      Action::Collapse if self.open => Some(PopupAct::Close),
      _ => None,
    }
  }

  fn actions(&self) -> Vec<Action> {
    vec![Action::Activate]
  }

  fn facts(&self) -> AccessFacts {
    AccessFacts {
      expanded: Some(self.open),
      has_popup: true,
      ..AccessFacts::default()
    }
  }
}
