use super::plan::{CanvasPolicyInputs, PreparedFrame};
use super::profile::FrameProfile;
use super::*;
use crate::backend::wayland::state::buffer_damage::{BufferDamageReport, BufferDamageTracker};
use crate::backend::wayland::surface::AcquiredBuffer;

#[derive(Clone, Copy)]
struct RenderAnimationState {
    highlight: bool,
    preset_feedback: bool,
    ui_toast: bool,
    blocked_feedback: bool,
    text_edit_entry: bool,
    input_hud: bool,
    ocr_scan: bool,
}

impl RenderAnimationState {
    fn any_active(self) -> bool {
        [
            self.highlight,
            self.preset_feedback,
            self.ui_toast,
            self.blocked_feedback,
            self.text_edit_entry,
            self.input_hud,
            self.ocr_scan,
        ]
        .into_iter()
        .any(|active| active)
    }
}

impl WaylandState {
    pub(super) fn prepare_frame(
        &mut self,
        geometry: FrameGeometry,
        visibility: FrameVisibility,
        acquired: &AcquiredBuffer,
        breakdown: &mut Option<PerfRenderBreakdown>,
    ) -> PreparedFrame {
        let width = geometry.width;
        let height = geometry.height;
        self.surface.update_pool_size(acquired.pool_size);

        let now = Instant::now();
        let animation_state = record_stage!(breakdown, advance_animations, {
            self.advance_render_animations(now)
        });
        let ui_animation_active = animation_state.any_active();
        self.ui_animation.schedule(now, ui_animation_active);
        let keep_rendering = ui_animation_active && self.ui_animation.is_uncapped();

        // Add new dirty regions from input state to the per-buffer damage
        // tracker. This runs after the buffer is acquired but before its damage
        // is drained below, so the current frame's changes are included in the
        // damage reported for this slot.
        let logical_width = width.min(i32::MAX as u32) as i32;
        let logical_height = height.min(i32::MAX as u32) as i32;
        let mut damage_diagnostics = record_stage!(breakdown, dirty_collect, {
            self.collect_frame_damage(visibility.render_ui, animation_state, &geometry)
        });

        // Take damage for this buffer slot (identified by canvas memory address).
        // Pool identity (generation + size) is passed to detect pool recreation/growth.
        // SlotPool reuses the same memory regions for released buffers, so the
        // canvas pointer serves as a stable slot identifier across buffer reuse.
        let damage_report = take_frame_damage(
            &mut self.buffer_damage,
            &geometry,
            acquired.canvas_ptr,
            acquired.pool_generation,
            acquired.pool_size,
        );
        damage_diagnostics.buffer_regions_before_merge = damage_report.regions_before_merge;
        damage_diagnostics.buffer_regions_after_merge = damage_report.regions_after_merge;
        let full_damage_reason = damage_report.full_reason;
        let damage_screen = damage_report.regions;
        damage_diagnostics.buffer_covers_surface =
            damage_covers_logical_surface(&damage_screen, logical_width, logical_height);
        let profile = self.input_state.active_render_profile().cloned();
        let remap_canvas = self.input_state.active_canvas_render_profile().is_some();
        let remap_ui = self.input_state.active_ui_render_profile().is_some();
        if let Some(breakdown) = breakdown.as_mut() {
            breakdown.render_profile = PerfRenderProfileKind::from_flags(remap_canvas, remap_ui);
        }
        PreparedFrame {
            geometry,
            visibility,
            canvas: CanvasPolicyInputs {
                capture_picker_active: self.capture_picker_chrome_suppressed(),
                include_drawings: self.region_picker_include_drawings(),
                transform_active: self.canvas_transform_active(),
                origin: self.canvas_view_origin(),
                zoom_scale: self.zoom.active.then_some(self.zoom.scale),
                text_halo_enabled: self.config.drawing.text_halo_enabled,
                layer_cache_usable: self.canvas_layer_cache_usable(),
            },
            damage_screen,
            full_damage_reason,
            damage_diagnostics,
            profile: FrameProfile::new(profile, remap_canvas, remap_ui),
            now,
            keep_rendering,
        }
    }

    fn advance_render_animations(&mut self, now: Instant) -> RenderAnimationState {
        RenderAnimationState {
            highlight: self.input_state.advance_click_highlights(now),
            preset_feedback: self.input_state.advance_preset_feedback(now),
            ui_toast: self.input_state.advance_ui_toast(now),
            blocked_feedback: self.input_state.advance_blocked_feedback(now),
            text_edit_entry: self.input_state.advance_text_edit_entry_feedback(now),
            input_hud: self.input_state.advance_input_hud(now),
            ocr_scan: self.input_state.advance_ocr_scan(now),
        }
    }

