use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

const PRODUCTION_BLOCKING_JOB_LIMIT: usize = 2;
const SLOW_JOB_THRESHOLD: Duration = Duration::from_millis(250);

static PRODUCTION_RUNNER: LazyLock<BlockingJobRunner> =
    LazyLock::new(|| BlockingJobRunner::new(PRODUCTION_BLOCKING_JOB_LIMIT));

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockingJobKind {
    ConfigLoad,
    ConfigSave,
    DaemonStatus,
    DaemonAction,
    SessionCatalogLoad,
    SessionCatalogMutation,
}

impl fmt::Display for BlockingJobKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::ConfigLoad => "config load",
            Self::ConfigSave => "config save",
            Self::DaemonStatus => "daemon status",
            Self::DaemonAction => "daemon action",
            Self::SessionCatalogLoad => "session catalog load",
            Self::SessionCatalogMutation => "session catalog mutation",
        };
        formatter.write_str(label)
    }
}

#[derive(Clone)]
struct BlockingJobRunner {
    permits: Arc<Semaphore>,
}

impl BlockingJobRunner {
    fn new(max_concurrent_jobs: usize) -> Self {
        assert!(
            max_concurrent_jobs > 0,
            "blocking job runner needs at least one permit"
        );
        Self {
            permits: Arc::new(Semaphore::new(max_concurrent_jobs)),
        }
    }

    async fn run<T, F>(&self, kind: BlockingJobKind, job: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, String> + Send + 'static,
    {
        let queued_at = Instant::now();
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| format!("{kind} blocking job queue is unavailable"))?;
        report_slow_phase(kind, "queue wait", queued_at.elapsed());

        let joined = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let started_at = Instant::now();
            let outcome = catch_unwind(AssertUnwindSafe(job));
            report_slow_phase(kind, "execution", started_at.elapsed());
            outcome
        })
        .await;

        let outcome =
            joined.map_err(|err| format!("{kind} blocking job did not complete: {err}"))?;

        match outcome {
            Ok(result) => result,
            Err(_) => Err(format!("{kind} blocking job panicked")),
        }
    }
}

pub(super) async fn run_blocking<T, F>(kind: BlockingJobKind, job: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    PRODUCTION_RUNNER.run(kind, job).await
}

