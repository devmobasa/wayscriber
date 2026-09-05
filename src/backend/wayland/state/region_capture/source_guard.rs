use super::*;

pub(super) fn screen_region_invariant(
    backend: Option<ActiveScreenRegion>,
    ui: RegionSelectUiState,
) -> bool {
    backend
        .map(ActiveScreenRegion::ui_state)
        .unwrap_or_default()
        == ui
}

pub(super) fn active_region_source_changed(
    region: Option<ActiveScreenRegion>,
    surface: (u32, u32),
    source_matches: &impl Fn(ScreenSourceToken) -> bool,
) -> bool {
    match region {
        Some(ActiveScreenRegion::Ready { source, .. }) => !source_matches(source),
        Some(ActiveScreenRegion::Measure { bounds, .. }) => bounds != surface,
        Some(ActiveScreenRegion::PendingFrozen { .. } | ActiveScreenRegion::PendingZoom { .. })
        | None => false,
    }
}

pub(super) fn active_eyedropper_source_changed(
    active: bool,
    expected_source: Option<ScreenSourceToken>,
    source_matches: &impl Fn(ScreenSourceToken) -> bool,
) -> bool {
    active && !expected_source.is_some_and(source_matches)
}

pub(in crate::backend::wayland::state) fn owned_generation_is_current(
    expected: u64,
    current: u64,
    frozen_active: bool,
) -> bool {
    frozen_active && expected == current
}
