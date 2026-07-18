//! Ordering barrier between GTK input suppression and popup quiescence.

use std::future::Future;

pub(in crate::toolbar_gtk::view) async fn commit_input_regions_before_popup_quiescence<
    Disable,
    Prove,
    ProveFuture,
    Fence,
>(
    generation: u64,
    disable_input: Disable,
    prove_commit: Prove,
    fence: Fence,
) -> Result<(), String>
where
    Disable: FnOnce(),
    Prove: FnOnce() -> ProveFuture,
    ProveFuture: Future<Output = Result<(), String>>,
    Fence: FnOnce(),
{
    disable_input();
    log::info!("capture.preflight id={generation} component=gtk phase=input-disabled");

    // GDK stores input-region changes until the surface's next commit. Force
    // and presentation-confirm a fresh transparent frame before roundtripping
    // the Wayland connection or beginning the popup quiet period.
    prove_commit().await?;
    log::info!(
        "capture.preflight id={generation} component=gtk phase=input-region-commit-confirmed"
    );

    fence();
    log::info!("capture.preflight id={generation} component=gtk phase=input-fence-complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::future::{self, Future};
    use std::rc::Rc;
    use std::task::{Context, Poll, Waker};

    use super::*;

    #[test]
    fn presented_input_region_commit_precedes_the_wayland_roundtrip() {
        let phases = Rc::new(RefCell::new(Vec::new()));
        let disabled_phases = Rc::clone(&phases);
        let committed_phases = Rc::clone(&phases);
        let fenced_phases = Rc::clone(&phases);

        poll_ready(commit_input_regions_before_popup_quiescence(
            17,
            move || disabled_phases.borrow_mut().push("input-disabled"),
            move || {
                committed_phases.borrow_mut().push("input-region-presented");
                future::ready(Ok(()))
            },
            move || fenced_phases.borrow_mut().push("display-roundtrip"),
        ))
        .expect("input-region fence succeeds");

        assert_eq!(
            phases.borrow().as_slice(),
            [
                "input-disabled",
                "input-region-presented",
                "display-roundtrip"
            ]
        );
    }

    #[test]
    fn failed_input_region_commit_never_reaches_the_wayland_roundtrip() {
        let phases = Rc::new(RefCell::new(Vec::new()));
        let disabled_phases = Rc::clone(&phases);
        let committed_phases = Rc::clone(&phases);
        let fenced_phases = Rc::clone(&phases);

        let error = poll_ready(commit_input_regions_before_popup_quiescence(
            18,
            move || disabled_phases.borrow_mut().push("input-disabled"),
            move || {
                committed_phases
                    .borrow_mut()
                    .push("input-region-unconfirmed");
                future::ready(Err("input region was not presented".to_string()))
            },
            move || fenced_phases.borrow_mut().push("display-roundtrip"),
        ))
        .expect_err("an unconfirmed input region must fail closed");

        assert_eq!(error, "input region was not presented");
        assert_eq!(
            phases.borrow().as_slice(),
            ["input-disabled", "input-region-unconfirmed"]
        );
    }

    fn poll_ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("input-fence test future unexpectedly yielded"),
        }
    }
}
