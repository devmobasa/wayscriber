use super::base::{InputState, UiToastKind};
use super::session_preflight_exact::{
    ClonePreflightAction, duplicate_board_id_for_preflight, exact_visible_save_allows,
};
use crate::draw::{BoardPages, Frame, Shape};
use crate::input::boards::BoardState;
use crate::session::{CompressionMode, DEFAULT_MAX_EXPANDED_SESSION_BYTES, SessionOptions};

const SESSION_RAW_OVERHEAD_BYTES: u64 = 256;
const BOARD_RAW_OVERHEAD_BYTES: u64 = 96;
const PAGE_RAW_OVERHEAD_BYTES: u64 = 64;
const IMAGE_SHAPE_RAW_OVERHEAD_BYTES: u64 = 1024;
const NON_IMAGE_SHAPE_RAW_OVERHEAD_BYTES: u64 = 2048;
const TOOL_STATE_RAW_OVERHEAD_BYTES: u64 = 512;
const COMPRESSED_IMAGE_OVERHEAD_BYTES: u64 = 128;
const COMPRESSED_IMAGE_PERCENT_OVERHEAD: u64 = 105;
const POINT_JSON_BYTES: u64 = 128;
const PRESSURE_POINT_JSON_BYTES: u64 = 192;
const VISIBLE_HISTORY_LIMIT: usize = 0;

#[derive(Debug, Clone, Copy, Default)]
struct CloneStorageEstimate {
    raw_bytes: u64,
    image_encoded_raw_bytes: u64,
    image_original_bytes: u64,
    image_count: u64,
    non_image_shape_count: u64,
}

impl CloneStorageEstimate {
    fn add_raw(&mut self, bytes: u64) {
        self.raw_bytes = self.raw_bytes.saturating_add(bytes);
    }

    fn add_image(&mut self, bytes: usize, mime_type: &str) {
        let image_bytes = usize_to_u64(bytes);
        let encoded_bytes = base64_encoded_len(image_bytes);
        let raw_bytes = IMAGE_SHAPE_RAW_OVERHEAD_BYTES
            .saturating_add(escaped_json_string_len(mime_type))
            .saturating_add(encoded_bytes);
        self.add_raw(raw_bytes);
        self.image_encoded_raw_bytes = self.image_encoded_raw_bytes.saturating_add(encoded_bytes);
        self.image_original_bytes = self.image_original_bytes.saturating_add(image_bytes);
        self.image_count = self.image_count.saturating_add(1);
    }

    fn add_non_image_shape(&mut self, bytes: u64) {
        self.add_raw(bytes);
        self.non_image_shape_count = self.non_image_shape_count.saturating_add(1);
    }

    fn saturating_add(self, other: Self) -> Self {
        Self {
            raw_bytes: self.raw_bytes.saturating_add(other.raw_bytes),
            image_encoded_raw_bytes: self
                .image_encoded_raw_bytes
                .saturating_add(other.image_encoded_raw_bytes),
            image_original_bytes: self
                .image_original_bytes
                .saturating_add(other.image_original_bytes),
            image_count: self.image_count.saturating_add(other.image_count),
            non_image_shape_count: self
                .non_image_shape_count
                .saturating_add(other.non_image_shape_count),
        }
    }

    fn estimated_written_bytes(&self, options: &SessionOptions) -> u64 {
        match options.compression {
            CompressionMode::Off => self.raw_bytes,
            CompressionMode::Auto if self.raw_bytes < options.auto_compress_threshold_bytes => {
                self.raw_bytes
            }
            CompressionMode::Auto | CompressionMode::On => {
                let non_image_raw = self.raw_bytes.saturating_sub(self.image_encoded_raw_bytes);
                self.image_original_bytes
                    .saturating_mul(COMPRESSED_IMAGE_PERCENT_OVERHEAD)
                    .div_ceil(100)
                    .saturating_add(
                        self.image_count
                            .saturating_mul(COMPRESSED_IMAGE_OVERHEAD_BYTES),
                    )
                    .saturating_add(non_image_raw)
            }
        }
    }

    fn would_be_compressed(&self, options: &SessionOptions) -> bool {
        match options.compression {
            CompressionMode::Off => false,
            CompressionMode::On => true,
            CompressionMode::Auto => self.raw_bytes >= options.auto_compress_threshold_bytes,
        }
    }