fn report_slow_phase(kind: BlockingJobKind, phase: &str, elapsed: Duration) {
    if elapsed >= SLOW_JOB_THRESHOLD {
        eprintln!(
            "wayscriber configurator: slow {kind} blocking job {phase}: {:.0} ms",
            elapsed.as_secs_f64() * 1_000.0
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;

    use tokio::time::{Duration, timeout};

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn success_and_failure_pass_through_unchanged() {
        let runner = BlockingJobRunner::new(1);

        assert_eq!(
            runner
                .run(BlockingJobKind::ConfigLoad, || Ok::<_, String>(42))
                .await,
            Ok(42)
        );
        assert_eq!(
            runner
                .run::<(), _>(BlockingJobKind::ConfigSave, || Err(
                    "write failed".to_string()
                ))
                .await,
            Err("write failed".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn blocking_work_is_offloaded_while_executor_work_progresses() {
        let runner = BlockingJobRunner::new(1);
        let caller_thread = thread::current().id();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let job = tokio::spawn(async move {
            runner
                .run(BlockingJobKind::DaemonStatus, move || {
                    started_tx.send(thread::current().id()).ok();
                    release_rx.recv().map_err(|err| err.to_string())?;
                    Ok(())
                })
                .await
        });

        let blocking_thread = timeout(Duration::from_secs(1), started_rx)
            .await
            .expect("blocking closure should start")
            .expect("blocking closure should report its thread");
        assert_ne!(blocking_thread, caller_thread);

        let unrelated = tokio::spawn(async { 17 });
        assert_eq!(
            timeout(Duration::from_secs(1), unrelated)
                .await
                .expect("unrelated executor work should progress")
                .expect("unrelated executor task should complete"),
            17
        );

        release_tx.send(()).unwrap();
        assert_eq!(job.await.unwrap(), Ok(()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn one_permit_prevents_a_second_closure_from_starting() {
        let runner = BlockingJobRunner::new(1);
        let (first_started_tx, first_started_rx) = tokio::sync::oneshot::channel();
        let (first_release_tx, first_release_rx) = mpsc::channel();
        let first_runner = runner.clone();
        let first = tokio::spawn(async move {
            first_runner
                .run(BlockingJobKind::SessionCatalogMutation, move || {
                    first_started_tx.send(()).ok();
                    first_release_rx.recv().map_err(|err| err.to_string())?;
                    Ok(())
                })
                .await
        });
        timeout(Duration::from_secs(1), first_started_rx)
            .await
            .expect("first closure should start")
            .expect("first closure should report startup");

        let (second_started_tx, mut second_started_rx) = tokio::sync::oneshot::channel();
        let second = tokio::spawn(async move {
            runner
                .run(BlockingJobKind::SessionCatalogLoad, move || {
                    second_started_tx.send(()).ok();
                    Ok(())
                })
                .await
        });

        assert!(
            timeout(Duration::from_millis(50), &mut second_started_rx)
                .await
                .is_err(),
            "second closure entered while the sole permit was held"
        );

        first_release_tx.send(()).unwrap();
        timeout(Duration::from_secs(1), &mut second_started_rx)
            .await
            .expect("second closure should start after permit release")
            .expect("second closure should report startup");
        assert_eq!(first.await.unwrap(), Ok(()));
        assert_eq!(second.await.unwrap(), Ok(()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn panic_becomes_an_error_and_releases_the_permit() {
        let runner = BlockingJobRunner::new(1);
        let error = runner
            .run::<(), _>(BlockingJobKind::DaemonAction, || {
                panic!("synthetic blocking job panic")
            })
            .await
            .unwrap_err();
        assert!(error.contains("daemon action"));
        assert!(error.contains("panicked"));

        assert_eq!(
            timeout(
                Duration::from_secs(1),
                runner.run(BlockingJobKind::ConfigLoad, || Ok::<_, String>("recovered"))
            )
            .await
            .expect("permit should be released after panic"),
            Ok("recovered")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropping_the_waiter_does_not_release_a_running_jobs_permit() {
        let runner = BlockingJobRunner::new(1);
        let (first_started_tx, first_started_rx) = tokio::sync::oneshot::channel();
        let (first_release_tx, first_release_rx) = mpsc::channel();
        let first_runner = runner.clone();
        let first = tokio::spawn(async move {
            first_runner
                .run(BlockingJobKind::SessionCatalogMutation, move || {
                    first_started_tx.send(()).ok();
                    first_release_rx.recv().map_err(|err| err.to_string())?;
                    Ok(())
                })
                .await
        });
        timeout(Duration::from_secs(1), first_started_rx)
            .await
            .expect("first closure should start")
            .expect("first closure should report startup");

        first.abort();
        assert!(
            first
                .await
                .expect_err("the async waiter should be cancelled")
                .is_cancelled()
        );
        assert_eq!(
            runner.permits.available_permits(),
            0,
            "the detached blocking closure must retain its permit"
        );

        let (second_started_tx, mut second_started_rx) = tokio::sync::oneshot::channel();
        let second = tokio::spawn(async move {
            runner
                .run(BlockingJobKind::SessionCatalogLoad, move || {
                    second_started_tx.send(()).ok();
                    Ok(())
                })
                .await
        });
        assert!(
            timeout(Duration::from_millis(50), &mut second_started_rx)
                .await
                .is_err(),
            "cancelling the waiter released the running closure's permit"
        );

        first_release_tx.send(()).unwrap();
        timeout(Duration::from_secs(1), &mut second_started_rx)
            .await
            .expect("second closure should start after detached work finishes")
            .expect("second closure should report startup");
        assert_eq!(second.await.unwrap(), Ok(()));
    }

    #[test]
    fn production_concurrency_is_bounded_to_two_jobs() {
        assert_eq!(PRODUCTION_BLOCKING_JOB_LIMIT, 2);
    }
}
