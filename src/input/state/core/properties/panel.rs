use super::super::base::InputState;
use super::metrics::MAX_SWATCHES;
use super::panel_layout::selection_panel_anchor;
use super::types::{
    PanelActions, PropertiesPanelLayout, PropertiesPanelLock, PropertiesPanelSwatch,
    SelectionPropertyEntry, SelectionPropertyValue, ShapePropertiesPanel,
};
use super::utils::format_timestamp;
use crate::draw::{Color, TextMeasurer};
use crate::util::Rect;

/// Most shape kinds the multi-selection tooltip names before it trails off.
const MAX_DETAIL_KINDS: usize = 4;

/// Everything the panel shows about the current selection, rebuilt whenever
/// the selection or its shapes change.
struct PanelContents {
    title: String,
    subtitle: Option<String>,
    details: Option<String>,
    lock: PropertiesPanelLock,
    anchor: (f64, f64),
    anchor_rect: Option<Rect>,
    entries: Vec<SelectionPropertyEntry>,
    swatches: Vec<PropertiesPanelSwatch>,
    actions: PanelActions,
    preview_color: Option<Color>,
    multiple_selection: bool,
}

impl InputState {
    pub fn properties_panel(&self) -> Option<&ShapePropertiesPanel> {
        self.properties.panel()
    }

    pub fn properties_panel_layout(&self) -> Option<&PropertiesPanelLayout> {
        self.properties.layout()
    }

    pub fn is_properties_panel_open(&self) -> bool {
        self.properties.is_open()
    }

    /// Entries for the top-strip style pill's selection docking: the same
    /// list the properties popup shows, built directly from the current
    /// selection. Empty when nothing is selected. The popup itself stays
    /// available from the context menu; the pill is an additional
    /// always-visible surface over the same apply machinery.
    pub fn selection_pill_entries(&self) -> Vec<SelectionPropertyEntry> {
        let ids = self.selected_shape_ids();
        if ids.is_empty() {
            return Vec::new();
        }
        self.build_selection_property_entries(ids)
    }

    pub fn close_properties_panel(&mut self) {
        if self.properties.close() {
            self.dirty_tracker.mark_full();
            self.needs_redraw = true;
        }
    }

    /// Forgets the pointer's hover and press, for when another surface opens
    /// over the panel and takes the pointer.
    pub(crate) fn clear_properties_panel_pointer_state(&mut self) {
        if let Some(panel) = self.properties.panel.as_mut() {
            panel.hover = None;
            panel.pressed = None;
        }
        if let Some(layout) = self.properties.layout.as_mut() {
            layout.tooltip = None;
        }
    }

