// The shared toolbar model is being adopted incrementally by both frontends,
// and this file holds staged shapes no renderer reads yet. Scoped here rather
// than over the whole `model` module so the other ~11.5k lines - the specs,
// snapshot mapping, and settings panes that are fully live - are policed for
// dead code again.
#![allow(dead_code)]

//! Renderer-neutral specification of the contextual style pill (island D).
//!
//! The pill is a fourth detached island rendered under the top-strip
//! islands. It morphs with the active tool: the specification derives one
//! [`StylePillState`] from [`ToolContext`]/[`ToolOptionsKind`] and
//! enumerates the ordered control list for that state. Both frontends and
//! the contract tests consume this one structure, mirroring the
//! `TopToolbarSpec` pattern.
//!
//! Select (`ToolOptionsKind::None`) docks the current selection's
//! properties into the pill (`StylePillState::Selection`) while a
//! selection exists, and hides the pill otherwise. The docked controls
//! route through the same apply machinery as the overlay properties
//! popup, which stays available from the context menu.

use std::borrow::Cow;

use crate::config::{
    Action, QuickColorPalette, action_label, action_short_label, toolbar_item_ids as ids,
};
use crate::draw::FontDescriptor;
use crate::input::{EraserMode, SelectionPropertyEntry, SelectionPropertyKind};
use crate::label_format::{format_binding_label, format_quick_color_tooltip};
use crate::ui::toolbar::{ToolContext, ToolOptionsKind, ToolbarEvent, ToolbarSnapshot};

use super::{ToolbarSliderSpec, TopStripPlan, toolbar_item_visible};

mod control;

/// Morph state of the style pill, derived from the active tool's options
/// kind. `Hidden` covers Select without a selection plus the
/// minimized/micro strip forms, where no contextual rows exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillState {
    Hidden,
    /// Select tool with an active selection: the pill docks the selection
    /// properties (the same entry list as the overlay properties popup).
    Selection,
    Stroke,
    Marker,
    Eraser,
    Shape,
    Arrow,
    StepMarker,
    Spotlight,
    Text,
}

impl StylePillState {
    #[cfg(test)]
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Hidden => "hidden",
            Self::Selection => "selection",
            Self::Stroke => "stroke",
            Self::Marker => "marker",
            Self::Eraser => "eraser",
            Self::Shape => "shape",
            Self::Arrow => "arrow",
            Self::StepMarker => "step-marker",
            Self::Spotlight => "spotlight",
            Self::Text => "text",
        }
    }
}

/// Which auto-number counter a reset button targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillCounter {
    Arrow,
    Step,
}

/// One control in the pill's ordered list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillControl {
    /// Current-color chip; opens the big overlay gradient picker popup.
    ColorChip,
    /// Quick-color swatch by palette index (up to [`StylePillSpec::MAX_SWATCHES`]).
    QuickSwatch(usize),
    /// Thickness/size slider. The snapshot's `thickness` already targets
    /// the eraser or marker size when those are active.
    ThicknessSlider,
    /// Live thickness numeral; clicking opens the precise-entry popup.
    ThicknessValue,
    /// Marker opacity slider.
    OpacitySlider,
    /// Spotlight magnification slider.
    SpotlightMagnificationSlider,
    /// Shape fill toggle.
    FillToggle,
    /// Arrow auto-number toggle.
    AutoNumberToggle,
    /// Reset the arrow/step counter; tooltip carries the next number.
    CounterReset(StylePillCounter),
    /// Text size slider.
    FontSizeSlider,
    /// Live text-size numeral; clicking opens the precise-entry popup.
    FontSizeValue,
    /// Sans/Mono font family segmented control.
    FontFamilySegment,
    /// Brush/Stroke eraser mode segmented control (the old checkbox
    /// semantics as a two-segment control emitting `SetEraserMode`).
    EraserModeSegment,
    /// Docked selection property rendered as a compact cycle button that
    /// shows the entry's value; clicking steps the property forward
    /// through the properties apply machinery (Color, Fill, ArrowHead,
    /// TextBackground).
    SelectionCycle(SelectionPropertyKind),
    /// Docked numeric selection property rendered as a −/value/+ stepper;
    /// the halves step the property through the properties apply
    /// machinery (Thickness, FontSize, ArrowLength, ArrowAngle). The
    /// machinery is relative (direction steps), so the pill uses steppers
    /// where the tool states use sliders.
    SelectionStepper(SelectionPropertyKind),
}

