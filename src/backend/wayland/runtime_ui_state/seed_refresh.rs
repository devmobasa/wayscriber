//! Shared seed reconciliation and publication; only protocol drag teardown is adapter-owned.
use super::{ToolbarPositionSnapshot, ToolbarRuntimeState};
use crate::backend::wayland::{
    state::{ToolbarChrome, ToolbarDrag},
    toolbar::ToolbarSurfaceManager,
};
use crate::{config::Config, draw::TextMeasurer, input::InputState, ui_text::UiTextEngine};

pub(in crate::backend::wayland) struct SeedRefreshContext<'a> {
    pub config: &'a Config,
    pub input: &'a mut InputState,
    pub engine: &'a UiTextEngine,
    pub measurer: &'a TextMeasurer,
    pub runtime: Option<&'a mut ToolbarRuntimeState>,
    pub drag: &'a mut ToolbarDrag,
    pub chrome: &'a mut ToolbarChrome,
    pub toolbar: &'a mut ToolbarSurfaceManager,
}

pub(in crate::backend::wayland) trait RuntimeUiSeedRefresh {
    fn seed_refresh_context(&mut self) -> SeedRefreshContext<'_>;
    fn cancel_position_drags(&mut self);
}

pub(in crate::backend::wayland) fn refresh_runtime_ui_config_seeds(
    owner: &mut impl RuntimeUiSeedRefresh,
) {
    let (positions, refresh) = {
        let context = owner.seed_refresh_context();

        context
            .input
            .boards
            .sync_pin_seeds_from_config(&context.config.resolved_boards());

        let Some(runtime) = context.runtime else {
            return;
        };

        let mut positions = ToolbarPositionSnapshot::from_chrome(context.chrome);
        let refresh = runtime.refresh_config_seeds(
            context.engine,
            context.measurer,
            context.config,
            context.input,
            &mut positions,
        );
        if !refresh.applied {
            return;
        }

        if refresh.item_drag_aborted {
            context.input.clear_toolbar_item_drag();
            context.drag.set_item_dragging(false);
        }

        (positions, refresh)
    };

    if refresh.position_drag_aborted {
        owner.cancel_position_drags();
    }

    // Seed reconciliation owns preview rollback; protocol drag teardown remains adapter-owned.
    let context = owner.seed_refresh_context();
    context.chrome.set_top_offset(positions.top);
    context.toolbar.mark_dirty();
    context.input.dirty_tracker.mark_full();
    context.input.needs_redraw = true;
}
