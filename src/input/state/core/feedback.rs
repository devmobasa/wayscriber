use super::base::{
    BLOCKED_ACTION_DURATION_MS, CompositorCapabilities, Toast, ToastCommand, ToastPress,
    ToastPriority, ToastPushOutcome, ToastQueue, UiToastState,
};
use crate::domain::Action;
use std::time::{Duration, Instant};

pub(crate) type ToastBounds = (f64, f64, f64, f64);

/// Toast queue, hit geometry, blocked-action animation, and feedback policy.
#[derive(Debug)]
pub(crate) struct Feedback {
    active_toast: Option<UiToastState>,
    queue: ToastQueue,
    toast_bounds: Option<ToastBounds>,
    toast_action_bounds: [Option<ToastBounds>; 2],
    blocked_action_started: Option<Instant>,
    capability_toast_caps: Option<CompositorCapabilities>,
    command_palette_toast_duration_ms: u64,
}

impl Default for Feedback {
    fn default() -> Self {
        Self {
            active_toast: None,
            queue: ToastQueue::default(),
            toast_bounds: None,
            toast_action_bounds: [None, None],
            blocked_action_started: None,
            capability_toast_caps: None,
            command_palette_toast_duration_ms: 1500,
        }
    }
}

impl Feedback {
    pub(crate) fn push(
        &mut self,
        priority: ToastPriority,
        key: &'static str,
        toast: Toast,
        now: Instant,
    ) -> ToastPushOutcome {
        let outcome = self
            .queue
            .push(&mut self.active_toast, priority, key, toast, now);
        if outcome.changed_active() {
            self.clear_geometry();
        }
        outcome
    }

    pub(crate) fn idle(&self) -> bool {
        self.active_toast.is_none() && self.queue.is_empty()
    }

    pub(crate) fn active(&self) -> Option<&UiToastState> {
        self.active_toast.as_ref()
    }

    pub(crate) fn advance(&mut self, now: Instant) -> bool {
        let had_toast = self.active_toast.is_some();
        let (still_showing, activated) = self.queue.advance(&mut self.active_toast, now);
        if activated || (had_toast && !still_showing) {
            self.clear_geometry();
        }
        still_showing
    }

    pub(crate) fn set_geometry(
        &mut self,
        bounds: Option<ToastBounds>,
        action_bounds: [Option<ToastBounds>; 2],
    ) {
        self.toast_bounds = bounds;
        self.toast_action_bounds = action_bounds;
    }

    pub(crate) fn contains(&self, x: i32, y: i32) -> bool {
        self.active_toast.is_some() && contains(self.toast_bounds, x, y)
    }

    pub(crate) fn press_at(&self, x: i32, y: i32) -> Option<ToastPress> {
        let toast = self.active_toast.as_ref()?;
        if !self.contains(x, y) {
            return None;
        }
        if toast.secondary_action.is_none() {
            return Some(ToastPress::body(toast.activation_id));
        }
        Some(match self.action_at(x, y) {
            Some(index) => ToastPress::new(toast.activation_id, index),
            None => ToastPress::body(toast.activation_id),
        })
    }

    pub(crate) fn release_at(
        &mut self,
        pressed: ToastPress,
        x: i32,
        y: i32,
        now: Instant,
    ) -> (bool, Option<ToastCommand>) {
        let Some(toast) = self.active_toast.as_ref() else {
            return (false, None);
        };
        let release_target = toast
            .secondary_action
            .is_some()
            .then(|| self.action_at(x, y))
            .flatten();
        let release_inside = self.contains(x, y);
        if !pressed.matches(toast) || !release_inside || !pressed.matches_target(release_target) {
            return (false, None);
        }

        let command = if toast.secondary_action.is_some() {
            match pressed.action_index() {
                Some(0) => toast.action.as_ref().map(|action| action.command),
                Some(1) => toast.secondary_action.as_ref().map(|action| action.command),
                _ => None,
            }
        } else {
            toast.action.as_ref().map(|action| action.command)
        };
        self.dismiss(now);
        (true, command)
    }