/// Presentation role of one pill control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StylePillRole {
    Swatch,
    Slider,
    /// Live numeral button (opens the precise-entry popover).
    Value,
    Toggle,
    Button,
    Segmented,
    /// −/value/+ stepper for docked numeric selection properties.
    Stepper,
}

/// One half of a pill segmented control.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillSegment {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) event: ToolbarEvent,
    pub(crate) active: bool,
    pub(crate) tooltip: String,
}

/// One half of a selection stepper (− or +).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillStep {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) event: ToolbarEvent,
    pub(crate) tooltip: String,
}

/// Stable id fragment for one selection property kind.
pub(crate) const fn selection_kind_slug(kind: SelectionPropertyKind) -> &'static str {
    match kind {
        SelectionPropertyKind::Color => "color",
        SelectionPropertyKind::Thickness => "thickness",
        SelectionPropertyKind::Fill => "fill",
        SelectionPropertyKind::FontSize => "font-size",
        SelectionPropertyKind::ArrowHead => "arrow-head",
        SelectionPropertyKind::ArrowLength => "arrow-length",
        SelectionPropertyKind::ArrowAngle => "arrow-angle",
        SelectionPropertyKind::TextBackground => "text-background",
        SelectionPropertyKind::SpotlightMagnification => "spotlight-magnification",
    }
}

/// The pill control a docked selection entry maps to: relative numeric
/// properties become steppers, everything else a cycle button.
pub(crate) const fn selection_control_for_kind(kind: SelectionPropertyKind) -> StylePillControl {
    match kind {
        SelectionPropertyKind::Color
        | SelectionPropertyKind::Fill
        | SelectionPropertyKind::ArrowHead
        | SelectionPropertyKind::TextBackground => StylePillControl::SelectionCycle(kind),
        SelectionPropertyKind::Thickness
        | SelectionPropertyKind::FontSize
        | SelectionPropertyKind::ArrowLength
        | SelectionPropertyKind::ArrowAngle
        | SelectionPropertyKind::SpotlightMagnification => StylePillControl::SelectionStepper(kind),
    }
}

fn selection_entry(
    snapshot: &ToolbarSnapshot,
    kind: SelectionPropertyKind,
) -> Option<&SelectionPropertyEntry> {
    snapshot
        .selection_properties
        .iter()
        .find(|entry| entry.kind == kind)
}

/// The style pill for one snapshot: a morph state plus its ordered controls.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StylePillSpec {
    state: StylePillState,
    controls: Vec<StylePillControl>,
}

impl StylePillSpec {
    /// Maximum quick-color swatches shown in the pill: always the strip's
    /// quick-color cap, so widening one ladder can never silently leave
    /// the other behind.
    pub(crate) const MAX_SWATCHES: usize = TopStripPlan::MAX_QUICK_COLORS;

