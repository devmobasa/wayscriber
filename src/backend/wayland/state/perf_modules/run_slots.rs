use std::collections::BTreeSet;
use std::time::Instant;

use log::info;

/// Bounded measurements at buffer acquisition and submission boundaries.
/// A compositor release is reflected on the next slot observation.
#[derive(Debug)]
pub(super) struct RunSlotStats {
    generation: Option<u64>,
    generations_seen: u64,
    painted_in_generation: BTreeSet<usize>,
    painted_total: u64,
    max_painted_in_generation: usize,
    peak_in_flight: usize,
    deferral_started: Option<Instant>,
    deferral_episodes: u64,
    deferral_total_us: u64,
    deferral_max_us: u64,
}

impl RunSlotStats {
    pub(super) fn new() -> Self {
        Self {
            generation: None,
            generations_seen: 0,
            painted_in_generation: BTreeSet::new(),
            painted_total: 0,
            max_painted_in_generation: 0,
            peak_in_flight: 0,
            deferral_started: None,
            deferral_episodes: 0,
            deferral_total_us: 0,
            deferral_max_us: 0,
        }
    }

    pub(super) fn defer(&mut self, now: Instant) {
        if self.deferral_started.is_none() {
            self.deferral_started = Some(now);
        }
    }

    pub(super) fn submit(
        &mut self,
        generation: u64,
        canvas_ptr: usize,
        in_flight: usize,
        now: Instant,
    ) {
        if self.generation != Some(generation) {
            self.generation = Some(generation);
            self.generations_seen += 1;
            self.painted_in_generation.clear();
        }
        if self.painted_in_generation.insert(canvas_ptr) {
            self.painted_total += 1;
            self.max_painted_in_generation = self
                .max_painted_in_generation
                .max(self.painted_in_generation.len());
        }
        self.peak_in_flight = self.peak_in_flight.max(in_flight);

        if let Some(started) = self.deferral_started.take() {
            let micros = now.saturating_duration_since(started).as_micros();
            let micros = micros.min(u128::from(u64::MAX)) as u64;
            self.deferral_episodes += 1;
            self.deferral_total_us = self.deferral_total_us.saturating_add(micros);
            self.deferral_max_us = self.deferral_max_us.max(micros);
        }
    }

    pub(super) fn log_final(&self, now: Instant) {
        let pending_us = self.deferral_started.map(|started| {
            now.saturating_duration_since(started)
                .as_micros()
                .min(u128::from(u64::MAX)) as u64
        });
        info!(
            "perf.run_buffer_slots run_pid={} generations_seen={} painted_slots_total={} max_painted_per_generation={} peak_in_flight={} deferral_episodes={} deferral_total_us={} deferral_max_us={} pending_deferral_us={:?}",
            std::process::id(),
            self.generations_seen,
            self.painted_total,
            self.max_painted_in_generation,
            self.peak_in_flight,
            self.deferral_episodes,
            self.deferral_total_us,
            self.deferral_max_us,
            pending_us
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn repeated_failed_acquires_form_one_episode_until_submission() {
        let start = Instant::now();
        let mut stats = RunSlotStats::new();
        stats.defer(start);
        stats.defer(start + Duration::from_millis(2));
        stats.submit(1, 10, 2, start + Duration::from_millis(5));

        assert_eq!(stats.deferral_episodes, 1);
        assert_eq!(stats.deferral_total_us, 5_000);
        assert_eq!(stats.deferral_max_us, 5_000);
        assert!(stats.deferral_started.is_none());
    }

    #[test]
    fn slot_identity_resets_per_generation_without_growing_history() {
        let now = Instant::now();
        let mut stats = RunSlotStats::new();
        stats.submit(1, 10, 1, now);
        stats.submit(1, 10, 1, now);
        stats.submit(1, 20, 2, now);
        stats.submit(2, 10, 1, now);

        assert_eq!(stats.generations_seen, 2);
        assert_eq!(stats.painted_total, 3);
        assert_eq!(stats.max_painted_in_generation, 2);
        assert_eq!(stats.peak_in_flight, 2);
        assert_eq!(stats.painted_in_generation.len(), 1);
    }
}
