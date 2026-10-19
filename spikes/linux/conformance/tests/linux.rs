//! The conformance suite on Linux: each scenario a `#[skene_gui::test]` of its own, run through
//! the Linux driver (AT-SPI reads, real input) in the session the display runner gives the test
//! (`SKENE_TEST_SESSION`: X11 by default, headless mutter, headless sway).

use skene_gui::testing::Drawn;

/// A test per scenario, and a check that every scenario has one.
macro_rules! scenarios {
  ($($name:ident),* $(,)?) => {
    $(
      #[skene_gui::test]
      fn $name() {
        skene_conformance::run(&mut Drawn::new(), stringify!($name));
      }
    )*

    #[test]
    fn every_scenario_has_a_test() {
      let tested = [$(stringify!($name)),*];
      let scenarios: Vec<&str> = skene_conformance::scenarios::<Drawn>()
        .into_iter()
        .map(|(name, _)| name)
        .collect();

      assert_eq!(scenarios, tested);
    }
  };
}

scenarios!(
  a_toggle_takes_a_flip_its_handler_accepts,
  a_toggle_refuses_a_flip_its_handler_ignores,
  a_text_field_takes_what_is_typed,
  a_text_field_shows_what_its_handler_makes_of_an_edit,
  a_text_field_refuses_a_character_its_handler_refuses,
  a_composition_commits_into_the_truth,
  a_slider_steps_and_its_handler_clamps,
  a_picker_chooses_from_its_list,
  a_control_owns_its_press_inside_a_box_that_hears_presses,
  tab_moves_focus_in_reading_order,
  tab_follows_the_scene_not_the_geometry,
  an_activatable_row_holds_controls_that_own_their_presses,
  a_shortcut_activates_its_button,
  the_dark_scheme_reinks_without_rebuilding,
  a_width_condition_follows_the_window,
  a_library_view_is_controlled,
  a_painted_view_is_its_description,
  a_painted_view_draws_in_its_first_frame,
  a_button_acts_on_enter_at_once_and_on_space_at_its_release,
  a_checkbox_face_follows_its_control,
  a_switch_thumb_moves_by_paint_alone,
  a_focus_ring_is_a_style_a_button_can_change,
  a_disabled_control_takes_no_focus_and_dims,
  a_library_restyles_a_built_in_from_its_palette,
  a_theme_redefines_the_system_accent,
  a_picker_opens_real_options_a_reader_focuses,
  a_picker_is_as_wide_as_its_widest_option,
  a_caption_beside_a_field_focuses_it,
  a_slider_drag_moves_its_fill_and_thumb,
  an_editor_scrolls_itself,
  a_third_party_control_is_first_class,
  what_is_drawn_later_covers_a_text_drawn_before,
  a_gradient_lies_over_its_box_and_follows_its_tokens,
  a_condition_flipping_runs_its_transition_on_the_frame_clock,
  a_redefined_token_runs_its_transition,
  a_derived_write_runs_its_transition_outside_the_graph,
  a_text_color_transition_repaints_its_text,
  reduced_motion_cuts_a_transition,
  a_keyframe_animation_plays_once_from_when_its_box_shows,
  nothing_in_motion_paints_no_frame,
  a_caption_hovers_its_control_and_a_box_hovers_alone,
  a_window_opens_with_its_autofocus_control_focused_or_none,
  a_popup_focuses_its_autofocus_control_and_closing_returns_focus,
  a_press_on_content_overflowing_its_control_reaches_the_control,
  a_picker_holding_a_value_no_option_carries_shows_none,
  a_derived_list_reorders_while_open_focus_staying_on_its_option,
  a_list_box_is_one_tab_stop_choosing_one_or_several,
  every_page_passes_the_audit,
);
