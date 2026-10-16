//! The client-side chrome's geometry: margins, hit testing, buttons.

use skene_gui_linux::chrome::{Button, Chrome, Edge, HEADER, Hit, SHADOW};

fn csd() -> Chrome {
  let mut chrome = Chrome::default();

  chrome.csd = true;

  chrome
}

#[test]
fn a_floating_window_has_shadow_margins_and_a_maximized_one_none() {
  let mut chrome = csd();

  assert_eq!(chrome.margins(), [SHADOW; 4]);
  assert_eq!(chrome.surface_size(400, 300), (448, 348));
  chrome.states.maximized = true;
  assert_eq!(chrome.margins(), [0.; 4]);
  assert_eq!(chrome.surface_size(400, 300), (400, 300));
}

#[test]
fn a_tiled_edge_has_no_margin_and_no_resize_band() {
  let mut chrome = csd();

  chrome.states.tiled = [true, false, false, false];

  assert_eq!(chrome.margins(), [0., SHADOW, SHADOW, SHADOW]);
  // At the tiled left edge a press reaches the content rather than resizing.
  assert_eq!(
    chrome.hit(1., SHADOW + 150., 400, 300),
    Hit::Content(1., 150.)
  );
}

#[test]
fn presses_near_the_edges_resize_and_on_the_bar_move_or_press_a_button() {
  let chrome = csd();
  let (ox, oy) = chrome.origin();

  assert_eq!(
    chrome.hit(ox + 400. + 6., oy + 300. + 6., 400, 300),
    Hit::Edge(Edge::BottomRight)
  );
  assert_eq!(
    chrome.hit(ox - 8., oy + 150., 400, 300),
    Hit::Edge(Edge::Left)
  );
  assert_eq!(
    chrome.hit(ox + 100., oy + HEADER / 2., 400, 300),
    Hit::Title
  );

  let (button, rect) = chrome.buttons(400)[0];

  assert_eq!(button, Button::Close);
  assert_eq!(
    chrome.hit(ox + rect.x + 4., oy + rect.y + 4., 400, 300),
    Hit::Button(Button::Close)
  );
  assert_eq!(
    chrome.hit(ox + 100., oy + HEADER + 10., 400, 300),
    Hit::Content(100., HEADER + 10.)
  );
  assert_eq!(chrome.hit(2., 2., 400, 300), Hit::Outside);
}
