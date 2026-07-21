//! Priority queue for UI toasts (M6).
//!
//! Replaces the old single-slot, last-writer-wins toast with a small queue:
//!
//! - Four priorities: `Critical` (freeze/capability/safety) > `Action`
//!   (toasts carrying a clickable chip) > `Info` > `Hint` (onboarding).
//! - A higher-priority push preempts the active toast; the preempted toast is
//!   re-queued at the front of its priority class when it is still fresh
//!   (enough remaining display time), otherwise dropped.
//! - Equal priority is FIFO.
//! - Pushes are deduplicated by key: a push with the key of the active or of a
//!   queued toast updates that toast in place instead of stacking a copy.
//! - Per-key rate limiting: a toast marked [`Toast::once_per_content`] is
//!   suppressed when the same key already showed the same message this
//!   session (the #156 capability-warning class: once per session unless the
//!   underlying state changes).
//! - Hints never block: a `Hint` push is only accepted when nothing is active
//!   and nothing is queued, and a preempted hint is dropped, never re-queued.
//!
//! The queue is pure logic over an external `Option<UiToastState>` active
//! slot (the field the renderer, damage tracker, and click handling already
//! consume), so it is unit-testable without an `InputState`.

use super::types::{ToastAction, UI_TOAST_DURATION_MS, UiToastKind, UiToastState};
use crate::domain::Action;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Display priority of a toast. Declaration order defines `Ord`:
/// `Hint < Info < Action < Critical`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToastPriority {
    /// Onboarding/discovery hints. Only shown when the queue is idle; never
    /// re-queued after preemption.
    Hint,
    /// Routine feedback (saves, copies, mode switches).
    Info,
    /// Toasts carrying a clickable action chip (undo-clear, retry, confirm).
    Action,
    /// Safety-critical notices (freeze/capture failures, capability limits).
    Critical,
}

/// Content of a toast push: everything except priority and key.
#[derive(Debug, Clone)]
pub struct Toast {
    pub kind: UiToastKind,
    pub message: String,
    pub duration_ms: u64,
    pub(crate) action: Option<ToastAction>,
    once_per_content: bool,
}

impl Toast {
    pub fn new(kind: UiToastKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            duration_ms: UI_TOAST_DURATION_MS,
            action: None,
            once_per_content: false,
        }
    }

    pub fn info(message: impl Into<String>) -> Self {
        Self::new(UiToastKind::Info, message)
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(UiToastKind::Warning, message)
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::new(UiToastKind::Error, message)
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn action(mut self, label: impl Into<String>, action: Action) -> Self {
        self.action = Some(ToastAction {
            label: label.into(),
            action,
        });
        self
    }

    /// Rate-limit this key: suppress the push when the same key already
    /// showed the same message this session. Content changes show again.
    pub fn once_per_content(mut self) -> Self {
        self.once_per_content = true;
        self
    }
}

/// What a [`ToastQueue::push`] did with the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastPushOutcome {
    /// The toast became the active (visible) toast immediately.
    Displayed,
    /// The toast was queued behind the active toast.
    Queued,
    /// The active toast had the same key and was updated in place.
    UpdatedActive,
    /// A queued toast had the same key and was updated in place.
    UpdatedQueued,
    /// Suppressed by the once-per-content rate limit.
    RateLimited,
    /// A hint was rejected because something else is active or queued.
    HintYielded,
}

impl ToastPushOutcome {
    /// Whether the push will (eventually) be visible.
    pub fn accepted(self) -> bool {
        !matches!(self, Self::RateLimited | Self::HintYielded)
    }

    /// Whether the push changed the currently visible toast.
    pub fn changed_active(self) -> bool {
        matches!(self, Self::Displayed | Self::UpdatedActive)
    }
}

#[derive(Debug, Clone)]
struct PendingToast {
    priority: ToastPriority,
    key: &'static str,
    toast: Toast,
    seq: u64,
}

/// Pending toasts plus per-key shown-content memory. The active slot lives
/// outside (on `InputState`) so rendering/damage/click code is untouched.
#[derive(Debug, Default)]
pub struct ToastQueue {
    /// Sorted by priority descending, FIFO (seq ascending) within a class.
    pending: Vec<PendingToast>,
    /// Last message shown per key this session (rate-limit memory).
    shown_contents: HashMap<&'static str, String>,
    seq: u64,
    activation_seq: u64,
}

