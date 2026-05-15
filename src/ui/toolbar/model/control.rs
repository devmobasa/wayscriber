use std::borrow::Cow;
use std::collections::HashSet;

use crate::config::Action;
use crate::draw::Color;

use super::activation::{ToolbarActivation, ToolbarControlId};

#[derive(Debug, Clone)]
pub(crate) struct ToolbarControl {
    pub(crate) id: ToolbarControlId,
    pub(crate) kind: ToolbarControlKind,
    pub(crate) enabled: bool,
    pub(crate) active: bool,
    pub(crate) presentation: ToolbarControlPresentation,
}

#[derive(Debug, Clone)]
pub(crate) enum ToolbarControlKind {
    Single(ToolbarSingleControl),
    Segmented(ToolbarSegmentedControl),
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSingleControl {
    pub(crate) activation: ToolbarActivation,
    pub(crate) action: Option<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ToolbarModelError {
    EmptySegments,
    DuplicateSegmentId(ToolbarControlId),
    MissingActiveSegment(ToolbarControlId),
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSegmentedControl {
    active_segment: Option<ToolbarControlId>,
    segments: Vec<ToolbarSegment>,
}

impl ToolbarSegmentedControl {
    pub(crate) fn try_new(
        active_segment: Option<ToolbarControlId>,
        segments: Vec<ToolbarSegment>,
    ) -> Result<Self, ToolbarModelError> {
        if segments.is_empty() {
            return Err(ToolbarModelError::EmptySegments);
        }

        let mut ids = HashSet::with_capacity(segments.len());
        for segment in &segments {
            if !ids.insert(segment.id) {
                return Err(ToolbarModelError::DuplicateSegmentId(segment.id));
            }
        }

        if let Some(active) = active_segment
            && !ids.contains(&active)
        {
            return Err(ToolbarModelError::MissingActiveSegment(active));
        }

        Ok(Self {
            active_segment,
            segments,
        })
    }

    pub(crate) fn active_segment(&self) -> Option<ToolbarControlId> {
        self.active_segment
    }

    pub(crate) fn segments(&self) -> &[ToolbarSegment] {
        &self.segments
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarSegment {
    pub(crate) id: ToolbarControlId,
    pub(crate) label: Cow<'static, str>,
    pub(crate) activation: ToolbarActivation,
    pub(crate) action: Option<Action>,
    pub(crate) tooltip: ToolbarTooltip,
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ToolbarControlPresentation {
    pub(crate) label: Cow<'static, str>,
    pub(crate) tooltip: ToolbarTooltip,
    pub(crate) icon: Option<ToolbarIcon>,
    pub(crate) role: ToolbarControlRole,
    pub(crate) payload: ToolbarPresentationPayload,
}

impl ToolbarControlPresentation {
    pub(crate) fn button(label: impl Into<Cow<'static, str>>, tooltip: ToolbarTooltip) -> Self {
        Self {
            label: label.into(),
            tooltip,
            icon: None,
            role: ToolbarControlRole::Button,
            payload: ToolbarPresentationPayload::None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ToolbarTooltip {
    None,
    Text(Cow<'static, str>),
    Binding {
        label: Cow<'static, str>,
        binding: Option<String>,
    },
}

impl ToolbarTooltip {
    pub(crate) fn text(text: impl Into<Cow<'static, str>>) -> Self {
        Self::Text(text.into())
    }

    pub(crate) fn as_string(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Text(text) => Some(text.to_string()),
            Self::Binding { label, binding } => Some(crate::label_format::format_binding_label(
                label,
                binding.as_deref(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarControlRole {
    Button,
    DestructiveButton,
    Checkbox,
    Segmented,
    DragHandle,
    ShapePicker,
    PresetSlotAction,
    Slider,
    ColorPicker,
    BoardChip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolbarIcon {
    Settings,
    File,
    More,
    Board,
}

#[derive(Debug, Clone)]
pub(crate) enum ToolbarPresentationPayload {
    None,
    BoardChip(ToolbarBoardChipPresentation),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ToolbarBoardChipPresentation {
    pub(crate) label: String,
    pub(crate) color: Option<Color>,
    pub(crate) board_index: usize,
    pub(crate) board_count: usize,
    pub(crate) page_count: usize,
}