    pub(super) fn set_properties_panel(&mut self, panel: ShapePropertiesPanel) {
        self.properties.open(panel);
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    pub(crate) fn show_properties_panel_with(&mut self, measurer: &TextMeasurer) -> bool {
        if self.selected_shape_ids().is_empty() {
            return false;
        }

        self.close_modals_for_open(crate::input::state::core::modal::ModalSurface::PropertiesPanel);

        let Some(contents) = self.properties_panel_contents(measurer) else {
            return false;
        };

        self.set_properties_panel(ShapePropertiesPanel {
            title: contents.title,
            subtitle: contents.subtitle,
            details: contents.details,
            lock: contents.lock,
            anchor: contents.anchor,
            anchor_rect: contents.anchor_rect,
            entries: contents.entries,
            swatches: contents.swatches,
            actions: contents.actions,
            preview_color: contents.preview_color,
            hover: None,
            pressed: None,
            keyboard_focus: None,
            focus_visible: false,
            scroll: 0.0,
            multiple_selection: contents.multiple_selection,
        });
        true
    }

    pub(super) fn refresh_properties_panel_with(&mut self, measurer: &TextMeasurer) {
        self.properties.begin_refresh();

        let Some(contents) = self.properties_panel_contents(measurer) else {
            self.close_properties_panel();
            return;
        };
        let Some(panel) = self.properties.panel.as_mut() else {
            return;
        };

        panel.title = contents.title;
        panel.subtitle = contents.subtitle;
        panel.details = contents.details;
        panel.lock = contents.lock;
        panel.anchor = contents.anchor;
        panel.anchor_rect = contents.anchor_rect;
        panel.entries = contents.entries;
        panel.swatches = contents.swatches;
        panel.actions = contents.actions;
        panel.preview_color = contents.preview_color;
        panel.multiple_selection = contents.multiple_selection;

        let valid_focus = panel
            .keyboard_focus
            .filter(|idx| *idx < panel.entries.len())
            .filter(|idx| !panel.entries[*idx].disabled);
        panel.keyboard_focus = valid_focus;
        // A row that disappeared takes its hover and press with it; the
        // pending recalculation below re-reads the pointer either way.
        let row_valid = |row: Option<usize>| row.is_none_or(|row| row < panel.entries.len());
        if panel.hover.is_some_and(|hit| !row_valid(hit.row())) {
            panel.hover = None;
        }
        if panel.pressed.is_some_and(|hit| !row_valid(hit.row())) {
            panel.pressed = None;
        }

        self.properties.request_hover_recalc();
        self.dirty_tracker.mark_full();
        self.needs_redraw = true;
    }

    fn properties_panel_contents(&self, measurer: &TextMeasurer) -> Option<PanelContents> {
        let ids = self.selected_shape_ids();
        if ids.is_empty() {
            return None;
        }

        let frame = self.boards.active_frame();
        let anchor_rect = self.selection_screen_bounding_box_with(measurer, ids);
        let anchor = selection_panel_anchor(anchor_rect, self.pointer.screen());
        let entries = self.build_selection_property_entries(ids);
        let swatches = self
            .style
            .quick_colors
            .rendered_entries()
            .iter()
            .take(MAX_SWATCHES)
            .map(|entry| PropertiesPanelSwatch {
                label: entry.label.clone(),
                color: entry.color,
            })
            .collect();
        let preview_color = entries.iter().find_map(|entry| match entry.state {
            SelectionPropertyValue::Color(color) => color,
            _ => None,
        });

        let shapes: Vec<_> = ids.iter().filter_map(|id| frame.shape(*id)).collect();
        let locked = shapes.iter().filter(|shape| shape.locked).count();
        let lock = if locked == 0 {
            PropertiesPanelLock::Unlocked
        } else if locked == shapes.len() {
            PropertiesPanelLock::Locked
        } else {
            PropertiesPanelLock::Partial
        };

        let (title, subtitle, details) = if ids.len() > 1 {
            let total = ids.len();
            let mut subtitle = self
                .selection_bounding_box_with(measurer, ids)
                .map(|bounds| format!("{}×{} px", bounds.width.max(0), bounds.height.max(0)))
                .into_iter()
                .collect::<Vec<_>>();
            if locked > 0 {
                subtitle.push(format!("{locked} of {total} locked"));
            }

            let mut kinds: Vec<&str> = Vec::new();
            for shape in &shapes {
                let kind = shape.shape.kind_name();
                if !kinds.contains(&kind) {
                    kinds.push(kind);
                }
            }
            let mut details = kinds
                .iter()
                .take(MAX_DETAIL_KINDS)
                .copied()
                .collect::<Vec<_>>()
                .join(", ");
            if kinds.len() > MAX_DETAIL_KINDS {
                details.push_str(", …");
            }

            (
                format!("{total} shapes"),
                (!subtitle.is_empty()).then(|| subtitle.join(" · ")),
                (!details.is_empty()).then_some(details),
            )
        } else {
            let shape_id = *ids.first()?;
            let index = frame.find_index(shape_id)?;
            let drawn = frame.shape(shape_id)?;

            let mut subtitle = vec![format!("Layer {} of {}", index + 1, frame.shapes.len())];
            if let Some(bounds) = drawn.bounding_box_with(measurer) {
                subtitle.push(format!("{}×{} px", bounds.width, bounds.height));
            }
            let mut details = vec![format!("Shape ID {shape_id}")];
            if let Some(timestamp) = format_timestamp(drawn.created_at) {
                details.push(format!("Created {timestamp}"));
            }

            (
                drawn.shape.kind_name().to_string(),
                Some(subtitle.join(" · ")),
                Some(details.join(" · ")),
            )
        };

        let actions = PanelActions {
            can_raise: self.selection_can_step(true),
            can_lower: self.selection_can_step(false),
            can_edit: lock != PropertiesPanelLock::Locked,
        };

        Some(PanelContents {
            title,
            subtitle,
            details,
            lock,
            anchor,
            anchor_rect,
            entries,
            swatches,
            actions,
            preview_color,
            multiple_selection: ids.len() > 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeybindingsConfig;
    use crate::draw::{Shape, ShapeId};

    fn make_state() -> InputState {
        let keybindings = KeybindingsConfig::default();
        let _action_map = keybindings
            .build_action_map()
            .expect("default keybindings map");

        crate::input::state::test_support::make_test_input_state()
    }

    fn add_rect(state: &mut InputState, x: i32, y: i32, w: i32, h: i32) -> ShapeId {
        state.boards.active_frame_mut().add_shape(Shape::Rect {
            x,
            y,
            w,
            h,
            fill: false,
            color: state.style.current_color,
            thick: state.style.current_thickness,
        })
    }

    fn set_selection_state(state: &mut InputState, ids: Vec<ShapeId>) {
        state.selection_interaction.set(ids);
    }

    #[test]
    fn show_properties_panel_returns_false_without_selection() {
        let mut state = make_state();
        let measurer = TextMeasurer::default();

        assert!(!state.show_properties_panel_with(&measurer));
        assert!(state.properties_panel().is_none());
    }

    #[test]
    fn refresh_properties_panel_closes_panel_when_selection_is_empty() {
        let mut state = make_state();
        let measurer = TextMeasurer::default();
        let shape_id = add_rect(&mut state, 10, 20, 30, 40);
        state.set_selection(vec![shape_id]);
        assert!(state.show_properties_panel_with(&measurer));

        state.selection_interaction.clear();
        state.refresh_properties_panel_with(&measurer);

        assert!(state.properties_panel().is_none());
    }

    #[test]
    fn refresh_properties_panel_preserves_valid_keyboard_focus() {
        let mut state = make_state();
        let measurer = TextMeasurer::default();
        let shape_id = add_rect(&mut state, 10, 20, 30, 40);
        state.set_selection(vec![shape_id]);
        assert!(state.show_properties_panel_with(&measurer));
        state
            .properties
            .panel
            .as_mut()
            .expect("panel")
            .keyboard_focus = Some(0);

        state.refresh_properties_panel_with(&measurer);

        assert_eq!(
            state
                .properties_panel()
                .and_then(|panel| panel.keyboard_focus),
            Some(0)
        );
    }

    #[test]
    fn refresh_properties_panel_clears_invalid_focus_and_hover() {
        let mut state = make_state();
        let measurer = TextMeasurer::default();
        let shape_id = add_rect(&mut state, 10, 20, 30, 40);
        state.set_selection(vec![shape_id]);
        assert!(state.show_properties_panel_with(&measurer));
        let panel = state.properties.panel.as_mut().expect("panel");
        panel.keyboard_focus = Some(99);
        panel.hover = Some(super::super::types::PropertiesPanelHit::Row(99));
        panel.pressed = Some(super::super::types::PropertiesPanelHit::StepUp(99));

        state.refresh_properties_panel_with(&measurer);

        let panel = state.properties_panel().expect("panel after refresh");
        assert_eq!(panel.keyboard_focus, None);
        assert_eq!(panel.hover, None);
        assert_eq!(panel.pressed, None);
    }

    #[test]
    fn refresh_properties_panel_updates_summary_when_selection_expands() {
        let mut state = make_state();
        let measurer = TextMeasurer::default();
        let first = add_rect(&mut state, 10, 20, 30, 40);
        let second = add_rect(&mut state, 80, 30, 20, 10);
        state.set_selection(vec![first]);
        assert!(state.show_properties_panel_with(&measurer));

        set_selection_state(&mut state, vec![first, second]);
        state.refresh_properties_panel_with(&measurer);

        let panel = state.properties_panel().expect("panel after refresh");
        assert_eq!(panel.title, "2 shapes");
        assert!(panel.multiple_selection);
        let subtitle = panel.subtitle.as_deref().expect("subtitle");
        assert!(subtitle.ends_with(" px"), "{subtitle}");
        assert_eq!(panel.details.as_deref(), Some("Rectangle"));
    }

    #[test]
    fn single_shape_header_names_the_type_layer_size_and_identity() {
        let mut state = make_state();
        let measurer = TextMeasurer::default();
        let _below = add_rect(&mut state, 0, 0, 5, 5);
        let shape_id = add_rect(&mut state, 10, 20, 30, 40);
        state.set_selection(vec![shape_id]);

        assert!(state.show_properties_panel_with(&measurer));

        let panel = state.properties_panel().expect("panel");
        assert_eq!(panel.title, "Rectangle");
        let subtitle = panel.subtitle.as_deref().expect("subtitle");
        assert!(subtitle.starts_with("Layer 2 of 2 · "), "{subtitle}");
        assert!(subtitle.ends_with(" px"), "{subtitle}");
        let details = panel.details.as_deref().expect("details");
        assert!(
            details.starts_with(&format!("Shape ID {shape_id}")),
            "{details}"
        );
        assert_eq!(panel.lock, PropertiesPanelLock::Unlocked);
        assert!(!panel.swatches.is_empty());
        assert!(panel.swatches.len() <= MAX_SWATCHES);
    }

    #[test]
    fn multi_selection_header_counts_locked_shapes_and_names_their_kinds() {
        let mut state = make_state();
        let measurer = TextMeasurer::default();
        let first = add_rect(&mut state, 10, 10, 20, 20);
        let second = state.boards.active_frame_mut().add_shape(Shape::Ellipse {
            cx: 80,
            cy: 30,
            rx: 10,
            ry: 10,
            fill: false,
            color: state.style.current_color,
            thick: state.style.current_thickness,
        });
        let index = state
            .boards
            .active_frame()
            .find_index(second)
            .expect("ellipse index");
        state.boards.active_frame_mut().shapes[index].locked = true;
        state.set_selection(vec![first, second]);

        assert!(state.show_properties_panel_with(&measurer));

        let panel = state.properties_panel().expect("panel");
        assert_eq!(panel.title, "2 shapes");
        let subtitle = panel.subtitle.as_deref().expect("subtitle");
        assert!(subtitle.ends_with(" · 1 of 2 locked"), "{subtitle}");
        assert_eq!(panel.details.as_deref(), Some("Rectangle, Ellipse"));
        assert_eq!(panel.lock, PropertiesPanelLock::Partial);
    }
}