    fn dismiss(&mut self, now: Instant) -> bool {
        let promoted = self.queue.on_dismissed(&mut self.active_toast, now);
        self.clear_geometry();
        promoted
    }

    pub(crate) fn remove_matching(
        &mut self,
        should_remove: impl FnMut(&'static str, Option<Action>) -> bool,
    ) -> bool {
        let active_removed = self
            .queue
            .remove_matching(&mut self.active_toast, should_remove);
        if active_removed {
            self.clear_geometry();
        }
        active_removed
    }

    pub(crate) fn trigger_blocked_action(&mut self, now: Instant) {
        self.blocked_action_started = Some(now);
    }

    pub(crate) fn advance_blocked_action(&mut self, now: Instant) -> bool {
        let Some(started) = self.blocked_action_started else {
            return false;
        };
        if now.saturating_duration_since(started)
            >= Duration::from_millis(BLOCKED_ACTION_DURATION_MS)
        {
            self.blocked_action_started = None;
            return false;
        }
        true
    }

    pub(crate) fn blocked_action_progress(&self, now: Instant) -> Option<f64> {
        let started = self.blocked_action_started?;
        let elapsed = now.saturating_duration_since(started).as_millis() as f64;
        Some((elapsed / BLOCKED_ACTION_DURATION_MS as f64).min(1.0))
    }

    pub(crate) fn note_capability_toast(&mut self, caps: CompositorCapabilities) -> Option<String> {
        if self.capability_toast_caps == Some(caps) {
            return None;
        }
        self.capability_toast_caps = Some(caps);
        caps.limitations_summary()
    }

    pub(crate) const fn command_palette_toast_duration_ms(&self) -> u64 {
        self.command_palette_toast_duration_ms
    }

    pub(crate) fn set_command_palette_toast_duration_ms(&mut self, duration_ms: u64) {
        self.command_palette_toast_duration_ms = duration_ms;
    }

    #[cfg(test)]
    pub(crate) fn toast_count(&self) -> usize {
        usize::from(self.active_toast.is_some()) + self.queue.pending_len()
    }

    #[cfg(test)]
    pub(crate) fn pending_toast_count(&self) -> usize {
        self.queue.pending_len()
    }

    #[cfg(test)]
    pub(crate) const fn geometry(&self) -> Option<ToastBounds> {
        self.toast_bounds
    }

    #[cfg(test)]
    pub(crate) fn blocked_action_active(&self) -> bool {
        self.blocked_action_started.is_some()
    }

    fn action_at(&self, x: i32, y: i32) -> Option<usize> {
        self.toast_action_bounds
            .iter()
            .position(|bounds| contains(*bounds, x, y))
    }

    fn clear_geometry(&mut self) {
        self.toast_bounds = None;
        self.toast_action_bounds = [None, None];
    }
}

fn contains(bounds: Option<ToastBounds>, x: i32, y: i32) -> bool {
    bounds.is_some_and(|(bx, by, bw, bh)| {
        let xf = x as f64;
        let yf = y as f64;
        xf >= bx && xf <= bx + bw && yf >= by && yf <= by + bh
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_preempts_by_priority_and_rate_limits_repeated_content() {
        let now = Instant::now();
        let mut feedback = Feedback::default();
        assert_eq!(
            feedback.push(ToastPriority::Info, "save", Toast::info("Saved"), now),
            ToastPushOutcome::Displayed
        );
        assert_eq!(
            feedback.push(
                ToastPriority::Critical,
                "limit",
                Toast::warning("Limited").once_per_content(),
                now,
            ),
            ToastPushOutcome::Displayed
        );
        assert_eq!(feedback.active().unwrap().message, "Limited");
        assert_eq!(
            feedback.push(
                ToastPriority::Critical,
                "limit",
                Toast::warning("Limited").once_per_content(),
                now,
            ),
            ToastPushOutcome::RateLimited
        );
    }

    #[test]
    fn press_and_release_must_hit_the_same_action_chip() {
        let now = Instant::now();
        let mut feedback = Feedback::default();
        feedback.push(
            ToastPriority::Action,
            "confirm",
            Toast::warning("Delete?")
                .action("Delete", Action::PageDelete)
                .secondary_action("Board", Action::BoardDelete),
            now,
        );
        feedback.set_geometry(
            Some((0.0, 0.0, 200.0, 50.0)),
            [
                Some((80.0, 0.0, 50.0, 50.0)),
                Some((140.0, 0.0, 50.0, 50.0)),
            ],
        );
        let pressed = feedback.press_at(100, 25).expect("primary press");

        assert_eq!(feedback.release_at(pressed, 160, 25, now), (false, None));
        assert!(feedback.active().is_some());
        assert_eq!(
            feedback.release_at(pressed, 100, 25, now),
            (true, Some(ToastCommand::Dispatch(Action::PageDelete)))
        );
        assert!(feedback.active().is_none());
    }

    #[test]
    fn advance_promotes_the_next_toast_and_clears_geometry() {
        let now = Instant::now();
        let mut feedback = Feedback::default();
        feedback.push(
            ToastPriority::Info,
            "first",
            Toast::info("First").duration_ms(10),
            now,
        );
        feedback.push(ToastPriority::Info, "second", Toast::info("Second"), now);
        feedback.set_geometry(Some((1.0, 2.0, 3.0, 4.0)), [None, None]);

        assert!(feedback.advance(now + Duration::from_millis(10)));
        assert_eq!(feedback.active().unwrap().message, "Second");
        assert!(feedback.geometry().is_none());
    }

    #[test]
    fn removing_a_queued_toast_preserves_active_hit_geometry() {
        let now = Instant::now();
        let mut feedback = Feedback::default();
        feedback.push(ToastPriority::Info, "active", Toast::info("Active"), now);
        feedback.push(ToastPriority::Info, "queued", Toast::info("Queued"), now);
        feedback.set_geometry(Some((1.0, 2.0, 3.0, 4.0)), [None, None]);

        assert!(!feedback.remove_matching(|key, _| key == "queued"));
        assert_eq!(feedback.geometry(), Some((1.0, 2.0, 3.0, 4.0)));
        assert_eq!(feedback.active().unwrap().key, "active");

        assert!(feedback.remove_matching(|key, _| key == "active"));
        assert!(feedback.geometry().is_none());
        assert!(feedback.active().is_none());
    }

    #[test]
    fn blocked_action_feedback_expires_at_its_duration() {
        let now = Instant::now();
        let mut feedback = Feedback::default();
        feedback.trigger_blocked_action(now);

        assert_eq!(
            feedback.blocked_action_progress(
                now + Duration::from_millis(BLOCKED_ACTION_DURATION_MS / 2)
            ),
            Some(0.5)
        );
        assert!(
            feedback.advance_blocked_action(
                now + Duration::from_millis(BLOCKED_ACTION_DURATION_MS - 1)
            )
        );
        assert!(
            !feedback
                .advance_blocked_action(now + Duration::from_millis(BLOCKED_ACTION_DURATION_MS))
        );
        assert!(feedback.blocked_action_progress(now).is_none());
    }

    #[test]
    fn capability_toast_fires_once_per_capability_change() {
        let mut feedback = Feedback::default();
        let available = CompositorCapabilities {
            layer_shell: true,
            screencopy: true,
            freeze_capture: true,
            pointer_constraints: true,
            ..CompositorCapabilities::default()
        };
        let limited = CompositorCapabilities {
            freeze_capture: false,
            ..available
        };

        assert!(feedback.note_capability_toast(limited).is_some());
        assert!(feedback.note_capability_toast(limited).is_none());
        assert!(
            feedback.note_capability_toast(available).is_none(),
            "an available capability set records the change without warning"
        );
        assert!(feedback.note_capability_toast(limited).is_some());
    }
}
