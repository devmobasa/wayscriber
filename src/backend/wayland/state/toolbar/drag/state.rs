use std::time::{Duration, Instant};

use crate::{
    backend::wayland::state::MoveDragKind,
    toolbar_gtk::{GtkToolbarFeedback, GtkToolbarKind},
};

#[derive(Debug, Clone, Copy)]
struct ApplyThrottle {
    pending_apply: bool,
    last_apply: Option<Instant>,
}

impl ApplyThrottle {
    fn new() -> Self {
        Self {
            pending_apply: false,
            last_apply: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MoveDragPhase {
    Idle,
    Moving {
        kind: MoveDragKind,
        last_coord: (f64, f64),
        coord_is_screen: bool,
        frozen_base: (f64, f64),
        throttle: ApplyThrottle,
    },
    Handoff {
        deadline: Instant,
    },
    GtkPreview {
        kind: Option<GtkToolbarKind>,
        frozen_base_x: f64,
        rebase: Option<(f64, f64)>,
        blocked: bool,
        handoff_deadline: Option<Instant>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland) struct MoveSample {
    pub coord: (f64, f64),
    pub is_screen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland) struct MoveEnd {
    pub commit_base: Option<f64>,
    pub pending_apply: bool,
    pub had_preview: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum HandoffEnd {
    BuiltIn,
    Gtk,
}

pub(in crate::backend::wayland) struct ToolbarDrag {
    item_drag: bool,
    preview: bool,
    flush_requested: bool,
    gtk_top_offset_seq: u64,
    phase: MoveDragPhase,
}

impl ToolbarDrag {
    pub(in crate::backend::wayland) fn new() -> Self {
        Self {
            item_drag: false,
            preview: false,
            flush_requested: false,
            gtk_top_offset_seq: 0,
            phase: MoveDragPhase::Idle,
        }
    }

    pub(in crate::backend::wayland) fn item_dragging(&self) -> bool {
        self.item_drag
    }

    pub(in crate::backend::wayland) fn set_item_dragging(&mut self, dragging: bool) {
        self.item_drag = dragging;
    }

    pub(in crate::backend::wayland) fn preview_active(&self) -> bool {
        self.preview
    }

    pub(in crate::backend::wayland) fn set_preview_active(&mut self, active: bool) {
        self.preview = active;
    }

    pub(in crate::backend::wayland) fn request_flush(&mut self) {
        self.flush_requested = true;
    }

    pub(in crate::backend::wayland) fn take_flush_requested(&mut self) -> bool {
        std::mem::take(&mut self.flush_requested)
    }

    pub(in crate::backend::wayland) fn begin_move(
        &mut self,
        kind: MoveDragKind,
        coord: (f64, f64),
        coord_is_screen: bool,
        frozen_base: (f64, f64),
    ) {
        self.phase = MoveDragPhase::Moving {
            kind,
            last_coord: coord,
            coord_is_screen,
            frozen_base,
            throttle: ApplyThrottle::new(),
        };
    }

    pub(in crate::backend::wayland) fn is_moving(&self) -> bool {
        matches!(self.phase, MoveDragPhase::Moving { .. })
    }

    pub(in crate::backend::wayland) fn kind(&self) -> Option<MoveDragKind> {
        match self.phase {
            MoveDragPhase::Moving { kind, .. } => Some(kind),
            _ => None,
        }
    }

    pub(in crate::backend::wayland) fn frozen_base_x(&self) -> Option<f64> {
        match self.phase {
            MoveDragPhase::Moving { frozen_base, .. } => Some(frozen_base.0),
            MoveDragPhase::GtkPreview {
                kind: Some(_),
                frozen_base_x,
                ..
            } => Some(frozen_base_x),
            MoveDragPhase::Idle
            | MoveDragPhase::Handoff { .. }
            | MoveDragPhase::GtkPreview { kind: None, .. } => None,
        }
    }

    pub(in crate::backend::wayland) fn frozen_base_y(&self) -> Option<f64> {
        match self.phase {
            MoveDragPhase::Moving { frozen_base, .. } => Some(frozen_base.1),
            _ => None,
        }
    }

    pub(in crate::backend::wayland) fn move_sample(&self) -> Option<MoveSample> {
        match self.phase {
            MoveDragPhase::Moving {
                last_coord,
                coord_is_screen,
                ..
            } => Some(MoveSample {
                coord: last_coord,
                is_screen: coord_is_screen,
            }),
            _ => None,
        }
    }

    pub(in crate::backend::wayland) fn note_move(
        &mut self,
        kind: MoveDragKind,
        coord: (f64, f64),
        coord_is_screen: bool,
    ) -> Option<MoveSample> {
        let MoveDragPhase::Moving {
            kind: active_kind,
            last_coord,
            coord_is_screen: active_is_screen,
            ..
        } = &mut self.phase
        else {
            return None;
        };
        if *active_kind != kind {
            return None;
        }
        let previous = MoveSample {
            coord: *last_coord,
            is_screen: *active_is_screen,
        };
        *last_coord = coord;
        *active_is_screen = coord_is_screen;
        Some(previous)
    }

    pub(in crate::backend::wayland) fn move_to(
        &mut self,
        kind: MoveDragKind,
        coord: (f64, f64),
        coord_is_screen: bool,
    ) -> Option<(f64, f64)> {
        let previous = self.note_move(kind, coord, coord_is_screen)?;
        if previous.is_screen != coord_is_screen {
            return None;
        }
        Some((coord.0 - previous.coord.0, coord.1 - previous.coord.1))
    }

    pub(in crate::backend::wayland) fn should_apply(
        &mut self,
        now: Instant,
        interval: Duration,
    ) -> bool {
        let MoveDragPhase::Moving { throttle, .. } = &mut self.phase else {
            return true;
        };
        let should_apply = throttle
            .last_apply
            .is_none_or(|last| now.saturating_duration_since(last) >= interval);
        if should_apply {
            throttle.last_apply = Some(now);
            throttle.pending_apply = false;
        } else {
            throttle.pending_apply = true;
        }
        should_apply
    }

    pub(in crate::backend::wayland) fn note_applied(&mut self, now: Instant) {
        if let MoveDragPhase::Moving { throttle, .. } = &mut self.phase {
            throttle.last_apply = Some(now);
            throttle.pending_apply = false;
        }
    }

    pub(in crate::backend::wayland) fn end_move(&mut self) -> Option<MoveEnd> {
        let MoveDragPhase::Moving {
            frozen_base,
            throttle,
            ..
        } = self.phase
        else {
            return None;
        };
        self.phase = MoveDragPhase::Idle;
        self.item_drag = false;
        Some(MoveEnd {
            commit_base: Some(frozen_base.0),
            pending_apply: throttle.pending_apply,
            had_preview: self.preview,
        })
    }

    pub(in crate::backend::wayland) fn begin_handoff(&mut self, deadline: Instant) {
        match &mut self.phase {
            MoveDragPhase::GtkPreview {
                handoff_deadline, ..
            } => *handoff_deadline = Some(deadline),
            _ => self.phase = MoveDragPhase::Handoff { deadline },
        }
    }

    pub(in crate::backend::wayland) fn handoff_timeout(&self, now: Instant) -> Option<Duration> {
        let deadline = match self.phase {
            MoveDragPhase::Handoff { deadline } => Some(deadline),
            MoveDragPhase::GtkPreview {
                handoff_deadline, ..
            } => handoff_deadline,
            _ => None,
        }?;
        Some(deadline.saturating_duration_since(now))
    }

    pub(in crate::backend::wayland) fn finish_handoff_if_due(
        &mut self,
        now: Instant,
    ) -> Option<HandoffEnd> {
        if self.handoff_timeout(now) != Some(Duration::ZERO) {
            return None;
        }
        self.finish_handoff()
    }

    pub(in crate::backend::wayland) fn finish_handoff(&mut self) -> Option<HandoffEnd> {
        let result = match self.phase {
            MoveDragPhase::Handoff { .. } => Some(HandoffEnd::BuiltIn),
            MoveDragPhase::GtkPreview { .. } => Some(HandoffEnd::Gtk),
            _ if self.preview => Some(HandoffEnd::BuiltIn),
            _ => None,
        };
        match result {
            Some(HandoffEnd::Gtk) => self.phase = MoveDragPhase::Idle,
            Some(HandoffEnd::BuiltIn) => {
                self.phase = MoveDragPhase::Idle;
                self.preview = false;
            }
            None => {}
        }
        result
    }

    pub(in crate::backend::wayland) fn block_gtk_drag(&mut self) {
        self.phase = MoveDragPhase::GtkPreview {
            kind: None,
            frozen_base_x: 0.0,
            rebase: None,
            blocked: true,
            handoff_deadline: None,
        };
    }

    pub(in crate::backend::wayland) fn begin_gtk_preview(
        &mut self,
        kind: GtkToolbarKind,
        frozen_base_x: f64,
    ) {
        self.phase = MoveDragPhase::GtkPreview {
            kind: Some(kind),
            frozen_base_x,
            rebase: None,
            blocked: false,
            handoff_deadline: None,
        };
    }

    pub(in crate::backend::wayland) fn gtk_preview_kind(&self) -> Option<GtkToolbarKind> {
        match self.phase {
            MoveDragPhase::GtkPreview { kind, .. } => kind,
            _ => None,
        }
    }

    pub(in crate::backend::wayland) fn gtk_rebase(&self) -> Option<(f64, f64)> {
        match self.phase {
            MoveDragPhase::GtkPreview { rebase, .. } => rebase,
            _ => None,
        }
    }

    pub(in crate::backend::wayland) fn set_gtk_rebase(&mut self, value: Option<(f64, f64)>) {
        if let MoveDragPhase::GtkPreview { rebase, .. } = &mut self.phase {
            *rebase = value;
        }
    }

    pub(in crate::backend::wayland) fn release_gtk_frozen_base(&mut self, resting_base_x: f64) {
        if let MoveDragPhase::GtkPreview { frozen_base_x, .. } = &mut self.phase {
            *frozen_base_x = resting_base_x;
        }
    }

    pub(in crate::backend::wayland) fn gtk_note_feedback(
        &mut self,
        modal_engaged: bool,
        feedback: &GtkToolbarFeedback,
    ) -> bool {
        match feedback {
            GtkToolbarFeedback::CaptureSuppressionReady { .. }
            | GtkToolbarFeedback::CaptureSuppressionFailed { .. }
            | GtkToolbarFeedback::TopHover { .. } => false,
            GtkToolbarFeedback::Event { .. } | GtkToolbarFeedback::PointerShortcut { .. } => {
                modal_engaged
            }
            GtkToolbarFeedback::SetTopOffset { phase, seq, .. } => {
                if modal_engaged && !matches!(self.phase, MoveDragPhase::GtkPreview { .. }) {
                    self.block_gtk_drag();
                }
                let blocked = match &mut self.phase {
                    MoveDragPhase::GtkPreview { blocked, .. } => {
                        let result = modal_engaged || *blocked;
                        if result {
                            *blocked = !phase.is_end();
                        }
                        result
                    }
                    _ => modal_engaged,
                };
                if blocked {
                    self.gtk_top_offset_seq = self.gtk_top_offset_seq.max(*seq);
                    if phase.is_end()
                        && matches!(self.phase, MoveDragPhase::GtkPreview { kind: None, .. })
                    {
                        self.phase = MoveDragPhase::Idle;
                    }
                }
                blocked
            }
        }
    }

    pub(in crate::backend::wayland) fn note_gtk_offset_seq(&mut self, seq: u64) {
        self.gtk_top_offset_seq = seq;
    }

    pub(in crate::backend::wayland) fn gtk_offset_seq(&self) -> u64 {
        self.gtk_top_offset_seq
    }

    pub(in crate::backend::wayland) fn cancel_gtk(&mut self) -> bool {
        if !matches!(self.phase, MoveDragPhase::GtkPreview { .. }) {
            return false;
        }
        self.phase = MoveDragPhase::Idle;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::toolbar_gtk::{GtkToolbarDragPhase, GtkToolbarSurfaceSize};

    const TEST_SURFACE_SIZE: GtkToolbarSurfaceSize = GtkToolbarSurfaceSize {
        width: 260,
        height: 789,
    };

    fn gtk_offset(phase: GtkToolbarDragPhase, seq: u64) -> GtkToolbarFeedback {
        GtkToolbarFeedback::SetTopOffset {
            x: 10.0,
            y: 20.0,
            surface_size: TEST_SURFACE_SIZE,
            seq,
            phase,
        }
    }

    fn moving(preview: bool) -> ToolbarDrag {
        let mut drag = ToolbarDrag::new();
        drag.set_preview_active(preview);
        drag.begin_move(MoveDragKind::Top, (1.0, 2.0), false, (24.0, 12.0));
        drag
    }

    #[test]
    fn move_and_handoff_transition_table_is_explicit() {
        let now = Instant::now();
        let mut drag = moving(true);
        let ended = drag.end_move().unwrap();
        assert_eq!(ended.commit_base, Some(24.0));
        assert!(ended.had_preview);

        drag.begin_handoff(now + Duration::from_millis(10));
        assert_eq!(drag.finish_handoff_if_due(now), None);
        assert_eq!(
            drag.finish_handoff_if_due(now + Duration::from_millis(10)),
            Some(HandoffEnd::BuiltIn)
        );
        assert!(!drag.preview_active());
    }

    #[test]
    fn move_cancel_can_return_directly_to_idle() {
        let mut drag = moving(false);
        assert!(drag.end_move().is_some());
        assert!(!drag.is_moving());
        assert_eq!(drag.finish_handoff(), None);
    }

    #[test]
    fn gtk_preview_handoff_and_cancel_follow_their_own_phase() {
        let now = Instant::now();
        let mut drag = ToolbarDrag::new();
        drag.begin_handoff(now);
        drag.begin_gtk_preview(GtkToolbarKind::Top, 24.0);
        assert_eq!(drag.handoff_timeout(now), None);
        drag.begin_handoff(now + Duration::from_millis(10));
        assert_eq!(
            drag.finish_handoff_if_due(now + Duration::from_millis(10)),
            Some(HandoffEnd::Gtk)
        );

        drag.begin_gtk_preview(GtkToolbarKind::Top, 24.0);
        assert!(drag.cancel_gtk());
        assert!(!drag.cancel_gtk());
    }

    #[test]
    fn throttle_reports_a_pending_terminal_apply() {
        let start = Instant::now();
        let mut drag = moving(false);
        let interval = Duration::from_millis(20);
        assert!(drag.should_apply(start, interval));
        assert!(!drag.should_apply(start + Duration::from_millis(5), interval));
        assert!(drag.end_move().unwrap().pending_apply);
    }

    #[test]
    fn blocked_gtk_drag_advances_sequence_and_stays_blocked_until_end() {
        let mut drag = ToolbarDrag::new();
        drag.note_gtk_offset_seq(4);

        assert!(drag.gtk_note_feedback(true, &gtk_offset(GtkToolbarDragPhase::Start, 9)));
        assert!(drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::Move, 8)));
        assert_eq!(drag.gtk_offset_seq(), 9);
        assert!(drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::End, 10)));
        assert_eq!(drag.gtk_offset_seq(), 10);
        assert!(!drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::Start, 11)));
    }

