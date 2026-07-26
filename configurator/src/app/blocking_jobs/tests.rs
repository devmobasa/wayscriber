use super::*;

fn failed_active_job(
    id: BlockingJobId,
    purpose: BlockingJobPurpose,
    failure: BlockingJobTaskFailure,
) -> ActiveJob {
    let (sender, result) = mpsc::channel();
    sender
        .send(BlockingJobCompletion::Failed(failure))
        .expect("the fixture receiver remains alive");
    ActiveJob {
        id,
        purpose,
        status: ActiveJobStatus::Current,
        result,
    }
}

#[test]
fn production_admission_starts_two_jobs_and_queues_the_third() {
    let mut jobs = BlockingJobs::new(crate::test_temp::path_resolver());

    let first = jobs.submit(BlockingJobRequest::ConfigLoad);
    let second = jobs.submit(BlockingJobRequest::DaemonStatus {
        preserve_feedback: false,
    });
    let third = jobs.submit(BlockingJobRequest::SessionCatalogLoad);

    assert_eq!(first.started.units(), 1);
    assert_eq!(second.started.units(), 1);
    assert_eq!(third.started.units(), 0);
    assert_eq!(jobs.active.len(), PRODUCTION_BLOCKING_JOB_LIMIT.get());
    assert_eq!(jobs.pending.len(), 1);
}

#[test]
fn newest_request_cancels_running_and_queued_predecessors_without_freeing_the_slot() {
    let mut jobs = BlockingJobs::with_limit(NonZeroUsize::MIN, crate::test_temp::path_resolver());
    let running = jobs.submit(BlockingJobRequest::DaemonStatus {
        preserve_feedback: false,
    });
    let queued = jobs.submit(BlockingJobRequest::SessionCatalogLoad);
    let newest_queued = jobs.submit(BlockingJobRequest::SessionCatalogLoad);

    assert_eq!(
        newest_queued.cancellation,
        BlockingJobCancellation::Superseded {
            running: 0,
            queued: 1,
        }
    );
    assert_eq!(jobs.active.len(), 1);
    assert_eq!(jobs.pending.len(), 1);

    let newest_running = jobs.submit(BlockingJobRequest::DaemonStatus {
        preserve_feedback: true,
    });
    assert_eq!(
        newest_running.cancellation,
        BlockingJobCancellation::Superseded {
            running: 1,
            queued: 0,
        }
    );
    assert_eq!(jobs.active.len(), 1);
    assert_eq!(jobs.pending.len(), 2);

    let (completed_sender, completed_result) = mpsc::channel();
    completed_sender
        .send(BlockingJobCompletion::Completed(
            BlockingJobOutput::Fixture(7),
        ))
        .expect("the canceled active-job fixture keeps its receiver alive");
    jobs.active[0].result = completed_result;

    let update = jobs.handle_ready(running.id);
    assert!(matches!(
        update.transition,
        BlockingJobTransition::Canceled {
            purpose: BlockingJobPurpose::DaemonStatus {
                preserve_feedback: false
            }
        }
    ));
    assert_eq!(update.started.units(), 1);
    assert_eq!(jobs.active.len(), 1);
    assert_eq!(jobs.pending.len(), 1);
    assert_ne!(queued.id, newest_queued.id);
}

#[test]
fn task_failure_is_a_typed_terminal_transition_and_admits_the_next_job() {
    let mut jobs = BlockingJobs::with_limit(NonZeroUsize::MIN, crate::test_temp::path_resolver());
    let id = jobs.mint_job_id();
    jobs.active.push(failed_active_job(
        id.clone(),
        BlockingJobPurpose::ConfigLoad,
        BlockingJobTaskFailure::WorkerPanicked,
    ));
    let pending_id = jobs.mint_job_id();
    jobs.pending.push_back(QueuedJob {
        id: pending_id,
        request: BlockingJobRequest::SessionCatalogLoad,
        queued_at: Instant::now(),
    });

    let update = jobs.handle_ready(id);

    assert!(matches!(
        update.transition,
        BlockingJobTransition::Failed {
            purpose: BlockingJobPurpose::ConfigLoad,
            failure: BlockingJobTaskFailure::WorkerPanicked,
        }
    ));
    assert_eq!(update.started.units(), 1);
    assert_eq!(jobs.active.len(), 1);
    assert!(jobs.pending.is_empty());
}

#[test]
fn duplicate_or_foreign_notice_is_stale_and_does_not_change_admission() {
    let mut jobs = BlockingJobs::with_limit(NonZeroUsize::MIN, crate::test_temp::path_resolver());
    let foreign = BlockingJobId(vec![99]);

    let update = jobs.handle_ready(foreign.clone());

    assert!(matches!(
        update.transition,
        BlockingJobTransition::Stale { id } if id == foreign
    ));
    assert_eq!(update.started.units(), 0);
    assert!(jobs.active.is_empty());
}

#[test]
fn two_roots_mint_independent_sequences_and_cancel_only_their_own_jobs() {
    let mut first = BlockingJobs::with_limit(NonZeroUsize::MIN, crate::test_temp::path_resolver());
    let mut second = BlockingJobs::with_limit(NonZeroUsize::MIN, crate::test_temp::path_resolver());
    let first_job = first.submit(BlockingJobRequest::DaemonStatus {
        preserve_feedback: false,
    });
    let second_job = second.submit(BlockingJobRequest::DaemonStatus {
        preserve_feedback: false,
    });

    assert_eq!(first_job.id, second_job.id);
    assert_eq!(
        first.cancel_daemon_status(),
        BlockingJobCancellation::Superseded {
            running: 1,
            queued: 0,
        }
    );
    assert_eq!(first.active[0].status, ActiveJobStatus::Canceled);
    assert_eq!(second.active[0].status, ActiveJobStatus::Current);
}

#[test]
fn local_identity_expands_instead_of_wrapping_or_reusing_a_live_id() {
    let mut jobs = BlockingJobs::with_limit(NonZeroUsize::MIN, crate::test_temp::path_resolver());
    jobs.next_job_id = vec![u8::MAX];

    let last_one_byte_id = jobs.mint_job_id();
    let first_two_byte_id = jobs.mint_job_id();

    assert_eq!(last_one_byte_id, BlockingJobId(vec![u8::MAX]));
    assert_eq!(first_two_byte_id, BlockingJobId(vec![1, 0]));
}

#[tokio::test(flavor = "current_thread")]
async fn worker_result_is_available_before_the_ready_notice_is_returned() {
    let id = BlockingJobId(vec![42]);
    let (result_sender, result) = mpsc::channel();

    let completed_id = run_blocking_job(
        id.clone(),
        BlockingJobPurpose::ConfigLoad,
        BlockingJobRequest::FixtureOutput(9),
        crate::test_temp::path_resolver(),
        result_sender,
    )
    .await;

    assert_eq!(completed_id, id);
    assert!(matches!(
        result
            .try_recv()
            .expect("the worker sends its fixture result before returning the ready ID"),
        BlockingJobCompletion::Completed(BlockingJobOutput::Fixture(9))
    ));
}