    fn is_near_limit(&self, options: &SessionOptions) -> bool {
        let written = self.estimated_written_bytes(options);
        if written == 0 {
            return false;
        }
        let threshold = ((options.max_file_size_bytes as u128) * 90).div_ceil(100);
        (written as u128) >= threshold
    }
}

impl InputState {
    #[allow(dead_code)]
    pub(crate) fn set_session_preflight_options(&mut self, options: Option<SessionOptions>) {
        self.session_preflight_options = options;
    }
    pub(crate) fn session_allows_page_duplicate(
        &mut self,
        board_index: usize,
        page_index: usize,
    ) -> bool {
        let Some(options) = self.session_preflight_options.as_ref() else {
            return true;
        };
        if !session_persistence_enabled(options) {
            return true;
        }
        let Some(board) = self.boards.board_states().get(board_index) else {
            return true;
        };
        if !board_should_persist_for_session(board, options) {
            return true;
        }
        let Some(page) = board.pages.pages().get(page_index) else {
            return true;
        };
        let mut added = estimate_frame_page_storage(page);
        if !board_pages_have_persistable_data(&board.pages, VISIBLE_HISTORY_LIMIT) {
            added = added.saturating_add(estimate_board_shell_storage_for_active(
                &board.spec.id,
                page_index.saturating_add(1),
            ));
            added = added.saturating_add(estimate_pages_storage(&board.pages));
        }
        self.session_allows_clone_heavy_storage(
            added,
            ClonePreflightAction::PageDuplicate {
                board_index,
                page_index,
            },
            "Page",
            "duplicate",
        )
    }
    pub(crate) fn session_allows_page_copy_between_boards(
        &mut self,
        source_board_index: usize,
        page_index: usize,
        target_board_index: usize,
    ) -> bool {
        if source_board_index == target_board_index {
            return true;
        }
        let Some(options) = self.session_preflight_options.as_ref() else {
            return true;
        };
        if !session_persistence_enabled(options) {
            return true;
        }
        let Some(source_board) = self.boards.board_states().get(source_board_index) else {
            return true;
        };
        let Some(target_board) = self.boards.board_states().get(target_board_index) else {
            return true;
        };
        if !board_should_persist_for_session(target_board, options) {
            return true;
        }
        let Some(page) = source_board.pages.pages().get(page_index) else {
            return true;
        };

        let mut added = estimate_frame_page_storage(page);
        if !board_pages_have_persistable_data(&target_board.pages, VISIBLE_HISTORY_LIMIT) {
            added = added.saturating_add(estimate_board_shell_storage(
                &target_board.spec.id,
                &target_board.pages,
            ));
            added = added.saturating_add(estimate_pages_storage(&target_board.pages));
        }
        self.session_allows_clone_heavy_storage(
            added,
            ClonePreflightAction::PageCopy {
                source_board_index,
                page_index,
                target_board_index,
            },
            "Page",
            "copy",
        )
    }
    pub(crate) fn session_allows_board_duplicate(&mut self) -> bool {
        let Some(options) = self.session_preflight_options.as_ref() else {
            return true;
        };
        if !session_persistence_enabled(options) {
            return true;
        }
        let source_id = self.boards.active_board().spec.id.clone();
        if !board_should_persist_for_session(self.boards.active_board(), options) {
            return true;
        }
        if !board_pages_have_persistable_data(
            &self.boards.active_board().pages,
            VISIBLE_HISTORY_LIMIT,
        ) {
            return true;
        }
        let duplicate_id = duplicate_board_id_for_preflight(self, &source_id);
        let added = estimate_board_storage(&duplicate_id, self.boards.active_board());
        self.session_allows_clone_heavy_storage(
            added,
            ClonePreflightAction::BoardDuplicate,
            "Board",
            "duplicate",
        )
    }