    #[test]
    fn passive_and_capture_feedback_follow_modal_policy() {
        let mut drag = ToolbarDrag::new();
        drag.block_gtk_drag();
        assert!(!drag.gtk_note_feedback(
            true,
            &GtkToolbarFeedback::CaptureSuppressionReady { generation: 7 }
        ));
        let shortcut = GtkToolbarFeedback::PointerShortcut {
            button: 8,
            ctrl: false,
            shift: false,
            alt: false,
            logo: false,
        };
        assert!(drag.gtk_note_feedback(true, &shortcut));
        assert!(!drag.gtk_note_feedback(false, &shortcut));
        assert!(drag.gtk_note_feedback(false, &gtk_offset(GtkToolbarDragPhase::Move, 1)));
    }

    #[test]
    fn move_samples_are_updated_with_their_coordinate_space() {
        let mut drag = moving(false);
        assert_eq!(
            drag.move_to(MoveDragKind::Top, (3.0, 4.0), false),
            Some((2.0, 2.0))
        );
        assert_eq!(drag.move_to(MoveDragKind::Top, (5.0, 7.0), true), None);
        assert_eq!(
            drag.move_sample(),
            Some(MoveSample {
                coord: (5.0, 7.0),
                is_screen: true,
            })
        );
    }
}
