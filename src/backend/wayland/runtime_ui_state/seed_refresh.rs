//! Shared seed reconciliation and publication; only protocol drag teardown is adapter-owned.
use super::{ToolbarPositionSnapshot, ToolbarRuntimeState};
use crate::backend::wayland::state::{ToolbarChrome, ToolbarDrag};
use crate::backend::wayland::toolbar::ToolbarSurfaceManager;
use crate::config::Config;
use crate::draw::TextMeasurer;
use crate::input::InputState;
use crate::ui_text::UiTextEngine;

pub(in crate::backend::wayland) struct SeedRefreshContext<'state> {
    pub config: &'state Config,
    pub input: &'state mut InputState,
    pub engine: &'state UiTextEngine,
    pub measurer: &'state TextMeasurer,
    pub runtime: Option<&'state mut ToolbarRuntimeState>,
    pub drag: &'state mut ToolbarDrag,
    pub chrome: &'state mut ToolbarChrome,
    pub toolbar: &'state mut ToolbarSurfaceManager,
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