    fn session_allows_clone_heavy_storage(
        &mut self,
        added: CloneStorageEstimate,
        action: ClonePreflightAction,
        action_label: &str,
        action_name: &str,
    ) -> bool {
        let Some(options) = self.session_preflight_options.clone() else {
            return true;
        };
        let current = estimate_visible_session_storage(self, &options);
        let projected = current.saturating_add(added);
        let projected_written = projected.estimated_written_bytes(&options);
        let near_limit = projected.is_near_limit(&options);

        if projected.image_count == 0
            || near_limit
            || (projected.non_image_shape_count > 0
                && projected_written > options.max_file_size_bytes)
        {
            match exact_visible_save_allows(self, &options, action) {
                Some(true) => return true,
                Some(false) => {
                    return self.block_clone_heavy_action(
                        action_label,
                        action_name,
                        "session would exceed save limits.".to_string(),
                    );
                }
                None => {
                    return self.block_clone_heavy_action(
                        action_label,
                        action_name,
                        "session size check failed.".to_string(),
                    );
                }
            }
        }

        if projected_written > options.max_file_size_bytes {
            let limit = format_session_limit(options.max_file_size_bytes);
            log::warn!(
                "Blocking {} {} because estimated visible image-heavy session payload would exceed configured cap: projected_written={} current_written={} added_written={} projected_raw={} images={} image_bytes={} max={}",
                action_label.to_lowercase(),
                action_name,
                projected_written,
                current.estimated_written_bytes(&options),
                added.estimated_written_bytes(&options),
                projected.raw_bytes,
                projected.image_count,
                projected.image_original_bytes,
                options.max_file_size_bytes
            );
            return self.block_clone_heavy_action(
                action_label,
                action_name,
                format!(
                    "session would exceed {limit}. Remove images or raise session.max_file_size_mb."
                ),
            );
        }

        if exceeds_expanded_safety_limit(&projected, &options, DEFAULT_MAX_EXPANDED_SESSION_BYTES) {
            let limit = format_session_limit(DEFAULT_MAX_EXPANDED_SESSION_BYTES);
            log::warn!(
                "Blocking {} {} because estimated visible image-heavy session payload would exceed expanded safety cap: projected_raw={} current_raw={} added_raw={} projected_written={} compression={:?} images={} image_bytes={} max_expanded={}",
                action_label.to_lowercase(),
                action_name,
                projected.raw_bytes,
                current.raw_bytes,
                added.raw_bytes,
                projected_written,
                options.compression,
                projected.image_count,
                projected.image_original_bytes,
                DEFAULT_MAX_EXPANDED_SESSION_BYTES
            );
            return self.block_clone_heavy_action(
                action_label,
                action_name,
                format!("session would exceed the {limit} safety limit. Remove images or reduce duplicated content."),
            );
        }
        true
    }

    fn block_clone_heavy_action(
        &mut self,
        action_label: &str,
        action_name: &str,
        reason: String,
    ) -> bool {
        self.set_ui_toast(
            UiToastKind::Warning,
            format!("{action_label} {action_name} blocked; {reason}"),
        );
        self.trigger_blocked_feedback();
        false
    }
}

fn session_persistence_enabled(options: &SessionOptions) -> bool {
    options.any_enabled() || options.restore_tool_state || options.persist_history
}

fn exceeds_expanded_safety_limit(
    estimate: &CloneStorageEstimate,
    options: &SessionOptions,
    max_expanded_size: u64,
) -> bool {
    estimate.would_be_compressed(options) && estimate.raw_bytes > max_expanded_size
}

fn estimate_visible_session_storage(
    input: &InputState,
    options: &SessionOptions,
) -> CloneStorageEstimate {
    let mut estimate = CloneStorageEstimate::default();
    estimate.add_raw(SESSION_RAW_OVERHEAD_BYTES);
    if options.restore_tool_state {
        estimate.add_raw(TOOL_STATE_RAW_OVERHEAD_BYTES);
    }

    for board in input.boards.board_states() {
        if !board_should_persist_for_session(board, options)
            || !board_pages_have_persistable_data(&board.pages, VISIBLE_HISTORY_LIMIT)
        {
            continue;
        }
        estimate = estimate.saturating_add(estimate_board_storage(&board.spec.id, board));
    }

    estimate
}

fn estimate_board_storage(board_id: &str, board: &BoardState) -> CloneStorageEstimate {
    let mut estimate = estimate_board_shell_storage(board_id, &board.pages);
    estimate = estimate.saturating_add(estimate_pages_storage(&board.pages));
    estimate
}

fn estimate_board_shell_storage(board_id: &str, pages: &BoardPages) -> CloneStorageEstimate {
    estimate_board_shell_storage_for_active(board_id, pages.active_index())
}

fn estimate_board_shell_storage_for_active(
    board_id: &str,
    active_index: usize,
) -> CloneStorageEstimate {
    let mut estimate = CloneStorageEstimate::default();
    estimate.add_raw(
        BOARD_RAW_OVERHEAD_BYTES
            .saturating_add(escaped_json_string_len(board_id))
            .saturating_add(usize_to_u64(active_index)),
    );
    estimate
}

