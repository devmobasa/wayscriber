use crate::config::BoardItemConfig;
use crate::input::DrawingState;
use crate::input::boards::PendingBoardConfigUpdate;

use super::WaylandState;

impl WaylandState {
    pub(in crate::backend::wayland) fn apply_board_config_update(
        &mut self,
        update: PendingBoardConfigUpdate,
    ) {
        apply_board_config_update_to_config(&mut self.config, update);
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save board config: {}", err);
        }
    }

    pub(in crate::backend::wayland) fn board_view_offset(&self) -> (f64, f64) {
        if self.input_state.board_is_transparent() || !self.input_state.boards.pan_enabled() {
            (0.0, 0.0)
        } else {
            let (x, y) = self.input_state.boards.active_frame().view_offset();
            (x as f64, y as f64)
        }
    }

    pub(in crate::backend::wayland) fn canvas_view_origin(&self) -> (f64, f64) {
        let (board_x, board_y) = self.board_view_offset();
        if self.zoom.active {
            (
                board_x + self.zoom.view_offset.0,
                board_y + self.zoom.view_offset.1,
            )
        } else {
            (board_x, board_y)
        }
    }

    pub(in crate::backend::wayland) fn canvas_transform_active(&self) -> bool {
        self.zoom.active
            || (self.input_state.boards.pan_enabled()
                && !self.input_state.board_is_transparent()
                && self.input_state.boards.active_frame().view_offset() != (0, 0))
    }

    pub(in crate::backend::wayland) fn canvas_world_coords(
        &self,
        screen_x: f64,
        screen_y: f64,
    ) -> (i32, i32) {
        let (board_x, board_y) = self.board_view_offset();
        if self.zoom.active {
            let (zoom_x, zoom_y) = self.zoom.screen_to_world(screen_x, screen_y);
            (
                (board_x + zoom_x).round() as i32,
                (board_y + zoom_y).round() as i32,
            )
        } else {
            (
                (board_x + screen_x).round() as i32,
                (board_y + screen_y).round() as i32,
            )
        }
    }

    pub(in crate::backend::wayland) fn can_start_board_pan(&self) -> bool {
        self.input_state.boards.pan_enabled()
            && !self.input_state.board_is_transparent()
            && !self.zoom.active
            && !self.input_state.tour_active
            && !self.input_state.command_palette_open
            && !self.input_state.is_board_picker_open()
            && !self.input_state.is_color_picker_popup_open()
            && !self.input_state.is_context_menu_open()
            && !self.input_state.is_properties_panel_open()
            && !self.input_state.is_radial_menu_open()
            && matches!(self.input_state.state, DrawingState::Idle)
    }

    pub(in crate::backend::wayland) fn start_board_pan(&mut self, screen_x: f64, screen_y: f64) {
        self.data.board_panning = true;
        self.data.board_pan_last_pos = (screen_x, screen_y);
    }

    pub(in crate::backend::wayland) fn stop_board_pan(&mut self) {
        self.data.board_panning = false;
    }

    pub(in crate::backend::wayland) fn board_panning_active(&self) -> bool {
        self.data.board_panning
    }

    pub(in crate::backend::wayland) fn board_pan_key_held(&self) -> bool {
        self.data.board_pan_key_held
    }

    pub(in crate::backend::wayland) fn set_board_pan_key_held(&mut self, held: bool) {
        self.data.board_pan_key_held = held;
    }

    pub(in crate::backend::wayland) fn pan_board_by_screen_delta(
        &mut self,
        dx: f64,
        dy: f64,
    ) -> bool {
        if self.input_state.board_is_transparent() || !self.input_state.boards.pan_enabled() {
            return false;
        }
        let dx = dx.round() as i32;
        let dy = dy.round() as i32;
        if dx == 0 && dy == 0 {
            return false;
        }
        let changed = self
            .input_state
            .boards
            .active_frame_mut()
            .pan_view_by(-dx, -dy);
        if changed {
            self.input_state.dirty_tracker.mark_full();
            self.input_state.needs_redraw = true;
            self.input_state.mark_session_dirty();
        }
        changed
    }

    pub(in crate::backend::wayland) fn update_board_pan_position(
        &mut self,
        screen_x: f64,
        screen_y: f64,
    ) -> (f64, f64) {
        let (last_x, last_y) = self.data.board_pan_last_pos;
        self.data.board_pan_last_pos = (screen_x, screen_y);
        (screen_x - last_x, screen_y - last_y)
    }

    pub(in crate::backend::wayland) fn should_capture_space_for_board_pan(&self) -> bool {
        self.input_state.boards.pan_enabled()
            && !self.input_state.board_is_transparent()
            && !self.zoom.active
            && !self.input_state.tour_active
            && !self.input_state.show_help
            && !self.input_state.command_palette_open
            && !self.input_state.is_board_picker_open()
            && !self.input_state.is_color_picker_popup_open()
            && !self.input_state.is_context_menu_open()
            && !self.input_state.is_properties_panel_open()
            && !self.input_state.is_radial_menu_open()
            && !self.pointer_over_toolbar()
            && self.toolbar_focus_target().is_none()
            && matches!(self.input_state.state, DrawingState::Idle)
    }
}

