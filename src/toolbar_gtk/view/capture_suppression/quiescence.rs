//! Bounded GTK-main-context settling for asynchronously created popovers.

use std::future::Future;
use std::time::{Duration, Instant};

use super::CaptureProofTarget;

pub(super) const CAPTURE_BARRIER_TIMEOUT: Duration = Duration::from_secs(1);

// GtkText uses a 50 ms delayed touch-selection popup in the audited GTK
// implementations from 4.12 through 4.22. Once input admission is closed,
// require a full lower-priority quiet window so already-scheduled popup
// callbacks run before capture can be acknowledged.
const GTK_POPUP_QUIET_PERIOD: Duration = Duration::from_millis(50);

pub(super) async fn wait_for_popover_quiescence<Scan, Mark, Prove, ProveFuture>(
    generation: u64,
    mut scan: Scan,
    mut mark_proven: Mark,
    mut prove: Prove,
) -> Result<(), String>
where
    Scan: FnMut() -> Vec<CaptureProofTarget>,
    Mark: FnMut(),
    Prove: FnMut(Vec<CaptureProofTarget>, Instant) -> ProveFuture,
    ProveFuture: Future<Output = Result<(), String>>,
{
    let started = Instant::now();
    let deadline = started + CAPTURE_BARRIER_TIMEOUT;
    let mut rounds = 0usize;

    let settle = async {
        // Run only after all default-priority sources that are already ready.
        // They may include an input callback that schedules GtkText's delayed
        // selection popup, so start the quiet clock after this checkpoint.
        gtk4::glib::timeout_future_with_priority(gtk4::glib::Priority::LOW, Duration::ZERO).await;
        let mut quiet_until = Instant::now() + GTK_POPUP_QUIET_PERIOD;

        loop {
            let targets = scan();
            let now = Instant::now();

            if now >= deadline {
                return Err(quiescence_timeout(generation, started, rounds));
            }

            if !targets.is_empty() {
                rounds += 1;
                log::info!(
                    "capture.preflight id={generation} component=gtk phase=popup-quiescence-proof round={rounds} targets={}",
                    targets.len()
                );
                prove(targets, deadline).await?;
                mark_proven();
                // A proof wait yields the GTK main context. Require another
                // complete quiet window in case that work admitted a second
                // wave of already-scheduled native popups.
                quiet_until = Instant::now() + GTK_POPUP_QUIET_PERIOD;
                continue;
            }

            if now >= quiet_until {
                log::info!(
                    "capture.preflight id={generation} component=gtk phase=popup-quiescent rounds={rounds} elapsed_ms={}",
                    started.elapsed().as_millis()
                );
                return Ok(());
            }

            let checkpoint_at = std::cmp::min(quiet_until, deadline);
            gtk4::glib::timeout_future_with_priority(
                gtk4::glib::Priority::LOW,
                checkpoint_at.saturating_duration_since(now),
            )
            .await;
        }
    };

    match gtk4::glib::future_with_timeout(CAPTURE_BARRIER_TIMEOUT, settle).await {
        Ok(result) => result,
        Err(_) => Err(quiescence_timeout(generation, started, rounds)),
    }
}

fn quiescence_timeout(generation: u64, started: Instant, rounds: usize) -> String {
    format!(
        "GTK capture suppression generation {generation} did not reach popup quiescence after {} ms ({rounds} proof rounds)",
        started.elapsed().as_millis()
    )
}