fn estimate_pages_storage(pages: &BoardPages) -> CloneStorageEstimate {
    let mut estimate = CloneStorageEstimate::default();
    for page in pages.pages() {
        estimate = estimate.saturating_add(estimate_frame_page_storage(page));
    }
    estimate
}

fn estimate_frame_page_storage(page: &Frame) -> CloneStorageEstimate {
    let mut estimate = CloneStorageEstimate::default();
    estimate.add_raw(PAGE_RAW_OVERHEAD_BYTES);
    if let Some(name) = page.page_name() {
        estimate.add_raw(escaped_json_string_len(name));
    }
    let offset = page.view_offset();
    if offset != (0, 0) {
        estimate.add_raw(32);
    }
    for shape in page.shapes.iter().map(|drawn| &drawn.shape) {
        estimate = estimate.saturating_add(estimate_shape_storage(shape));
    }
    estimate
}

fn estimate_shape_storage(shape: &Shape) -> CloneStorageEstimate {
    let mut estimate = CloneStorageEstimate::default();
    match shape {
        Shape::Freehand { points, .. }
        | Shape::MarkerStroke { points, .. }
        | Shape::EraserStroke { points, .. }
        | Shape::Polygon { points, .. } => {
            estimate.add_non_image_shape(
                NON_IMAGE_SHAPE_RAW_OVERHEAD_BYTES
                    .saturating_add(usize_to_u64(points.len()).saturating_mul(POINT_JSON_BYTES)),
            );
        }
        Shape::FreehandPressure { points, .. } => {
            estimate.add_non_image_shape(NON_IMAGE_SHAPE_RAW_OVERHEAD_BYTES.saturating_add(
                usize_to_u64(points.len()).saturating_mul(PRESSURE_POINT_JSON_BYTES),
            ));
        }
        Shape::Image { data, .. } => estimate.add_image(data.bytes.len(), &data.mime_type),
        Shape::Text {
            text,
            font_descriptor,
            ..
        }
        | Shape::StickyNote {
            text,
            font_descriptor,
            ..
        } => {
            estimate.add_non_image_shape(
                NON_IMAGE_SHAPE_RAW_OVERHEAD_BYTES
                    .saturating_add(escaped_json_string_len(text))
                    .saturating_add(escaped_json_string_len(&font_descriptor.family))
                    .saturating_add(escaped_json_string_len(&font_descriptor.weight))
                    .saturating_add(escaped_json_string_len(&font_descriptor.style)),
            );
        }
        _ => estimate.add_non_image_shape(NON_IMAGE_SHAPE_RAW_OVERHEAD_BYTES),
    }
    estimate
}

pub(super) fn board_should_persist_for_session(
    board: &BoardState,
    options: &SessionOptions,
) -> bool {
    if board.spec.background.is_transparent() {
        options.persist_transparent
    } else {
        (options.persist_whiteboard || options.persist_blackboard) && board.spec.persist
    }
}

fn board_pages_have_persistable_data(pages: &BoardPages, history_limit: usize) -> bool {
    pages.page_count() > 1
        || pages.active_index() > 0
        || pages
            .pages()
            .iter()
            .any(|page| page.has_persistable_data_after_history_limit(history_limit))
}

fn base64_encoded_len(bytes: u64) -> u64 {
    bytes.div_ceil(3).saturating_mul(4)
}

fn escaped_json_string_len(value: &str) -> u64 {
    // Include quotes and conservatively account for JSON escaping.
    value
        .chars()
        .fold(2u64, |len, ch| len.saturating_add(json_char_len(ch)))
}

fn json_char_len(ch: char) -> u64 {
    match ch {
        '"' | '\\' => 2,
        '\u{08}' | '\u{0C}' | '\n' | '\r' | '\t' => 2,
        ch if ch <= '\u{1F}' => 6,
        ch => ch.len_utf8() as u64,
    }
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn format_session_limit(bytes: u64) -> String {
    let mib = bytes as f64 / 1024.0 / 1024.0;
    if mib >= 10.0 {
        format!("{mib:.0} MiB")
    } else {
        format!("{mib:.1} MiB")
    }
}

#[cfg(test)]
#[path = "session_preflight_tests.rs"]
mod session_preflight_tests;
