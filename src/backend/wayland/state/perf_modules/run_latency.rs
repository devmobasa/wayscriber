use std::time::{Duration, Instant};

use log::info;

// 100 microsecond buckets through one second. Collection allocates once when
// explicitly enabled and does not allocate or log for individual input events.
const BIN_WIDTH_US: u64 = 100;
const MAX_EXACT_US: u64 = 1_000_000;
const BIN_COUNT: usize = (MAX_EXACT_US / BIN_WIDTH_US) as usize;

#[derive(Debug)]
pub(super) struct RunLatencyHistogram {
    bins: Box<[u64]>,
    samples: u64,
    overflow: u64,
    max_us: u64,
    first_input: Option<Instant>,
    last_commit: Option<Instant>,
}

impl RunLatencyHistogram {
    pub(super) fn new() -> Self {
        Self {
            bins: vec![0; BIN_COUNT].into_boxed_slice(),
            samples: 0,
            overflow: 0,
            max_us: 0,
            first_input: None,
            last_commit: None,
        }
    }

    pub(super) fn begin(&mut self, received_at: Instant) {
        if self.first_input.is_none() {
            self.first_input = Some(received_at);
        }
    }

    pub(super) fn record(&mut self, latency: Duration, commit_at: Instant) {
        let micros = latency.as_micros().min(u128::from(u64::MAX)) as u64;
        self.samples = self.samples.saturating_add(1);
        self.max_us = self.max_us.max(micros);
        self.last_commit = Some(commit_at);

        if micros >= MAX_EXACT_US {
            self.overflow = self.overflow.saturating_add(1);
        } else {
            let index = (micros / BIN_WIDTH_US) as usize;
            self.bins[index] = self.bins[index].saturating_add(1);
        }
    }

    // Upper edge of the selected bucket; None means the requested tail fell
    // into the >= one-second overflow population and cannot be resolved.
    fn percentile_upper_us(&self, percentile: u64) -> Option<u64> {
        if self.samples == 0 {
            return None;
        }
        let rank = self.samples.saturating_mul(percentile).div_ceil(100);
        let mut count = 0;
        for (index, value) in self.bins.iter().enumerate() {
            count += value;
            if count >= rank {
                return Some((index as u64 + 1) * BIN_WIDTH_US);
            }
        }
        None
    }

    pub(super) fn log_final(&self, pending: usize, dropped: u64) {
        let elapsed_ms = self
            .first_input
            .zip(self.last_commit)
            .map(|(start, end)| end.saturating_duration_since(start).as_millis())
            .unwrap_or(0);
        info!(
            "perf.run_input_latency proxy=input_to_wayland_commit run_pid={} first_input_to_last_commit_ms={} samples={} pending={} dropped={} overflow_ge_1s={} bin_width_us={} p50_upper_us={:?} p95_upper_us={:?} p99_upper_us={:?} max_us={}",
            std::process::id(),
            elapsed_ms,
            self.samples,
            pending,
            dropped,
            self.overflow,
            BIN_WIDTH_US,
            self.percentile_upper_us(50),
            self.percentile_upper_us(95),
            self.percentile_upper_us(99),
            self.max_us
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_percentiles_cover_the_full_population_with_microsecond_resolution() {
        let now = Instant::now();
        let mut run = RunLatencyHistogram::new();
        run.begin(now);
        for micros in [50, 150, 250, 350, 450, 550, 650, 750, 850, 950] {
            run.record(
                Duration::from_micros(micros),
                now + Duration::from_millis(1),
            );
        }

        assert_eq!(run.samples, 10);
        assert_eq!(run.percentile_upper_us(50), Some(500));
        assert_eq!(run.percentile_upper_us(95), Some(1_000));
        assert_eq!(run.percentile_upper_us(99), Some(1_000));
        assert_eq!(run.max_us, 950);
    }

    #[test]
    fn overflow_is_reported_without_inventing_a_tail_percentile() {
        let mut run = RunLatencyHistogram::new();
        run.record(Duration::from_micros(100), Instant::now());
        run.record(Duration::from_secs(2), Instant::now());

        assert_eq!(run.overflow, 1);
        assert_eq!(run.percentile_upper_us(50), Some(200));
        assert_eq!(run.percentile_upper_us(99), None);
        assert_eq!(run.max_us, 2_000_000);
    }
}
