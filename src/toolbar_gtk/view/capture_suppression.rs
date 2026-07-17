use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::prelude::*;

use super::{after_next_surface_paint_counter, widget_native_is_mapped};

const CAPTURE_PAINT_TIMEOUT: Duration = Duration::from_millis(500);
const CAPTURE_PAINT_POLL_INTERVAL: Duration = Duration::from_millis(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CapturePresentationState {
    Pending,
    Presented,
    CompositorFrame,
}

fn capture_presentation_state(timings: Option<(bool, i64)>) -> CapturePresentationState {
    match timings {
        Some((_, presentation_time)) if presentation_time > 0 => {
            CapturePresentationState::Presented
        }
        Some((true, _)) => CapturePresentationState::CompositorFrame,
        Some((false, _)) | None => CapturePresentationState::Pending,
    }
}

pub(super) async fn wait_for_presented_transparency(
    generation: u64,
    targets: Vec<(&'static str, gtk4::Widget)>,
) -> Result<(), String> {
    let started = Instant::now();
    log::info!(
        "capture.preflight id={generation} component=gtk phase=wait-mapped targets={}",
        targets
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(",")
    );
    while targets
        .iter()
        .any(|(_, widget)| !widget_native_is_mapped(widget))
    {
        if started.elapsed() >= CAPTURE_PAINT_TIMEOUT {
            let states = targets
                .iter()
                .map(|(name, widget)| format!("{name}_mapped={}", widget_native_is_mapped(widget)))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "GTK capture suppression generation {generation} timed out waiting for normally visible toolbar surfaces to map ({states})"
            ));
        }
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }

    let mut frame_clocks = Vec::with_capacity(targets.len());
    for (name, widget) in &targets {
        let frame_clock = widget.frame_clock().ok_or_else(|| {
            format!(
                "GTK capture suppression generation {generation} found no frame clock for the mapped {name} toolbar"
            )
        })?;
        frame_clocks.push(frame_clock);
    }

    let frame_counters = targets
        .iter()
        .map(|_| Rc::new(Cell::new(None)))
        .collect::<Vec<_>>();
    for ((name, widget), frame_counter) in targets.iter().zip(&frame_counters) {
        let name = *name;
        let frame_counter = Rc::clone(frame_counter);
        after_next_surface_paint_counter(widget, move |counter| {
            frame_counter.set(counter);
            log::info!(
                "capture.preflight id={generation} component=gtk surface={name} phase=painted frame_counter={counter:?} elapsed_ms={}",
                started.elapsed().as_millis()
            );
        });
    }
    wait_for_frame_counters(generation, "transparent-paint", started, &frame_counters).await?;

    let mut states = vec![CapturePresentationState::Pending; targets.len()];
    let presentation_started = Instant::now();
    loop {
        for (index, ((name, _), frame_clock)) in targets.iter().zip(&frame_clocks).enumerate() {
            if states[index] != CapturePresentationState::Pending {
                continue;
            }
            let frame_counter = frame_counters[index]
                .get()
                .expect("transparent paint counter recorded");
            let timings = frame_clock.timings(frame_counter);
            let next = capture_presentation_state(
                timings
                    .as_ref()
                    .map(|timings| (timings.is_complete(), timings.presentation_time())),
            );
            if next != CapturePresentationState::Pending {
                let complete = timings
                    .as_ref()
                    .is_some_and(|timings| timings.is_complete());
                let presentation_time = timings
                    .as_ref()
                    .map_or(0, |timings| timings.presentation_time());
                log::info!(
                    "capture.preflight id={generation} component=gtk surface={name} phase=presentation status={next:?} frame_counter={frame_counter} complete={complete} presentation_time_us={presentation_time} elapsed_ms={}",
                    started.elapsed().as_millis()
                );
                states[index] = next;
            }
        }

        if states
            .iter()
            .all(|state| *state != CapturePresentationState::Pending)
        {
            break;
        }
        if presentation_started.elapsed() >= CAPTURE_PAINT_TIMEOUT {
            let pending = targets
                .iter()
                .zip(&states)
                .filter_map(|((name, _), state)| {
                    (*state == CapturePresentationState::Pending).then_some(*name)
                })
                .collect::<Vec<_>>()
                .join(",");
            return Err(format!(
                "generation {generation} received no compositor frame or presentation feedback for {pending} after {} ms",
                presentation_started.elapsed().as_millis()
            ));
        }
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }

    log::info!(
        "capture.preflight id={generation} component=gtk phase=transparent-commit-confirmed elapsed_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

async fn wait_for_frame_counters(
    generation: u64,
    phase: &str,
    started: Instant,
    counters: &[Rc<Cell<Option<i64>>>],
) -> Result<(), String> {
    while counters.iter().any(|counter| counter.get().is_none()) {
        if started.elapsed() >= CAPTURE_PAINT_TIMEOUT {
            return Err(format!(
                "GTK capture suppression generation {generation} timed out during {phase} waiting for {} toolbar frame(s)",
                counters
                    .iter()
                    .filter(|counter| counter.get().is_none())
                    .count()
            ));
        }
        gtk4::glib::timeout_future(CAPTURE_PAINT_POLL_INTERVAL).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_paint_is_not_capture_safe_until_the_compositor_presents_it() {
        assert_eq!(
            capture_presentation_state(None),
            CapturePresentationState::Pending
        );
        assert_eq!(
            capture_presentation_state(Some((false, 0))),
            CapturePresentationState::Pending
        );
        assert_eq!(
            capture_presentation_state(Some((true, 0))),
            CapturePresentationState::CompositorFrame
        );
        assert_eq!(
            capture_presentation_state(Some((false, 42))),
            CapturePresentationState::Presented
        );
    }
}
