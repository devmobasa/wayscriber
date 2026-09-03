//! Clipboard state for copied selections and image-save fallback.

use crate::draw::Shape;
use crate::input::state::core::base::{
    ClipboardFingerprint, ClipboardPasteRequest, PasteAnchor, PendingClipboardFallback,
    PendingSelectionClipboardPublish, SelectionPublishState, WayscriberClipboardSelection,
};
use crate::util::Rect;
use std::time::{SystemTime, UNIX_EPOCH};

const PRIVATE_CLIPBOARD_SCHEMA_VERSION: u32 = 1;

/// Local selection clipboard identity, publication, paste requests, and capture fallback.
#[derive(Debug, Clone)]
pub(crate) struct SelectionClipboard {
    shapes: Option<Vec<Shape>>,
    generation: u64,
    publish_state: SelectionPublishState,
    app_instance_id: String,
    paste_request_counter: u64,
    active_paste_request_id: Option<u64>,
    pending_image_fallback: Option<PendingClipboardFallback>,
}

/// Immutable selection clipboard state used while the backend plans a transfer.
#[derive(Debug, Clone)]
pub(crate) struct LocalSelectionContext {
    shapes: Option<Vec<Shape>>,
    generation: u64,
    publish_state: SelectionPublishState,
    app_instance_id: String,
    active_paste_request_id: Option<u64>,
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

impl SelectionClipboard {
    pub(crate) fn copy_shapes(
        &mut self,
        shapes: Vec<Shape>,
    ) -> Option<PendingSelectionClipboardPublish> {
        if shapes.is_empty() {
            return None;
        }

        self.generation = self.generation.wrapping_add(1);
        self.publish_state = SelectionPublishState::NotAttempted;
        self.shapes = Some(shapes.clone());
        let payload = WayscriberClipboardSelection {
            schema_version: PRIVATE_CLIPBOARD_SCHEMA_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            app_instance_id: self.app_instance_id.clone(),
            copy_generation: self.generation,
            shapes,
        };
        serde_json::to_string(&payload)
            .ok()
            .map(|payload_json| PendingSelectionClipboardPublish {
                generation: payload.copy_generation,
                payload_json,
            })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.shapes
            .as_ref()
            .is_none_or(|clipboard| clipboard.is_empty())
    }

    pub(crate) fn shapes(&self) -> Option<Vec<Shape>> {
        self.shapes.clone().filter(|shapes| !shapes.is_empty())
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn snapshot(&self) -> LocalSelectionContext {
        LocalSelectionContext {
            shapes: self.shapes.clone(),
            generation: self.generation,
            publish_state: self.publish_state.clone(),
            app_instance_id: self.app_instance_id.clone(),
            active_paste_request_id: self.active_paste_request_id,
        }
    }

    pub(crate) fn complete_publish(
        &mut self,
        generation: u64,
        fingerprint_at_failure: Option<ClipboardFingerprint>,
        succeeded: bool,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.publish_state = if succeeded {
            SelectionPublishState::Published { generation }
        } else {
            SelectionPublishState::Failed {
                generation,
                clipboard_fingerprint_at_failure: fingerprint_at_failure,
            }
        };
        true
    }

    pub(crate) fn begin_paste_request(
        &mut self,
        target_board_id: String,
        target_page_index: usize,
        target_page_generation: u64,
        anchor: PasteAnchor,
        visible_canvas_rect: Rect,
        screen_size: (u32, u32),
    ) -> ClipboardPasteRequest {
        self.paste_request_counter = self.paste_request_counter.wrapping_add(1);
        let request = ClipboardPasteRequest {
            id: self.paste_request_counter,
            target_board_id,
            target_page_index,
            target_page_generation,
            anchor,
            visible_canvas_rect,
            screen_size,
            selection_clipboard_generation_at_request: self.generation,
            local_selection_fallback_generation: self.snapshot().fallback_generation(),
        };
        self.active_paste_request_id = Some(request.id);
        request
    }

    pub(crate) fn finish_paste_request(&mut self, id: u64) {
        if self.active_paste_request_id == Some(id) {
            self.active_paste_request_id = None;
        }
    }

    pub(crate) fn failed_after_fingerprint_probe(
        &mut self,
        request_generation: Option<u64>,
        current: Option<ClipboardFingerprint>,
    ) -> Option<Vec<Shape>> {
        let request_generation = request_generation?;
        let SelectionPublishState::Failed {
            generation,
            clipboard_fingerprint_at_failure,
        } = &self.publish_state
        else {
            return None;
        };
        if *generation != request_generation || *generation != self.generation {
            return None;
        }

        match (clipboard_fingerprint_at_failure.as_ref(), current.as_ref()) {
            (Some(previous), Some(current)) if previous == current => {}
            (None, None) => return None,
            _ => {
                self.mark_superseded(Some(*generation));
                return None;
            }
        }

        self.shapes()
    }

    pub(crate) fn mark_superseded(&mut self, generation: Option<u64>) {
        if generation == Some(self.generation) && !self.is_empty() {
            self.publish_state = SelectionPublishState::Superseded {
                generation: self.generation,
            };
        }
    }

    pub(crate) fn set_pending_image_fallback(&mut self, fallback: PendingClipboardFallback) {
        self.pending_image_fallback = Some(fallback);
    }

    pub(crate) fn take_pending_image_fallback(&mut self) -> Option<PendingClipboardFallback> {
        self.pending_image_fallback.take()
    }

    pub(crate) fn restore_pending_image_fallback(&mut self, fallback: PendingClipboardFallback) {
        self.pending_image_fallback = Some(fallback);
    }

    #[cfg(test)]
    pub(crate) fn has_pending_image_fallback(&self) -> bool {
        self.pending_image_fallback.is_some()
    }
}

impl LocalSelectionContext {
    pub(crate) fn active_paste_request_id(&self) -> Option<u64> {
        self.active_paste_request_id
    }

