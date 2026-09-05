//! Capture admission and the layout identity retained through fallback.
#[derive(Debug, Clone, Copy)]
pub(super) struct CaptureLayout {
    output_id: Option<u32>,
    generation: u64,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) enum CapturePreflight<B> {
    #[default]
    Idle,
    Pending {
        backend: B,
        layout: CaptureLayout,
    },
    Capturing {
        layout: CaptureLayout,
    },
}

impl<B: Copy> CapturePreflight<B> {
    pub(super) fn begin(&mut self, backend: B, output_id: Option<u32>, generation: u64) {
        *self = Self::Pending {
            backend,
            layout: CaptureLayout {
                output_id,
                generation,
            },
        };
    }

    pub(super) fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub(super) fn take_pending(&mut self) -> Option<B> {
        let Self::Pending { backend, layout } = *self else {
            return None;
        };
        *self = Self::Capturing { layout };
        Some(backend)
    }

    pub(super) fn layout_matches(&self, output_id: Option<u32>, generation: u64) -> bool {
        let layout = match self {
            Self::Idle => return true,
            Self::Pending { layout, .. } | Self::Capturing { layout } => layout,
        };
        super::portal_capture::layout_token_matches(
            layout.output_id,
            layout.generation,
            output_id,
            generation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_retains_layout_until_reset() {
        let mut phase = CapturePreflight::default();
        phase.begin(7, Some(3), 10);
        assert!(phase.is_pending());
        assert_eq!(phase.take_pending(), Some(7));
        assert!(!phase.is_pending());
        assert_eq!(phase.take_pending(), None);
        assert!(phase.layout_matches(Some(3), 10));
        assert!(!phase.layout_matches(Some(4), 10));
        assert!(!phase.layout_matches(Some(3), 11));
        phase = CapturePreflight::Idle;
        assert!(phase.layout_matches(Some(4), 11));
    }
}
