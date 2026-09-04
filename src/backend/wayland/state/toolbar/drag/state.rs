use std::time::{Duration, Instant};

use crate::toolbar_gtk::{GtkToolbarFeedback, GtkToolbarKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum MoveDragKind {
    Top,
}

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
        sample: MoveSample,
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
pub(super) enum MoveSample {
    Local((f64, f64)),
    Screen((f64, f64)),
}

impl MoveSample {
    fn screen_coord(self, local_origin: (f64, f64)) -> (f64, f64) {
        match self {
            Self::Local(coord) => (local_origin.0 + coord.0, local_origin.1 + coord.1),
            Self::Screen(coord) => coord,
        }
    }
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
            sample: if coord_is_screen {
                MoveSample::Screen(coord)
            } else {
                MoveSample::Local(coord)
            },
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

    /// Consume a rejected event too, so resuming the same drag authority does
    /// not replay movement that happened behind its update barrier.
    pub(super) fn note_move(
        &mut self,
        kind: MoveDragKind,
        sample: MoveSample,
    ) -> Option<MoveSample> {
        let MoveDragPhase::Moving {
            kind: active_kind,
            sample: previous,
            ..
        } = &mut self.phase
        else {
            return None;
        };
        if *active_kind != kind {
            return None;
        }
        Some(std::mem::replace(previous, sample))
    }

    /// Compare screen samples across surfaces, except that a suppressed toolbar's
    /// local preview keeps its own local baseline. Entering that local preview
    /// from screen space rebases without applying a jump.
    pub(super) fn move_to(
        &mut self,
        kind: MoveDragKind,
        sample: MoveSample,
        local_origin: (f64, f64),
    ) -> Option<(f64, f64)> {
        let (coord, previous_coord) = if self.preview
            && let MoveSample::Local(coord) = sample
        {
            let MoveSample::Local(previous) = self.note_move(kind, sample)? else {
                return None;
            };
            (coord, previous)
        } else {
            let coord = sample.screen_coord(local_origin);
            let previous = self.note_move(kind, MoveSample::Screen(coord))?;
            (coord, previous.screen_coord(local_origin))
        };
        Some((coord.0 - previous_coord.0, coord.1 - previous_coord.1))
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
#[path = "tests.rs"]
mod tests;