    pub(crate) fn fallback_generation(&self) -> Option<u64> {
        self.fallback_allowed().then_some(self.generation)
    }

    pub(crate) fn fallback_allowed(&self) -> bool {
        if self
            .shapes
            .as_ref()
            .is_none_or(|clipboard| clipboard.is_empty())
        {
            return false;
        }
        match self.publish_state {
            SelectionPublishState::NotAttempted => true,
            SelectionPublishState::Failed { generation, .. }
            | SelectionPublishState::Published { generation } => generation == self.generation,
            SelectionPublishState::Superseded { .. } => false,
        }
    }

    pub(crate) fn shapes_for_fallback(&self, generation: u64) -> Option<Vec<Shape>> {
        (generation == self.generation && self.fallback_allowed())
            .then(|| self.shapes.clone())
            .flatten()
            .filter(|shapes| !shapes.is_empty())
    }

    pub(crate) fn shapes_for_pending_publish(&self, generation: Option<u64>) -> Option<Vec<Shape>> {
        let generation = generation?;
        (generation == self.generation
            && matches!(self.publish_state, SelectionPublishState::NotAttempted))
        .then(|| self.shapes.clone())
        .flatten()
        .filter(|shapes| !shapes.is_empty())
    }

    pub(crate) fn failed_probe(
        &self,
        generation: Option<u64>,
    ) -> Option<(u64, Option<ClipboardFingerprint>)> {
        let request_generation = generation?;
        let SelectionPublishState::Failed {
            generation,
            clipboard_fingerprint_at_failure,
        } = &self.publish_state
        else {
            return None;
        };
        (*generation == request_generation
            && *generation == self.generation
            && !self.shapes.as_ref().is_none_or(Vec::is_empty))
        .then(|| (*generation, clipboard_fingerprint_at_failure.clone()))
    }

    pub(crate) fn private_payload_matches_request_selection(
        &self,
        request: &ClipboardPasteRequest,
        payload: &WayscriberClipboardSelection,
    ) -> bool {
        payload.app_instance_id == self.app_instance_id
            && request.local_selection_fallback_generation == Some(payload.copy_generation)
    }

    pub(crate) fn private_payload_is_same_instance(
        &self,
        payload: &WayscriberClipboardSelection,
    ) -> bool {
        payload.app_instance_id == self.app_instance_id
    }