/// Pending toasts beyond this are dropped (oldest of the lowest class first).
const MAX_PENDING: usize = 8;

/// A preempted toast is only re-queued when at least this much of its display
/// time remains; staler toasts are dropped.
const REQUEUE_MIN_REMAINING_MS: u64 = 750;

impl ToastQueue {
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Push a toast. Mutates `active` in place when the push displays or
    /// updates the visible toast.
    pub(crate) fn push(
        &mut self,
        active: &mut Option<UiToastState>,
        priority: ToastPriority,
        key: &'static str,
        toast: Toast,
        now: Instant,
    ) -> ToastPushOutcome {
        if toast.once_per_content && self.shown_contents.get(key) == Some(&toast.message) {
            return ToastPushOutcome::RateLimited;
        }

        // Dedup by key: update in place instead of stacking.
        if active.as_ref().is_some_and(|current| current.key == key) {
            *active = Some(self.activate_toast(priority, key, toast, now));
            self.record_shown(key, active.as_ref().map(|t| t.message.clone()));
            return ToastPushOutcome::UpdatedActive;
        }
        if let Some(index) = self.pending.iter().position(|entry| entry.key == key) {
            let priority_changed = self.pending[index].priority != priority;
            self.pending[index].priority = priority;
            self.pending[index].toast = toast;
            if priority_changed {
                self.pending
                    .sort_by(|a, b| b.priority.cmp(&a.priority).then(a.seq.cmp(&b.seq)));
            }

            // A same-key update can promote a queued informational result into
            // a critical failure. Re-run the normal preemption rule after the
            // in-place update instead of leaving that critical update hidden
            // behind a lower-priority active toast.
            if active
                .as_ref()
                .is_some_and(|current| priority > current.priority)
            {
                self.requeue_preempted(active.take().expect("checked above"), now);
                self.activate_next(active, now);
                debug_assert!(active.as_ref().is_some_and(|current| current.key == key));
                return ToastPushOutcome::Displayed;
            }
            return ToastPushOutcome::UpdatedQueued;
        }

        // Hints yield: only shown when nothing is active and nothing queued.
        if priority == ToastPriority::Hint && (active.is_some() || !self.pending.is_empty()) {
            return ToastPushOutcome::HintYielded;
        }

        match active {
            None => {
                self.enqueue_fifo(priority, key, toast);
                self.activate_next(active, now);
                if active.as_ref().is_some_and(|current| current.key == key) {
                    ToastPushOutcome::Displayed
                } else {
                    ToastPushOutcome::Queued
                }
            }
            Some(current) if priority > current.priority => {
                // Preempt: the active toast yields and is re-queued while
                // fresh (hints are dropped outright).
                self.requeue_preempted(active.take().expect("checked above"), now);
                self.enqueue_fifo(priority, key, toast);
                self.activate_next(active, now);
                debug_assert!(active.as_ref().is_some_and(|current| current.key == key));
                ToastPushOutcome::Displayed
            }
            Some(_) => {
                self.enqueue_fifo(priority, key, toast);
                ToastPushOutcome::Queued
            }
        }
    }

    /// Advance timers: expire the active toast and promote the next pending
    /// one. Returns `(still_showing, activated_new)`.
    pub(crate) fn advance(
        &mut self,
        active: &mut Option<UiToastState>,
        now: Instant,
    ) -> (bool, bool) {
        if let Some(current) = active.as_ref() {
            let duration = Duration::from_millis(current.duration_ms);
            if now.saturating_duration_since(current.started) >= duration {
                *active = None;
            }
        }
        let mut activated = false;
        if active.is_none() && !self.pending.is_empty() {
            self.activate_next(active, now);
            activated = active.is_some();
        }
        (active.is_some(), activated)
    }

    /// The active toast was dismissed (e.g. clicked); promote the next one.
    /// Returns whether a new toast was activated.
    pub(crate) fn on_dismissed(&mut self, active: &mut Option<UiToastState>, now: Instant) -> bool {
        *active = None;
        self.activate_next(active, now);
        active.is_some()
    }