fn apply_board_config_update_to_config(
    config: &mut crate::config::Config,
    update: PendingBoardConfigUpdate,
) {
    if config
        .boards
        .as_ref()
        .is_none_or(|boards| boards.items.is_empty())
    {
        config.boards = Some(config.resolved_boards());
    }
    let boards = config.boards.as_mut().expect("resolved boards are present");
    let PendingBoardConfigUpdate {
        snapshot,
        structure_changed,
        created_ids,
        deleted_ids: _,
        changed_names,
        changed_appearances,
        changed_pins,
    } = update;

    let apply_changed_fields = |saved: &mut BoardItemConfig, live: &BoardItemConfig| {
        if changed_names.contains(&live.id) {
            saved.name = live.name.clone();
        }
        if changed_appearances.contains(&live.id) {
            saved.background = live.background.clone();
            saved.default_pen_color = live.default_pen_color.clone();
        }
        if changed_pins.contains(&live.id) {
            saved.pinned = live.pinned;
        }
    };

    if structure_changed {
        boards.default_board = snapshot.default_board.clone();
        let mut existing = std::mem::take(&mut boards.items);
        boards.items = snapshot
            .items
            .into_iter()
            .map(|live| {
                if created_ids.contains(&live.id) {
                    return live;
                }
                if let Some(index) = existing.iter().position(|saved| saved.id == live.id) {
                    let mut saved = existing.remove(index);
                    apply_changed_fields(&mut saved, &live);
                    saved
                } else {
                    live
                }
            })
            .collect();
        return;
    }

    for live in snapshot.items {
        let changed = changed_names.contains(&live.id)
            || changed_appearances.contains(&live.id)
            || changed_pins.contains(&live.id);
        if !changed {
            continue;
        }
        if let Some(saved) = boards.items.iter_mut().find(|saved| saved.id == live.id) {
            apply_changed_fields(saved, &live);
        } else {
            boards.items.push(live);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BoardsConfig;
    use crate::input::boards::{BoardConfigChange, PendingBoardConfigUpdate};

    fn board_mut<'a>(
        boards: &'a mut BoardsConfig,
        id: &str,
    ) -> &'a mut crate::config::BoardItemConfig {
        boards
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .unwrap_or_else(|| panic!("missing test board {id}"))
    }

    #[test]
    fn board_reorder_preserves_unrelated_live_item_fields() {
        let mut config = crate::config::Config::default();
        let configured = config.boards.as_mut().expect("configured boards");
        board_mut(configured, "whiteboard").pinned = true;
        board_mut(configured, "whiteboard").name = "Configured whiteboard".to_string();

        let mut live = configured.clone();
        board_mut(&mut live, "whiteboard").pinned = false;
        board_mut(&mut live, "whiteboard").name = "Unrelated live name".to_string();
        let whiteboard = live
            .items
            .iter()
            .position(|item| item.id == "whiteboard")
            .expect("whiteboard index");
        let blackboard = live
            .items
            .iter()
            .position(|item| item.id == "blackboard")
            .expect("blackboard index");
        live.items.swap(whiteboard, blackboard);

        apply_board_config_update_to_config(
            &mut config,
            PendingBoardConfigUpdate::new(live, BoardConfigChange::Structure),
        );

        let saved = config.boards.as_ref().expect("saved boards");
        let saved_whiteboard = saved
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("saved whiteboard");
        assert!(saved_whiteboard.pinned);
        assert_eq!(saved_whiteboard.name, "Configured whiteboard");
        assert!(
            saved
                .items
                .iter()
                .position(|item| item.id == "blackboard")
                .expect("saved blackboard index")
                < saved
                    .items
                    .iter()
                    .position(|item| item.id == "whiteboard")
                    .expect("saved whiteboard index")
        );
    }

    #[test]
    fn board_field_update_changes_only_the_named_field() {
        let mut config = crate::config::Config::default();
        let configured = config.boards.as_mut().expect("configured boards");
        board_mut(configured, "whiteboard").pinned = true;
        let original_background = board_mut(configured, "whiteboard").background.clone();

        let mut live = configured.clone();
        board_mut(&mut live, "whiteboard").name = "Renamed".to_string();
        board_mut(&mut live, "whiteboard").pinned = false;

        apply_board_config_update_to_config(
            &mut config,
            PendingBoardConfigUpdate::new(live, BoardConfigChange::Name("whiteboard".to_string())),
        );

        let saved = config.boards.as_ref().expect("saved boards");
        let saved_whiteboard = saved
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("saved whiteboard");
        assert_eq!(saved_whiteboard.name, "Renamed");
        assert!(saved_whiteboard.pinned);
        assert_eq!(
            saved_whiteboard.background.rgb_for_test(),
            original_background.rgb_for_test()
        );
    }

    #[test]
    fn board_appearance_update_changes_only_the_appearance_fields() {
        let mut config = crate::config::Config::default();
        let configured = config.boards.as_mut().expect("configured boards");
        board_mut(configured, "whiteboard").name = "Configured whiteboard".to_string();
        board_mut(configured, "whiteboard").pinned = true;

        let replacement = configured
            .items
            .iter()
            .find(|item| item.id == "blackboard")
            .expect("blackboard appearance")
            .clone();
        let mut live = configured.clone();
        let live_whiteboard = board_mut(&mut live, "whiteboard");
        live_whiteboard.background = replacement.background.clone();
        live_whiteboard.default_pen_color = replacement.default_pen_color.clone();
        live_whiteboard.name = "Unrelated live name".to_string();
        live_whiteboard.pinned = false;

        apply_board_config_update_to_config(
            &mut config,
            PendingBoardConfigUpdate::new(
                live,
                BoardConfigChange::Appearance("whiteboard".to_string()),
            ),
        );

        let saved = config.boards.as_ref().expect("saved boards");
        let saved_whiteboard = saved
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("saved whiteboard");
        assert_eq!(
            saved_whiteboard.background.rgb_for_test(),
            replacement.background.rgb_for_test()
        );
        assert_eq!(
            saved_whiteboard
                .default_pen_color
                .as_ref()
                .map(crate::config::BoardColorConfig::rgb),
            replacement
                .default_pen_color
                .as_ref()
                .map(crate::config::BoardColorConfig::rgb)
        );
        assert_eq!(saved_whiteboard.name, "Configured whiteboard");
        assert!(saved_whiteboard.pinned);
    }

    #[test]
    fn coalesced_board_pin_and_reorder_apply_both_without_copying_other_fields() {
        let mut config = crate::config::Config::default();
        let configured = config.boards.as_mut().expect("configured boards");
        board_mut(configured, "whiteboard").name = "Configured whiteboard".to_string();

        let mut live = configured.clone();
        board_mut(&mut live, "whiteboard").pinned = true;
        let mut update = PendingBoardConfigUpdate::new(
            live.clone(),
            BoardConfigChange::Pinned("whiteboard".to_string()),
        );

        board_mut(&mut live, "whiteboard").name = "Unrelated live name".to_string();
        let whiteboard = live
            .items
            .iter()
            .position(|item| item.id == "whiteboard")
            .expect("whiteboard index");
        let blackboard = live
            .items
            .iter()
            .position(|item| item.id == "blackboard")
            .expect("blackboard index");
        live.items.swap(whiteboard, blackboard);
        update.merge(live, BoardConfigChange::Structure);

        apply_board_config_update_to_config(&mut config, update);

        let saved = config.boards.as_ref().expect("saved boards");
        let saved_whiteboard = saved
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("saved whiteboard");
        assert!(saved_whiteboard.pinned);
        assert_eq!(saved_whiteboard.name, "Configured whiteboard");
        assert!(
            saved
                .items
                .iter()
                .position(|item| item.id == "blackboard")
                .expect("saved blackboard index")
                < saved
                    .items
                    .iter()
                    .position(|item| item.id == "whiteboard")
                    .expect("saved whiteboard index")
        );
    }

    #[test]
    fn board_structure_update_adds_and_deletes_only_the_requested_entries() {
        let mut config = crate::config::Config::default();
        let configured = config.boards.as_mut().expect("configured boards");
        configured.default_board = "blackboard".to_string();
        board_mut(configured, "whiteboard").pinned = true;

        let mut live = configured.clone();
        live.items.retain(|item| item.id != "blackboard");
        live.default_board = "transparent".to_string();
        let mut created = live
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("whiteboard template")
            .clone();
        created.id = "custom-board".to_string();
        created.name = "Custom board".to_string();
        created.pinned = false;
        live.items.push(created);

        apply_board_config_update_to_config(
            &mut config,
            PendingBoardConfigUpdate::new(live, BoardConfigChange::Structure),
        );

        let saved = config.boards.as_ref().expect("saved boards");
        assert_eq!(saved.default_board, "transparent");
        assert!(saved.items.iter().all(|item| item.id != "blackboard"));
        assert!(
            saved
                .items
                .iter()
                .any(|item| item.id == "custom-board" && item.name == "Custom board")
        );
        assert!(
            saved
                .items
                .iter()
                .find(|item| item.id == "whiteboard")
                .expect("retained whiteboard")
                .pinned
        );
    }

    #[test]
    fn board_structure_update_uses_new_snapshot_for_a_reused_id() {
        let mut config = crate::config::Config::default();
        let configured = config.boards.as_mut().expect("configured boards");
        let mut old_board = configured
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("whiteboard template")
            .clone();
        old_board.id = "board-4".to_string();
        old_board.name = "Old board".to_string();
        old_board.pinned = true;
        configured.items.push(old_board);

        let mut after_delete = configured.clone();
        after_delete.items.retain(|item| item.id != "board-4");
        let mut update = PendingBoardConfigUpdate::new(
            after_delete.clone(),
            BoardConfigChange::IdentityDeleted("board-4".to_string()),
        );

        let mut replacement = after_delete
            .items
            .iter()
            .find(|item| item.id == "blackboard")
            .expect("blackboard template")
            .clone();
        replacement.id = "board-4".to_string();
        replacement.name = "New board".to_string();
        replacement.pinned = false;
        after_delete.items.push(replacement.clone());
        update.merge(
            after_delete,
            BoardConfigChange::IdentitiesCreated(vec!["board-4".to_string()]),
        );

        apply_board_config_update_to_config(&mut config, update);

        let saved = config.boards.as_ref().expect("saved boards");
        let board = saved
            .items
            .iter()
            .find(|item| item.id == "board-4")
            .expect("replacement board");
        assert_eq!(board.name, replacement.name);
        assert_eq!(
            board.background.rgb_for_test(),
            replacement.background.rgb_for_test()
        );
        assert_eq!(board.pinned, replacement.pinned);
    }

    #[test]
    fn board_update_seeds_from_legacy_config_before_applying_exact_field() {
        let mut config = crate::config::Config {
            boards: None,
            ..crate::config::Config::default()
        };
        config.board.enabled = true;
        config.board.whiteboard_color = [0.1, 0.2, 0.3];
        let mut live = config.resolved_boards();
        board_mut(&mut live, "whiteboard").name = "Renamed legacy board".to_string();

        apply_board_config_update_to_config(
            &mut config,
            PendingBoardConfigUpdate::new(live, BoardConfigChange::Name("whiteboard".to_string())),
        );

        let saved = config.boards.as_ref().expect("materialized boards");
        let whiteboard = saved
            .items
            .iter()
            .find(|item| item.id == "whiteboard")
            .expect("legacy whiteboard");
        assert_eq!(whiteboard.name, "Renamed legacy board");
        assert_eq!(whiteboard.background.rgb_for_test(), Some([0.1, 0.2, 0.3]));
    }

    trait BoardBackgroundTestExt {
        fn rgb_for_test(&self) -> Option<[f64; 3]>;
    }

    impl BoardBackgroundTestExt for crate::config::BoardBackgroundConfig {
        fn rgb_for_test(&self) -> Option<[f64; 3]> {
            match self {
                crate::config::BoardBackgroundConfig::Transparent(_) => None,
                crate::config::BoardBackgroundConfig::Color(color) => Some(color.rgb()),
            }
        }
    }
}