    pub(crate) fn private_payload_shapes_for_request(
        &self,
        request: &ClipboardPasteRequest,
        payload: WayscriberClipboardSelection,
    ) -> Option<Vec<Shape>> {
        if payload.app_instance_id == self.app_instance_id {
            if request.local_selection_fallback_generation == Some(payload.copy_generation) {
                if self.generation == payload.copy_generation
                    && let Some(shapes) = &self.shapes
                    && !shapes.is_empty()
                {
                    return Some(shapes.clone());
                }
                return non_empty_shapes(payload.shapes);
            }

            if request.local_selection_fallback_generation.is_none()
                && payload.copy_generation == request.selection_clipboard_generation_at_request
            {
                return non_empty_shapes(payload.shapes);
            }

            return None;
        }

        non_empty_shapes(payload.shapes)
    }
}

fn non_empty_shapes(shapes: Vec<Shape>) -> Option<Vec<Shape>> {
    (!shapes.is_empty()).then_some(shapes)
}

fn new_clipboard_app_instance_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{}-{millis}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::WHITE;

    fn rectangle(x: i32) -> Shape {
        Shape::Rect {
            x,
            y: 20,
            w: 30,
            h: 40,
            fill: false,
            color: WHITE,
            thick: 2.0,
        }
    }

    #[test]
    fn new_copy_invalidates_stale_publication_and_keeps_current_fallback() {
        let mut clipboard = SelectionClipboard::default();
        let first = clipboard
            .copy_shapes(vec![rectangle(10)])
            .expect("first publish");
        assert!(clipboard.complete_publish(first.generation, None, true));

        let second = clipboard
            .copy_shapes(vec![rectangle(200)])
            .expect("second publish");
        assert!(!clipboard.complete_publish(first.generation, None, false));

        let snapshot = clipboard.snapshot();
        assert_eq!(snapshot.fallback_generation(), Some(second.generation));
        let shapes = snapshot
            .shapes_for_pending_publish(Some(second.generation))
            .expect("current pending shapes");
        assert!(matches!(shapes.as_slice(), [Shape::Rect { x: 200, .. }]));
    }

    #[test]
    fn paste_request_identity_advances_and_only_active_request_finishes() {
        let mut clipboard = SelectionClipboard::default();
        clipboard.copy_shapes(vec![rectangle(10)]);
        let first = clipboard.begin_paste_request(
            "board".to_string(),
            0,
            4,
            PasteAnchor::Pointer { x: 50, y: 60 },
            Rect::new(0, 0, 800, 600).expect("visible rect"),
            (800, 600),
        );
        let second = clipboard.begin_paste_request(
            "board".to_string(),
            0,
            4,
            PasteAnchor::VisibleCenter { x: 400, y: 300 },
            Rect::new(0, 0, 800, 600).expect("visible rect"),
            (800, 600),
        );

        assert_eq!(second.id, first.id.wrapping_add(1));
        clipboard.finish_paste_request(first.id);
        assert_eq!(
            clipboard.snapshot().active_paste_request_id(),
            Some(second.id)
        );
        clipboard.finish_paste_request(second.id);
        assert_eq!(clipboard.snapshot().active_paste_request_id(), None);
    }

    #[test]
    fn changed_failure_fingerprint_supersedes_the_local_fallback() {
        let mut clipboard = SelectionClipboard::default();
        let publish = clipboard.copy_shapes(vec![rectangle(10)]).expect("publish");
        let original = ClipboardFingerprint {
            offered_mime_types: vec!["image/png".to_string()],
            selected_mime_type: Some("image/png".to_string()),
            bounded_content_hash: Some(1),
            bounded_content_len: Some(128),
            bounded_content_truncated: false,
        };
        assert!(clipboard.complete_publish(publish.generation, Some(original.clone()), false,));
        assert!(
            clipboard
                .failed_after_fingerprint_probe(Some(publish.generation), Some(original))
                .is_some()
        );

        let changed = ClipboardFingerprint {
            bounded_content_hash: Some(2),
            ..clipboard
                .snapshot()
                .failed_probe(Some(publish.generation))
                .and_then(|(_, fingerprint)| fingerprint)
                .expect("failed fingerprint")
        };
        assert!(
            clipboard
                .failed_after_fingerprint_probe(Some(publish.generation), Some(changed))
                .is_none()
        );
        assert!(!clipboard.snapshot().fallback_allowed());
    }
}
