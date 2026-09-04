use std::time::{Duration, Instant};

use super::runtime::{UiDamageHistory, UiEffect};
use super::{RenderOutcome, render_acquired_frame};
use crate::backend::wayland::state::ui_animation::UiAnimationClock;
use crate::input::InputState;
use crate::input::state::test_support::make_test_input_state;
use crate::util::Rect;

struct FrameOwners {
    input: InputState,
    history: UiDamageHistory,
    clock: UiAnimationClock,
    now: Instant,
    previous: Rect,
    current: Rect,
    damage: Vec<Rect>,
    stages: Vec<&'static str>,
}

impl FrameOwners {
    fn new() -> Self {
        let mut input = make_test_input_state();
        input.update_screen_dimensions(800, 600);
        input.take_dirty_region_report();
        input.mark_selection_dirty_region(Rect::new(100, 100, 20, 20));
        let previous = Rect::new(20, 20, 10, 10).expect("old effect bounds");
        let current = Rect::new(60, 20, 10, 10).expect("new effect bounds");
        let mut history = UiDamageHistory::default();
        history.roll(UiEffect::ToolPreview, Some(previous), &mut Vec::new());
        let now = Instant::now();
        let mut clock = UiAnimationClock::from_fps(20);
        clock.schedule(now, true);
        Self {
            input,
            history,
            clock,
            now,
            previous,
            current,
            damage: Vec::new(),
            stages: Vec::new(),
        }
    }

    fn attempt(
        &mut self,
        acquired: Option<u8>,
        paint_result: anyhow::Result<()>,
        keep_rendering: bool,
    ) -> anyhow::Result<RenderOutcome> {
        // Exercise the production acquisition gate with real mutable owners.
        // The continuation stands in for preparation, painting and submission;
        // a busy acquisition must not enter any part of it.
        render_acquired_frame(acquired, |slot| {
            assert_eq!(slot, 7);
            self.stages.push("prepare");
            self.clock
                .schedule(self.now + Duration::from_millis(10), true);
            self.damage = self.input.take_dirty_region_report().regions;
            self.history
                .roll(UiEffect::ToolPreview, Some(self.current), &mut self.damage);
            self.stages.push("paint");
            paint_result?;
            self.stages.push("submit");
            Ok(keep_rendering)
        })
    }

    fn assert_prepared(&mut self) {
        assert!(self.input.take_dirty_region_report().regions.is_empty());
        assert_eq!(
            self.history.previous(UiEffect::ToolPreview),
            Some(self.current)
        );
        assert_eq!(
            self.clock.timeout(self.now),
            Some(Duration::from_millis(60))
        );
        assert!(self.damage.contains(&self.previous));
        assert!(self.damage.contains(&self.current));
        assert!(
            self.damage.len() > 2,
            "input damage was drained into the frame"
        );
    }
}

#[test]
fn busy_acquisition_preserves_pending_input_effect_history_and_animation_deadline() {
    let mut owners = FrameOwners::new();

    let outcome = owners.attempt(None, Ok(()), true).expect("deferred frame");

    assert_eq!(outcome, RenderOutcome::BuffersInFlight);
    assert!(
        owners.stages.is_empty(),
        "preparation, paint and submit are deferred"
    );
    assert!(owners.damage.is_empty());
    assert!(!owners.input.take_dirty_region_report().regions.is_empty());
    assert_eq!(
        owners.history.previous(UiEffect::ToolPreview),
        Some(owners.previous)
    );
    assert_eq!(
        owners.clock.timeout(owners.now),
        Some(Duration::from_millis(50))
    );
}

#[test]
fn acquired_frame_runs_the_continuation_and_preserves_redraw_outcome() {
    for keep_rendering in [false, true] {
        let mut owners = FrameOwners::new();

        let outcome = owners
            .attempt(Some(7), Ok(()), keep_rendering)
            .expect("committed frame");

        assert_eq!(outcome, RenderOutcome::Committed { keep_rendering });
        assert_eq!(owners.stages, ["prepare", "paint", "submit"]);
        owners.assert_prepared();
    }
}

#[test]
fn paint_failure_retains_preparation_mutations_without_reporting_a_commit() {
    let mut owners = FrameOwners::new();

    let error = owners
        .attempt(Some(7), Err(anyhow::anyhow!("paint failed")), true)
        .expect_err("paint failure must propagate");

    assert_eq!(error.to_string(), "paint failed");
    assert_eq!(owners.stages, ["prepare", "paint"]);
    owners.assert_prepared();
}
