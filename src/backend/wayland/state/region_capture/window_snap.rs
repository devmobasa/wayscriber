use crate::backend::wayland::state::WaylandState;
use crate::backend::wayland::state::screen_image::{ScreenSourceToken, screen_rect_for_image_rect};
use crate::capture::window_geometry::{WindowQueryContext, WindowTarget};
use crate::input::state::{RegionInputSource, RegionPurposeTag, RegionSelection};
use crate::screen_pixels::{ImagePixelRect, ImagePoint};
use crate::util::Rect;

mod query;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::backend::wayland::state) struct WindowSnapCorrelation {
    generation: u64,
    source: ScreenSourceToken,
}

impl WindowSnapCorrelation {
    pub(super) const fn new(generation: u64, source: ScreenSourceToken) -> Self {
        Self { generation, source }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland) struct WindowSnapQuery {
    pub(super) correlation: WindowSnapCorrelation,
    pub(super) context: WindowQueryContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::backend::wayland) struct WindowSnapTarget {
    image_rect: ImagePixelRect,
    screen_rect: Rect,
}

impl WindowSnapTarget {
    pub(in crate::backend::wayland) const fn image_rect(&self) -> ImagePixelRect {
        self.image_rect
    }

    #[cfg(test)]
    pub(in crate::backend::wayland) const fn screen_rect(&self) -> Rect {
        self.screen_rect
    }
}

#[derive(Debug, Clone, PartialEq)]
enum WindowSnapAvailability {
    Pending(WindowSnapQueryStage),
    Ready {
        targets: Vec<WindowSnapTarget>,
        display_selections: Vec<RegionSelection>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WindowSnapQueryStage {
    Queued(WindowQueryContext),
    Started,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::backend::wayland::state) struct WindowSnapSession {
    correlation: WindowSnapCorrelation,
    availability: WindowSnapAvailability,
    mode_active: bool,
    hovered: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::backend::wayland) enum WindowSnapDirection {
    Left,
    Right,
    Up,
    Down,
}

impl WindowSnapSession {
    #[cfg(test)]
    pub(super) const fn pending(correlation: WindowSnapCorrelation) -> Self {
        Self {
            correlation,
            availability: WindowSnapAvailability::Pending(WindowSnapQueryStage::Started),
            mode_active: false,
            hovered: None,
        }
    }

    pub(super) fn queued(correlation: WindowSnapCorrelation, context: WindowQueryContext) -> Self {
        Self {
            correlation,
            availability: WindowSnapAvailability::Pending(WindowSnapQueryStage::Queued(context)),
            mode_active: false,
            hovered: None,
        }
    }

    pub(super) fn queued_query(&self) -> Option<WindowSnapQuery> {
        let WindowSnapAvailability::Pending(WindowSnapQueryStage::Queued(context)) =
            &self.availability
        else {
            return None;
        };
        Some(WindowSnapQuery {
            correlation: self.correlation,
            context: context.clone(),
        })
    }

    pub(super) fn mark_query_started(&mut self, correlation: WindowSnapCorrelation) -> bool {
        if self.correlation != correlation
            || !matches!(
                self.availability,
                WindowSnapAvailability::Pending(WindowSnapQueryStage::Queued(_))
            )
        {
            return false;
        }
        self.availability = WindowSnapAvailability::Pending(WindowSnapQueryStage::Started);
        true
    }

    pub(super) const fn correlation(&self) -> WindowSnapCorrelation {
        self.correlation
    }

    pub(super) fn is_ready(&self) -> bool {
        matches!(self.availability, WindowSnapAvailability::Ready { .. })
    }

    pub(super) const fn mode_active(&self) -> bool {
        self.mode_active
    }

    pub(super) fn targets(&self) -> &[WindowSnapTarget] {
        match &self.availability {
            WindowSnapAvailability::Pending(_) => &[],
            WindowSnapAvailability::Ready { targets, .. } => targets,
        }
    }

    pub(super) fn display_selections(&self) -> &[RegionSelection] {
        match &self.availability {
            WindowSnapAvailability::Pending(_) => &[],
            WindowSnapAvailability::Ready {
                display_selections, ..
            } => display_selections,
        }
    }

    pub(super) fn hovered_target(&self) -> Option<&WindowSnapTarget> {
        self.hovered.and_then(|index| self.targets().get(index))
    }

    pub(super) const fn hovered_index(&self) -> Option<usize> {
        self.hovered
    }

    pub(super) fn toggle_mode(&mut self) -> bool {
        if !self.is_ready() {
            return false;
        }
        self.mode_active = !self.mode_active;
        self.hovered = None;
        true
    }

    pub(super) fn update_hover(&mut self, point: (f64, f64)) -> bool {
        let next = if self.mode_active && point.0.is_finite() && point.1.is_finite() {
            let x = point.0.floor() as i32;
            let y = point.1.floor() as i32;
            self.targets()
                .iter()
                .rposition(|target| target.screen_rect.contains(x, y))
        } else {
            None
        };
        if self.hovered == next {
            return false;
        }
        self.hovered = next;
        true
    }

    pub(super) fn navigate(&mut self, direction: WindowSnapDirection, pointer: (f64, f64)) -> bool {
        if !self.mode_active || !pointer.0.is_finite() || !pointer.1.is_finite() {
            return false;
        }
        let origin = self
            .hovered_target()
            .map(|target| rect_center(target.screen_rect))
            .unwrap_or(pointer);
        let current = self.hovered;
        let mut best: Option<(usize, f64)> = None;
        // Reverse provider order makes the visually topmost target win an
        // exact score tie, matching pointer hit testing.
        for (index, target) in self.targets().iter().enumerate().rev() {
            if Some(index) == current {
                continue;
            }
            let center = rect_center(target.screen_rect);
            let delta = (center.0 - origin.0, center.1 - origin.1);
            let (along, across) = match direction {
                WindowSnapDirection::Left if delta.0 < 0.0 => (-delta.0, delta.1.abs()),
                WindowSnapDirection::Right if delta.0 > 0.0 => (delta.0, delta.1.abs()),
                WindowSnapDirection::Up if delta.1 < 0.0 => (-delta.1, delta.0.abs()),
                WindowSnapDirection::Down if delta.1 > 0.0 => (delta.1, delta.0.abs()),
                WindowSnapDirection::Left
                | WindowSnapDirection::Right
                | WindowSnapDirection::Up
                | WindowSnapDirection::Down => continue,
            };
            let score = along + across * 1.75;
            if best.is_none_or(|(_, best_score)| score < best_score) {
                best = Some((index, score));
            }
        }
        let Some((next, _)) = best else {
            return false;
        };
        self.hovered = Some(next);
        true
    }
}

fn rect_center(rect: Rect) -> (f64, f64) {
    (
        f64::from(rect.x) + f64::from(rect.width) / 2.0,
        f64::from(rect.y) + f64::from(rect.height) / 2.0,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WindowQueryApply {
    Stale,
    Unavailable,
    Ready,
}

pub(super) fn apply_window_query_completion(
    session: &mut Option<WindowSnapSession>,
    correlation: WindowSnapCorrelation,
    targets: Vec<WindowTarget>,
) -> WindowQueryApply {
    let Some(current) = session.as_ref() else {
        return WindowQueryApply::Stale;
    };
    if current.correlation != correlation
        || !matches!(
            current.availability,
            WindowSnapAvailability::Pending(WindowSnapQueryStage::Started)
        )
    {
        return WindowQueryApply::Stale;
    }
    let mapped: Vec<_> = targets
        .into_iter()
        .filter_map(|target| map_window_target(correlation.source, target))
        .collect();
    if mapped.is_empty() {
        *session = None;
        return WindowQueryApply::Unavailable;
    }
    let current = session
        .as_mut()
        .expect("a correlated pending window-snap session still exists");
    let display_selections = mapped
        .iter()
        .map(|target| region_selection_for_rect(target.screen_rect))
        .collect();
    current.availability = WindowSnapAvailability::Ready {
        targets: mapped,
        display_selections,
    };
    WindowQueryApply::Ready
}

fn region_selection_for_rect(rect: Rect) -> RegionSelection {
    RegionSelection {
        start: (f64::from(rect.x), f64::from(rect.y)),
        end: (
            f64::from(rect.x) + f64::from(rect.width),
            f64::from(rect.y) + f64::from(rect.height),
        ),
    }
}

fn map_window_target(source: ScreenSourceToken, target: WindowTarget) -> Option<WindowSnapTarget> {
    let logical_rect = target.logical_rect;
    let first = output_logical_to_image_point(source, (logical_rect.x, logical_rect.y))?;
    let second = output_logical_to_image_point(
        source,
        (
            logical_rect.x.checked_add(logical_rect.width)?,
            logical_rect.y.checked_add(logical_rect.height)?,
        ),
    )?;
    let image_rect = ImagePixelRect::from_points(first, second, source.image_size)?;
    Some(WindowSnapTarget {
        image_rect,
        screen_rect: screen_rect_for_image_rect(&source, image_rect),
    })
}

fn output_logical_to_image_point(
    source: ScreenSourceToken,
    point: (i32, i32),
) -> Option<ImagePoint> {
    if point.0 < 0 || point.1 < 0 || source.surface.0 == 0 || source.surface.1 == 0 {
        return None;
    }
    Some(ImagePoint::new(
        f64::from(point.0) * f64::from(source.image_size.0) / f64::from(source.surface.0),
        f64::from(point.1) * f64::from(source.image_size.1) / f64::from(source.surface.1),
    ))
}

impl WaylandState {
    pub(in crate::backend::wayland) fn region_window_snap_available(&self) -> bool {
        self.data
            .window_snap
            .as_ref()
            .is_some_and(WindowSnapSession::is_ready)
    }

    pub(in crate::backend::wayland) fn region_window_snap_active(&self) -> bool {
        self.data
            .window_snap
            .as_ref()
            .is_some_and(WindowSnapSession::mode_active)
    }

    pub(in crate::backend::wayland) fn region_window_snap_display_selections(
        &self,
    ) -> &[RegionSelection] {
        self.data
            .window_snap
            .as_ref()
            .map(WindowSnapSession::display_selections)
            .unwrap_or_default()
    }

    pub(in crate::backend::wayland) fn region_window_snap_highlighted_index(
        &self,
    ) -> Option<usize> {
        self.data
            .window_snap
            .as_ref()
            .and_then(WindowSnapSession::hovered_index)
    }

    pub(in crate::backend::wayland) fn toggle_region_window_snap(&mut self) -> bool {
        self.cancel_screen_modals_if_source_changed();
        let ui = self.input_state.region_state();
        if !ui.is_active()
            || ui.is_review()
            || !ui.purpose().is_some_and(RegionPurposeTag::is_capture)
        {
            return false;
        }
        let owner = ui.selection_owner();
        let toggled = self
            .data
            .window_snap
            .as_mut()
            .is_some_and(WindowSnapSession::toggle_mode);
        if !toggled {
            return false;
        }
        if owner.is_some() {
            super::events::rearm_region_selection_event(
                &mut self.data.active_screen_region,
                &mut self.input_state,
            );
            self.retire_region_selection_owner(owner);
        }
        self.update_region_window_hover(self.current_region_pointer());
        self.mark_region_window_snap_dirty();
        true
    }

    pub(in crate::backend::wayland) fn navigate_region_window_snap(
        &mut self,
        direction: WindowSnapDirection,
    ) -> bool {
        self.cancel_screen_modals_if_source_changed();
        let pointer = self.current_region_pointer();
        let changed = self
            .data
            .window_snap
            .as_mut()
            .is_some_and(|session| session.navigate(direction, pointer));
        if changed {
            self.mark_region_window_snap_dirty();
        }
        changed
    }

    pub(in crate::backend::wayland) fn choose_hovered_region_window(&mut self) -> bool {
        self.choose_region_window(None, None)
    }

    pub(super) fn begin_region_window_choice(
        &mut self,
        owner: RegionInputSource,
        point: (f64, f64),
    ) -> bool {
        if !self.region_window_snap_active() {
            return false;
        }
        self.choose_region_window(Some(owner), Some(point));
        true
    }

    pub(super) fn update_region_window_hover(&mut self, point: (f64, f64)) -> bool {
        if !self.region_window_snap_active() {
            return false;
        }
        let changed = self
            .data
            .window_snap
            .as_mut()
            .is_some_and(|session| session.update_hover(point));
        if changed {
            self.mark_region_window_snap_dirty();
        }
        true
    }

    fn choose_region_window(
        &mut self,
        owner: Option<RegionInputSource>,
        point: Option<(f64, f64)>,
    ) -> bool {
        if let Some(point) = point {
            self.update_region_window_hover(point);
        }
        let Some(rect) = self
            .data
            .window_snap
            .as_ref()
            .and_then(WindowSnapSession::hovered_target)
            .map(WindowSnapTarget::image_rect)
        else {
            return false;
        };
        let Some(purpose) = self
            .data
            .active_screen_region
            .map(|region| region.purpose())
            .filter(|purpose| purpose.is_capture())
        else {
            return false;
        };
        self.clear_region_window_snap();
        self.retire_region_selection_owner(owner);
        if purpose == RegionPurposeTag::CaptureInteractive {
            self.enter_region_review(rect)
        } else {
            self.submit_region_capture(rect);
            true
        }
    }

    pub(super) fn highlighted_region_window_rect(&self) -> Option<ImagePixelRect> {
        self.data
            .window_snap
            .as_ref()
            .filter(|session| session.mode_active())
            .and_then(WindowSnapSession::hovered_target)
            .map(WindowSnapTarget::image_rect)
    }

    fn current_region_pointer(&self) -> (f64, f64) {
        let (x, y) = self.current_mouse();
        (f64::from(x), f64::from(y))
    }

    fn mark_region_window_snap_dirty(&mut self) {
        self.input_state.dirty_tracker.mark_full();
        self.input_state.needs_redraw = true;
    }
}