    /// Retract every toast — the active one and every pending one — that
    /// `should_remove(key, action)` selects, so the next entry the queue
    /// promotes is guaranteed to be a valid (non-retracted) one.
    ///
    /// Cleanup paths (session-snapshot apply / full replacement) call this to
    /// drop toasts whose backing state was just discarded. Unlike clearing the
    /// active slot directly, it scans BOTH the active slot and the pending
    /// queue, so a queued confirm/undo toast can never surface later against a
    /// different session state. It deliberately does *not* eagerly promote a
    /// surviving pending toast: that is left to the normal
    /// [`Self::advance`] cycle, so an unrelated queued toast is not surfaced a
    /// frame early (and stays dormant exactly as it did before this scan).
    ///
    /// Returns whether the active (visible) toast was retracted.
    pub(crate) fn remove_matching(
        &mut self,
        active: &mut Option<UiToastState>,
        mut should_remove: impl FnMut(&'static str, Option<Action>) -> bool,
    ) -> bool {
        let active_removed = active.as_ref().is_some_and(|toast| {
            should_remove(toast.key, toast.action.as_ref().map(|entry| entry.action))
        });
        if active_removed {
            *active = None;
        }
        self.pending.retain(|entry| {
            !should_remove(entry.key, entry.toast.action.as_ref().map(|a| a.action))
        });
        active_removed
    }

    fn activate_toast(
        &mut self,
        priority: ToastPriority,
        key: &'static str,
        toast: Toast,
        now: Instant,
    ) -> UiToastState {
        self.activation_seq = self.activation_seq.wrapping_add(1);
        UiToastState {
            kind: toast.kind,
            message: toast.message,
            started: now,
            duration_ms: toast.duration_ms,
            action: toast.action,
            priority,
            key,
            activation_id: self.activation_seq,
        }
    }

    fn activate_next(&mut self, active: &mut Option<UiToastState>, now: Instant) {
        debug_assert!(active.is_none());
        if self.pending.is_empty() {
            return;
        }
        let entry = self.pending.remove(0);
        self.record_shown(entry.key, Some(entry.toast.message.clone()));
        *active = Some(self.activate_toast(entry.priority, entry.key, entry.toast, now));
    }

    fn record_shown(&mut self, key: &'static str, message: Option<String>) {
        if let Some(message) = message {
            self.shown_contents.insert(key, message);
        }
    }

    /// FIFO within the priority class: insert after every entry of equal or
    /// higher priority.
    fn enqueue_fifo(&mut self, priority: ToastPriority, key: &'static str, toast: Toast) {
        let seq = self.next_seq();
        let position = self
            .pending
            .iter()
            .position(|entry| entry.priority < priority)
            .unwrap_or(self.pending.len());
        self.pending.insert(
            position,
            PendingToast {
                priority,
                key,
                toast,
                seq,
            },
        );
        self.enforce_capacity();
    }

    /// Re-queue a preempted active toast at the *front* of its priority class
    /// (it was already showing, so it keeps seniority over queued peers) with
    /// its remaining display time. Stale toasts and hints are dropped.
    fn requeue_preempted(&mut self, current: UiToastState, now: Instant) {
        if current.priority == ToastPriority::Hint {
            return;
        }
        let elapsed = now.saturating_duration_since(current.started);
        let remaining = Duration::from_millis(current.duration_ms)
            .saturating_sub(elapsed)
            .as_millis() as u64;
        if remaining < REQUEUE_MIN_REMAINING_MS {
            return;
        }
        let seq = self.next_seq();
        let position = self
            .pending
            .iter()
            .position(|entry| entry.priority <= current.priority)
            .unwrap_or(self.pending.len());
        self.pending.insert(
            position,
            PendingToast {
                priority: current.priority,
                key: current.key,
                toast: Toast {
                    kind: current.kind,
                    message: current.message,
                    duration_ms: remaining,
                    action: current.action,
                    once_per_content: false,
                },
                seq,
            },
        );
        self.enforce_capacity();
    }

    fn enforce_capacity(&mut self) {
        while self.pending.len() > MAX_PENDING {
            // Drop the oldest entry (lowest seq) of the lowest priority class
            // present. Selecting by seq rather than position keeps a freshly
            // re-queued preempted toast — inserted at the *front* of its class
            // with a new (highest) seq to preserve its seniority — from being
            // evicted ahead of genuinely older queued peers.
            let Some(lowest) = self.pending.iter().map(|entry| entry.priority).min() else {
                return;
            };
            let victim = self
                .pending
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.priority == lowest)
                .min_by_key(|(_, entry)| entry.seq)
                .map(|(index, _)| index);
            match victim {
                Some(index) => {
                    self.pending.remove(index);
                }
                None => return,
            }
        }
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0_DURATION: u64 = UI_TOAST_DURATION_MS;

    fn active_key(active: &Option<UiToastState>) -> Option<&'static str> {
        active.as_ref().map(|toast| toast.key)
    }

