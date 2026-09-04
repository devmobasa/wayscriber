use super::super::buffer_damage::BufferDamageTracker;
use super::super::*;
use crate::env_vars::FORCE_INLINE_TOOLBARS_ENV;

impl WaylandState {
    pub(in crate::backend::wayland) fn new(init: WaylandStateInit) -> Self {
        let WaylandStateInit {
            globals,
            config,
            input_state,
            startup_activation_token,
            onboarding,
            palette_recents,
            capture_manager,
            session_options,
            session_config_failed,
            persistence,
            runtime_ui,
            runtime_ui_unavailable,
            runtime_wake,
            tokio_handle,
            exit_after_capture_mode,
            frozen_enabled,
            preferred_output_identity,
            xdg_fullscreen,
            main_surface_uses_overlay_layer,
            pending_freeze_on_start,
            screencopy_manager,
            ext_image_copy_managers,
            portal_freeze_supported,
            text_input_manager,
            #[cfg(feature = "tablet-input")]
            tablet_manager,
        } = init;
        #[cfg(feature = "tablet-input")]
        let tablet_settings = {
            TabletSettings {
                enabled: config.tablet.enabled,
                pressure_enabled: config.tablet.pressure_enabled,
                min_thickness: config.tablet.min_thickness,
                max_thickness: config.tablet.max_thickness,
            }
        };

        let placement = SurfacePlacement::new(
            preferred_output_identity,
            xdg_fullscreen,
            main_surface_uses_overlay_layer,
        );
        let force_inline_toolbars = force_inline_toolbars_requested(&config);
        let inline_toolbars = globals.layer_shell().is_none()
            || force_inline_toolbars
            || main_surface_uses_overlay_layer;
        if force_inline_toolbars {
            info!(
                "Forcing inline toolbars (config/ui.toolbar.force_inline or {FORCE_INLINE_TOOLBARS_ENV})"
            );
        }
        if main_surface_uses_overlay_layer {
            info!(
                "Using inline toolbars because the main overlay surface runs above fullscreen windows"
            );
        }
        // Authored offsets are the seeds; a retained runtime override from a
        // committed drag is layered on top. Clamping happens on the first
        // apply against real output geometry, not here, so an override from a
        // now-disconnected monitor degrades instead of being lost.
        let mut positions = crate::backend::wayland::runtime_ui_state::ToolbarPositionSnapshot {
            top: (config.ui.toolbar.top_offset, config.ui.toolbar.top_offset_y),
        };
        if let Some(runtime_ui) = runtime_ui.as_ref() {
            runtime_ui.apply_startup_positions(&mut positions);
        }
        let toolbar_chrome =
            super::super::toolbar::ToolbarChrome::new(inline_toolbars, positions.top);
        drag_log(|| {
            format!(
                "load offsets from config seeds and runtime overrides: top_offset=({}, {})",
                positions.top.0, positions.top.1
            )
        });
        let zoom_manager = screencopy_manager.clone();
        let ui_animation = super::super::ui_animation::UiAnimationClock::from_fps(
            config.performance.ui_animation_fps,
        );

        let buffer_count = config.performance.buffer_count as usize;
        let runtime_operation_ids = RuntimeOperationIdSource::new();
        let font_catalog = super::super::font_catalog::FontCatalogPrewarm::new(
            runtime_operation_ids.clone(),
            runtime_wake.clone(),
        );
        let clipboard = super::super::clipboard_runtime::ClipboardRuntime::new(
            runtime_operation_ids.clone(),
            runtime_wake.clone(),
        );
        let desktop_open =
            RuntimeOperationController::new(runtime_operation_ids.clone(), runtime_wake.clone());
        let region_capture = super::super::region_capture::RegionCaptureRuntime::new(
            runtime_operation_ids,
            runtime_wake.clone(),
        );
        let ocr = crate::ocr::OcrController::new(runtime_wake.clone());
        let preferences = super::super::preference_stores::PreferenceStores::new(
            onboarding,
            palette_recents,
            runtime_ui,
            runtime_ui_unavailable,
            runtime_wake.clone(),
        );

        Self {
            protocol: globals,
            surface: SurfaceState::new(placement),
            toolbar: ToolbarSurfaceManager::new(),
            toolbar_chrome,
            toolbar_drag: super::super::toolbar::ToolbarDrag::new(),
            render: super::super::render::RenderRuntime::new(crate::ui::theme::Theme::resolve(
                config.ui.theme.to_theme_mode(),
            )),
            suppression: Default::default(),
            shortcut_coach: Default::default(),
            focus: super::super::focus::FocusState::new(startup_activation_token),
            buffer_damage: BufferDamageTracker::new(buffer_count),
            spotlight: super::super::spotlight_runtime::SpotlightRuntime::new(),
            config,
            preferences,
            input_state,
            font_catalog,
            clipboard,
            desktop_open,
            region_capture,
            acquisition: Default::default(),
            ocr,
            gtk_toolbar: None,
            ui_animation,
            capture: CaptureState::new(capture_manager),
            frozen: FrozenState::new_with_backends(
                screencopy_manager,
                ext_image_copy_managers,
                portal_freeze_supported,
                runtime_wake.clone(),
                frozen_enabled,
                pending_freeze_on_start,
            ),
            zoom: ZoomState::new_with_runtime_wake(zoom_manager, runtime_wake.clone()),
            perf: perf::PerfMetrics::from_env(),
            exit_after_capture_mode,
            pointer: super::super::pointer_runtime::PointerRuntime::new(),
            key_repeat: Default::default(),
            text_input: super::super::text_input::TextInputState::new(text_input_manager),
            #[cfg(feature = "tablet-input")]
            tablet: super::super::tablet_runtime::TabletState::new(tablet_manager, tablet_settings),
            session: SessionState::new(session_options),
            session_config_failed,
            persistence,
            input_hud: super::super::input_hud::InputHudRuntime::new(runtime_wake.clone()),
            session_dialog: super::super::toolbar::SessionFileDialogController::new(runtime_wake),
            durable_action_finish: None,
            durable_action_retry_at: None,
            tokio_handle,
        }
    }
}