    pub(crate) fn build(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> Self {
        let state = Self::state_of(snapshot, plan);
        if state == StylePillState::Hidden {
            return Self {
                state,
                controls: Vec::new(),
            };
        }

        if state == StylePillState::Selection {
            let controls = snapshot
                .selection_properties
                .iter()
                .map(|entry| selection_control_for_kind(entry.kind))
                .collect();
            return Self { state, controls };
        }

        let context = ToolContext::from_snapshot(snapshot);
        let mut controls = Vec::new();
        if context.needs_color {
            controls.push(StylePillControl::ColorChip);
            // The quick-color swatch row is the on-strip home of the
            // "Quick colors" customization toggle: M7 moved colors off the
            // strip into this pill (M7-C1), so the toggle now gates these
            // swatches (the color chip stays either way). Hiding the item
            // hides the swatch row.
            if toolbar_item_visible(snapshot, ids::TOP_GROUP_QUICK_COLORS) {
                // Swatches follow the strip's width-degradation plan (8→6→4→0)
                // so the pill narrows with the islands above it.
                let count = snapshot
                    .quick_colors
                    .rendered_entries()
                    .len()
                    .min(Self::MAX_SWATCHES)
                    .min(plan.swatch_count);
                controls.extend((0..count).map(StylePillControl::QuickSwatch));
            }
        }
        if context.needs_thickness {
            controls.push(StylePillControl::ThicknessSlider);
            controls.push(StylePillControl::ThicknessValue);
        }
        if context.show_marker_opacity {
            controls.push(StylePillControl::OpacitySlider);
        }
        if context.tool_options_kind == ToolOptionsKind::Spotlight {
            controls.push(StylePillControl::SpotlightMagnificationSlider);
        }
        if context.show_fill_toggle {
            controls.push(StylePillControl::FillToggle);
        }
        if context.show_arrow_labels {
            controls.push(StylePillControl::AutoNumberToggle);
            if snapshot.arrow_label_enabled {
                controls.push(StylePillControl::CounterReset(StylePillCounter::Arrow));
            }
        }
        if context.show_step_counter {
            controls.push(StylePillControl::CounterReset(StylePillCounter::Step));
        }
        if context.show_font_controls {
            controls.push(StylePillControl::FontSizeSlider);
            controls.push(StylePillControl::FontSizeValue);
            controls.push(StylePillControl::FontFamilySegment);
        }
        if context.show_eraser_mode {
            controls.push(StylePillControl::EraserModeSegment);
        }

        Self { state, controls }
    }

    pub(crate) fn state(&self) -> StylePillState {
        self.state
    }

    pub(crate) fn controls(&self) -> &[StylePillControl] {
        &self.controls
    }

    /// Allocation-free visibility query for the sizing/planning paths. Every
    /// non-hidden state materializes at least one control (pinned by test),
    /// so this equals `!build(snapshot, plan).controls().is_empty()`.
    pub(crate) fn visible(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> bool {
        Self::state_of(snapshot, plan) != StylePillState::Hidden
    }

    /// Allocation-free morph-state query matching [`Self::build`].
    pub(crate) fn state_of(snapshot: &ToolbarSnapshot, plan: &TopStripPlan) -> StylePillState {
        if snapshot.top_minimized || snapshot.top_micro_active() {
            return StylePillState::Hidden;
        }
        // The last-resort compact presentation keeps only the protected
        // strip core: the pill yields entirely under that width pressure.
        if plan.compact {
            return StylePillState::Hidden;
        }
        match ToolContext::from_snapshot(snapshot).tool_options_kind {
            // Select: docks the selection properties while a selection
            // exists; hidden otherwise.
            ToolOptionsKind::None => {
                if snapshot.selection_properties.is_empty() {
                    StylePillState::Hidden
                } else {
                    StylePillState::Selection
                }
            }
            ToolOptionsKind::Stroke => StylePillState::Stroke,
            ToolOptionsKind::Marker => StylePillState::Marker,
            ToolOptionsKind::Eraser => StylePillState::Eraser,
            ToolOptionsKind::Shape => StylePillState::Shape,
            ToolOptionsKind::Arrow => StylePillState::Arrow,
            ToolOptionsKind::StepMarker => StylePillState::StepMarker,
            ToolOptionsKind::Spotlight => StylePillState::Spotlight,
            ToolOptionsKind::Text => StylePillState::Text,
        }
    }
}

#[cfg(test)]
mod tests;