    #[test]
    fn empty_queue_displays_immediately() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        let outcome = queue.push(
            &mut active,
            ToastPriority::Info,
            "a",
            Toast::info("hello"),
            now,
        );

        assert_eq!(outcome, ToastPushOutcome::Displayed);
        assert!(outcome.accepted());
        assert_eq!(active_key(&active), Some("a"));
        assert!(queue.is_empty());
    }

    #[test]
    fn equal_priority_is_fifo() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(&mut active, ToastPriority::Info, "a", Toast::info("a"), now);
        assert_eq!(
            queue.push(&mut active, ToastPriority::Info, "b", Toast::info("b"), now),
            ToastPushOutcome::Queued
        );
        assert_eq!(
            queue.push(&mut active, ToastPriority::Info, "c", Toast::info("c"), now),
            ToastPushOutcome::Queued
        );

        // Expire "a" -> "b" activates, then "c".
        let later = now + Duration::from_millis(T0_DURATION);
        assert_eq!(queue.advance(&mut active, later), (true, true));
        assert_eq!(active_key(&active), Some("b"));
        let later = later + Duration::from_millis(T0_DURATION);
        assert_eq!(queue.advance(&mut active, later), (true, true));
        assert_eq!(active_key(&active), Some("c"));
        let later = later + Duration::from_millis(T0_DURATION);
        assert_eq!(queue.advance(&mut active, later), (false, false));
        assert!(active.is_none());
    }

    #[test]
    fn higher_priority_preempts_and_requeues_fresh_active() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Info,
            "info",
            Toast::info("i"),
            now,
        );
        let outcome = queue.push(
            &mut active,
            ToastPriority::Critical,
            "crit",
            Toast::error("e"),
            now,
        );

        assert_eq!(outcome, ToastPushOutcome::Displayed);
        assert_eq!(active_key(&active), Some("crit"));
        assert_eq!(queue.pending_len(), 1, "preempted info toast re-queued");

        // When the critical toast expires the info toast resumes.
        let later = now + Duration::from_millis(T0_DURATION);
        assert_eq!(queue.advance(&mut active, later), (true, true));
        assert_eq!(active_key(&active), Some("info"));
    }

    #[test]
    fn preempted_toast_resumes_with_remaining_duration() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Info,
            "info",
            Toast::info("i"),
            now,
        );
        let preempt_at = now + Duration::from_millis(2000);
        queue.push(
            &mut active,
            ToastPriority::Critical,
            "crit",
            Toast::error("e"),
            preempt_at,
        );

        let crit_expiry = preempt_at + Duration::from_millis(T0_DURATION);
        queue.advance(&mut active, crit_expiry);
        let resumed = active.as_ref().expect("info resumed");
        assert_eq!(resumed.key, "info");
        assert_eq!(
            resumed.duration_ms,
            T0_DURATION - 2000,
            "resumes with the remaining display time, not a fresh timer"
        );
    }

    #[test]
    fn stale_active_is_dropped_instead_of_requeued() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Info,
            "info",
            Toast::info("i"),
            now,
        );
        // Preempt with less than REQUEUE_MIN_REMAINING_MS left.
        let preempt_at = now + Duration::from_millis(T0_DURATION - 200);
        queue.push(
            &mut active,
            ToastPriority::Critical,
            "crit",
            Toast::error("e"),
            preempt_at,
        );

        assert_eq!(active_key(&active), Some("crit"));
        assert!(queue.is_empty(), "stale info toast dropped, not re-queued");
    }

    #[test]
    fn preempted_toast_resumes_before_queued_peers() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Info,
            "showing",
            Toast::info("s"),
            now,
        );
        queue.push(
            &mut active,
            ToastPriority::Info,
            "waiting",
            Toast::info("w"),
            now,
        );
        queue.push(
            &mut active,
            ToastPriority::Critical,
            "crit",
            Toast::error("e"),
            now,
        );

        let later = now + Duration::from_millis(T0_DURATION);
        queue.advance(&mut active, later);
        assert_eq!(
            active_key(&active),
            Some("showing"),
            "preempted toast keeps seniority over queued peers"
        );
    }

    #[test]
    fn equal_or_lower_priority_never_preempts() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Action,
            "action",
            Toast::info("a").action("Undo?", Action::Undo),
            now,
        );
        assert_eq!(
            queue.push(
                &mut active,
                ToastPriority::Action,
                "b",
                Toast::info("b"),
                now
            ),
            ToastPushOutcome::Queued
        );
        assert_eq!(
            queue.push(&mut active, ToastPriority::Info, "c", Toast::info("c"), now),
            ToastPushOutcome::Queued
        );
        assert_eq!(active_key(&active), Some("action"));
    }

    #[test]
    fn same_key_updates_active_in_place() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Info,
            "board",
            Toast::info("Board 2"),
            now,
        );
        let later = now + Duration::from_millis(1000);
        let outcome = queue.push(
            &mut active,
            ToastPriority::Info,
            "board",
            Toast::info("Board 3"),
            later,
        );

        assert_eq!(outcome, ToastPushOutcome::UpdatedActive);
        assert!(outcome.changed_active());
        let toast = active.as_ref().expect("active toast");
        assert_eq!(toast.message, "Board 3");
        assert_eq!(toast.started, later, "update restarts the display timer");
        assert!(queue.is_empty(), "no stacking");
    }

    #[test]
    fn same_key_updates_queued_entry_in_place() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(&mut active, ToastPriority::Info, "a", Toast::info("a"), now);
        queue.push(
            &mut active,
            ToastPriority::Info,
            "page",
            Toast::info("Page 2"),
            now,
        );
        let outcome = queue.push(
            &mut active,
            ToastPriority::Info,
            "page",
            Toast::info("Page 5"),
            now,
        );

        assert_eq!(outcome, ToastPushOutcome::UpdatedQueued);
        assert_eq!(queue.pending_len(), 1, "updated, not stacked");
        let later = now + Duration::from_millis(T0_DURATION);
        queue.advance(&mut active, later);
        assert_eq!(active.as_ref().map(|t| t.message.as_str()), Some("Page 5"));
    }

    #[test]
    fn queued_same_key_priority_upgrade_preempts_lower_priority_active() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Action,
            "action",
            Toast::info("Undo available").action("Undo", Action::Undo),
            now,
        );
        queue.push(
            &mut active,
            ToastPriority::Info,
            "capture",
            Toast::info("Capture started"),
            now,
        );

        let outcome = queue.push(
            &mut active,
            ToastPriority::Critical,
            "capture",
            Toast::error("Capture failed"),
            now,
        );

        assert_eq!(outcome, ToastPushOutcome::Displayed);
        assert_eq!(active_key(&active), Some("capture"));
        assert_eq!(
            active.as_ref().map(|toast| toast.priority),
            Some(ToastPriority::Critical)
        );

        let later = now + Duration::from_millis(T0_DURATION);
        queue.advance(&mut active, later);
        assert_eq!(
            active_key(&active),
            Some("action"),
            "the preempted action toast keeps its remaining display time"
        );
    }

    #[test]
    fn once_per_content_shows_once_until_content_changes() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        let outcome = queue.push(
            &mut active,
            ToastPriority::Critical,
            "capability",
            Toast::warning("Freeze unavailable").once_per_content(),
            now,
        );
        assert_eq!(outcome, ToastPushOutcome::Displayed);

        // Same content again while showing: rate limited (would otherwise
        // restart the timer forever on per-tick producers).
        let outcome = queue.push(
            &mut active,
            ToastPriority::Critical,
            "capability",
            Toast::warning("Freeze unavailable").once_per_content(),
            now,
        );
        assert_eq!(outcome, ToastPushOutcome::RateLimited);
        assert!(!outcome.accepted());

        // Even after expiry and dismissal the same content stays suppressed.
        let later = now + Duration::from_millis(T0_DURATION);
        queue.advance(&mut active, later);
        assert!(active.is_none());
        let outcome = queue.push(
            &mut active,
            ToastPriority::Critical,
            "capability",
            Toast::warning("Freeze unavailable").once_per_content(),
            later,
        );
        assert_eq!(outcome, ToastPushOutcome::RateLimited);

        // State change -> new content -> shows again.
        let outcome = queue.push(
            &mut active,
            ToastPriority::Critical,
            "capability",
            Toast::warning("Freeze uses portal capture").once_per_content(),
            later,
        );
        assert_eq!(outcome, ToastPushOutcome::Displayed);
    }

    #[test]
    fn hints_yield_to_active_and_queued_toasts() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        // Idle queue: hint shows.
        let outcome = queue.push(
            &mut active,
            ToastPriority::Hint,
            "hint",
            Toast::info("Press F1"),
            now,
        );
        assert_eq!(outcome, ToastPushOutcome::Displayed);

        // Busy queue: hint rejected, never queued.
        let mut active =
            Some(queue.activate_toast(ToastPriority::Info, "busy", Toast::info("busy"), now));
        let outcome = queue.push(
            &mut active,
            ToastPriority::Hint,
            "hint2",
            Toast::info("Press F1"),
            now,
        );
        assert_eq!(outcome, ToastPushOutcome::HintYielded);
        assert!(queue.is_empty());
        assert_eq!(active_key(&active), Some("busy"));
    }

    #[test]
    fn hint_yields_when_only_pending_exists() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(&mut active, ToastPriority::Info, "a", Toast::info("a"), now);
        queue.push(&mut active, ToastPriority::Info, "b", Toast::info("b"), now);
        // Expire "a"; do not advance yet: active empty, pending non-empty.
        let later = now + Duration::from_millis(T0_DURATION);
        active = None;
        let outcome = queue.push(
            &mut active,
            ToastPriority::Hint,
            "hint",
            Toast::info("hint"),
            later,
        );
        assert_eq!(outcome, ToastPushOutcome::HintYielded);
    }

    #[test]
    fn preempted_hint_is_dropped_not_requeued() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Hint,
            "hint",
            Toast::info("hint"),
            now,
        );
        assert_eq!(active_key(&active), Some("hint"));

        queue.push(
            &mut active,
            ToastPriority::Info,
            "info",
            Toast::info("i"),
            now,
        );
        assert_eq!(active_key(&active), Some("info"));
        assert!(queue.is_empty(), "hint dropped, never re-queued");
    }

    #[test]
    fn dismissal_promotes_next_pending_toast() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(&mut active, ToastPriority::Info, "a", Toast::info("a"), now);
        queue.push(&mut active, ToastPriority::Info, "b", Toast::info("b"), now);

        assert!(queue.on_dismissed(&mut active, now));
        assert_eq!(active_key(&active), Some("b"));
        assert!(!queue.on_dismissed(&mut active, now));
        assert!(active.is_none());
    }

    #[test]
    fn capacity_drops_oldest_of_lowest_class() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(
            &mut active,
            ToastPriority::Critical,
            "active",
            Toast::error("x"),
            now,
        );
        let keys: [&'static str; 9] = ["p1", "p2", "p3", "p4", "p5", "p6", "p7", "p8", "p9"];
        for key in keys {
            queue.push(&mut active, ToastPriority::Info, key, Toast::info(key), now);
        }
        queue.push(
            &mut active,
            ToastPriority::Action,
            "act",
            Toast::info("act").action("Go", Action::Undo),
            now,
        );

        assert_eq!(queue.pending_len(), MAX_PENDING);
        // The action toast survived; the oldest info entries were dropped.
        let later = now + Duration::from_millis(T0_DURATION);
        queue.advance(&mut active, later);
        assert_eq!(active_key(&active), Some("act"));
    }

    #[test]
    fn requeued_preempted_toast_survives_capacity_eviction() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        // Active info toast plus a full pending queue of distinct info toasts.
        queue.push(
            &mut active,
            ToastPriority::Info,
            "i0",
            Toast::info("i0"),
            now,
        );
        let keys: [&'static str; MAX_PENDING] = ["i1", "i2", "i3", "i4", "i5", "i6", "i7", "i8"];
        for key in keys {
            queue.push(&mut active, ToastPriority::Info, key, Toast::info(key), now);
        }
        assert_eq!(active_key(&active), Some("i0"));
        assert_eq!(queue.pending_len(), MAX_PENDING);

        // A critical push preempts the fresh active i0. i0 is re-queued with
        // seniority; capacity eviction must drop a genuinely older queued peer,
        // never the just-preempted toast the user was actively watching.
        queue.push(
            &mut active,
            ToastPriority::Critical,
            "crit",
            Toast::error("e"),
            now,
        );
        assert_eq!(active_key(&active), Some("crit"));

        // When the critical toast expires the preempted i0 resumes first.
        let later = now + Duration::from_millis(T0_DURATION);
        queue.advance(&mut active, later);
        assert_eq!(
            active_key(&active),
            Some("i0"),
            "the freshly preempted senior toast survives capacity eviction"
        );
    }

    #[test]
    fn remove_matching_drops_queued_entry_and_leaves_active_untouched() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        // Active critical toast with a queued action toast behind it.
        queue.push(
            &mut active,
            ToastPriority::Critical,
            "crit",
            Toast::error("e"),
            now,
        );
        queue.push(
            &mut active,
            ToastPriority::Action,
            "board.delete",
            Toast::info("Board deleted").action("Undo", Action::BoardRestoreDeleted),
            now,
        );
        assert_eq!(active_key(&active), Some("crit"));
        assert_eq!(
            queue.pending_len(),
            1,
            "action toast queued behind critical"
        );

        // Retract the queued delete/restore toast; the active critical stays.
        let changed = queue.remove_matching(&mut active, |key, action| {
            key == "board.delete" && matches!(action, Some(Action::BoardRestoreDeleted))
        });
        assert!(!changed, "active slot did not change");
        assert_eq!(active_key(&active), Some("crit"));
        assert!(queue.is_empty(), "stale queued action removed");

        // When the critical toast expires nothing stale surfaces.
        let later = now + Duration::from_millis(T0_DURATION);
        assert_eq!(queue.advance(&mut active, later), (false, false));
        assert!(active.is_none());
    }

    #[test]
    fn remove_matching_retracts_active_and_leaves_survivors_for_advance() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        // Active delete/restore toast with an unrelated info toast queued.
        queue.push(
            &mut active,
            ToastPriority::Action,
            "board.delete",
            Toast::info("Board deleted").action("Undo", Action::BoardRestoreDeleted),
            now,
        );
        queue.push(
            &mut active,
            ToastPriority::Info,
            "info",
            Toast::info("i"),
            now,
        );
        assert_eq!(active_key(&active), Some("board.delete"));

        // Removing the active toast vacates the slot but does NOT eagerly
        // surface the unrelated survivor — that would pop an unrelated toast a
        // frame early. The survivor stays queued for the normal advance cycle.
        let changed = queue.remove_matching(&mut active, |key, _| key == "board.delete");
        assert!(changed, "active slot was retracted");
        assert!(active.is_none(), "no eager promotion");
        assert_eq!(queue.pending_len(), 1, "unrelated survivor stays queued");

        // The normal advance cycle then promotes the surviving valid entry.
        assert_eq!(queue.advance(&mut active, now), (true, true));
        assert_eq!(active_key(&active), Some("info"));
    }

    #[test]
    fn advance_reports_activation_of_pending_toast() {
        let mut queue = ToastQueue::default();
        let mut active = None;
        let now = Instant::now();

        queue.push(&mut active, ToastPriority::Info, "a", Toast::info("a"), now);
        // Mid-display: still showing, nothing new.
        assert_eq!(
            queue.advance(&mut active, now + Duration::from_millis(100)),
            (true, false)
        );
    }
}