    fn collect_frame_damage(
        &mut self,
        render_ui: bool,
        animation: RenderAnimationState,
        geometry: &FrameGeometry,
    ) -> PerfDamageDiagnostics {
        let logical_width = geometry.width.min(i32::MAX as u32) as i32;
        let logical_height = geometry.height.min(i32::MAX as u32) as i32;
        let input_damage_report = self.input_state.take_dirty_region_report();
        let input_damage = input_damage_report.regions;
        let input_full_reason = input_full_damage_reason(input_damage_report.full_reason);
        let diagnostics = PerfDamageDiagnostics {
            input_regions: input_damage.len(),
            input_full_reason,
            input_covers_surface: damage_covers_logical_surface(
                &input_damage,
                logical_width,
                logical_height,
            ),
            ..PerfDamageDiagnostics::default()
        };
        let ui_effects = UiEffectFlags::default()
            .with(UiEffect::UiToast, animation.ui_toast)
            .with(UiEffect::PresetToast, animation.preset_feedback)
            .with(UiEffect::TextEditEntry, animation.text_edit_entry)
            .with(
                UiEffect::StatusHud,
                render_ui && self.input_state.ui_visibility.show_status_bar,
            )
            .with(UiEffect::ZoomChip, render_ui && self.zoom_chip_visible())
            .with(
                UiEffect::InputHud,
                render_ui && self.input_state.input_hud_visible(),
            )
            .with(
                UiEffect::CommandPalette,
                render_ui && self.input_state.command_palette_is_engaged(),
            )
            .with(
                UiEffect::ColorPicker,
                render_ui && self.input_state.is_color_picker_popup_open(),
            )
            .with(
                UiEffect::ToolPreview,
                render_ui && self.mouse_tool_preview_eligible(),
            )
            .with(
                UiEffect::ShapeMeasureBadge,
                render_ui && !self.capture_picker_chrome_suppressed(),
            )
            .with_blocked_feedback(animation.blocked_feedback);
        let ui_effect_damage =
            self.collect_ui_effect_damage(ui_effects, geometry.width, geometry.height);
        if let Some(reason) = self.render_force_full_damage_reason().or(input_full_reason) {
            self.buffer_damage.mark_all_full(reason);
        } else {
            self.buffer_damage.add_regions(input_damage);
            self.buffer_damage.add_regions(ui_effect_damage);
        }
        diagnostics
    }

    fn render_force_full_damage_reason(&self) -> Option<FullDamageReason> {
        if self.zoom.active {
            Some(FullDamageReason::Zoom)
        } else if self.canvas_transform_active() {
            Some(FullDamageReason::BoardPan)
        } else if self
            .spotlight
            .needs_dim_washout(self.input_state.has_spotlight())
        {
            // A spotlight darkens every pixel outside itself, so no partial
            // damage rect can describe adding, moving, or removing one. The
            // previous frame counts as well: after the last spotlight is deleted
            // or undone the flag is already false, yet the buffer on screen still
            // holds its dim layer and only the former opening would be redrawn.
            Some(FullDamageReason::Spotlight)
        } else {
            None
        }
    }
}

fn take_frame_damage(
    tracker: &mut BufferDamageTracker,
    geometry: &FrameGeometry,
    slot_id: usize,
    pool_generation: u64,
    pool_size: usize,
) -> BufferDamageReport {
    let logical_width = geometry.width.min(i32::MAX as u32) as i32;
    let logical_height = geometry.height.min(i32::MAX as u32) as i32;
    let mut report = tracker.take_buffer_damage_report(
        slot_id,
        logical_width,
        logical_height,
        pool_generation,
        pool_size,
    );
    if report.regions.is_empty()
        && let Some(full) = crate::util::Rect::new(0, 0, logical_width, logical_height)
    {
        report.regions = vec![full];
        report.full_reason = Some(FullDamageReason::EmptyDamageFallback);
        tracker.mark_all_full(FullDamageReason::EmptyDamageFallback);
    }
    // Merge diagnostics describe the tracker result before the fallback above.
    report
}

fn input_full_damage_reason(
    reason: Option<crate::draw::DirtyFullReason>,
) -> Option<FullDamageReason> {
    reason.map(|reason| match reason {
        crate::draw::DirtyFullReason::CanvasClear => FullDamageReason::CanvasClear,
        crate::draw::DirtyFullReason::FirstRunOnboarding => FullDamageReason::FirstRunOnboarding,
        crate::draw::DirtyFullReason::InlineToolbar => FullDamageReason::InlineToolbar,
    })
}

#[cfg(test)]
mod tests;
