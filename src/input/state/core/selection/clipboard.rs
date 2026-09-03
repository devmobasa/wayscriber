//! Clipboard state for copied selections and image-save fallback.

use crate::draw::Shape;
use crate::input::state::core::base::{PendingClipboardFallback, SelectionPublishState};
use std::time::{SystemTime, UNIX_EPOCH};

/// Local selection clipboard identity, publication, paste requests, and capture fallback.
#[derive(Debug, Clone)]
pub(crate) struct SelectionClipboard {
    pub(in crate::input::state) shapes: Option<Vec<Shape>>,
    pub(in crate::input::state) generation: u64,
    pub(in crate::input::state) publish_state: SelectionPublishState,
    pub(in crate::input::state) app_instance_id: String,
    pub(in crate::input::state) paste_request_counter: u64,
    pub(in crate::input::state) active_paste_request_id: Option<u64>,
    pub(in crate::input::state) pending_image_fallback: Option<PendingClipboardFallback>,
}

impl Default for SelectionClipboard {
    fn default() -> Self {
        Self {
            shapes: None,
            generation: 0,
            publish_state: SelectionPublishState::NotAttempted,
            app_instance_id: new_clipboard_app_instance_id(),
            paste_request_counter: 0,
            active_paste_request_id: None,
            pending_image_fallback: None,
        }
    }
}

fn new_clipboard_app_instance_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{}-{millis}", std::process::id())
}
