use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsRawFd;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::*;

#[test]
fn profiles_decode_only_their_owned_events() {
    assert_eq!(
        decode_signal(SignalProfile::Daemon, libc::SIGUSR1)
            .expect("daemon fixture decodes its registered toggle signal"),
        SignalEvent::ToggleOverlay
    );
    assert_eq!(
        decode_signal(SignalProfile::Overlay, libc::SIGUSR2)
            .expect("overlay fixture decodes its registered tray signal"),
        SignalEvent::TrayAction
    );
    assert_eq!(
        decode_signal(SignalProfile::Daemon, libc::SIGUSR2)
            .expect_err("daemon fixture rejects the overlay-only tray signal")
            .kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn independent_fake_sources_publish_without_process_global_coordination() {
    let mut first = FakeSignalSource::new().expect("fixture creates its first signal source");
    let mut second = FakeSignalSource::new().expect("fixture creates its second signal source");
    first
        .publish(SignalEvent::Shutdown(ShutdownSignal::Terminate))
        .expect("fixture publishes to its first source");
    second
        .publish(SignalEvent::TrayAction)
        .expect("fixture publishes to its second source");

    for source in [&first, &second] {
        let mut pollfd = libc::pollfd {
            fd: source
                .poll_fd()
                .expect("fixture source retains its poll descriptor")
                .as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe {
            // SAFETY: each fake retains its descriptor for this nonblocking poll.
            libc::poll(&mut pollfd, 1, 0)
        };
        assert_eq!(ready, 1);
        assert_ne!(pollfd.revents & libc::POLLIN, 0);
    }

    assert_eq!(
        first
            .drain()
            .expect("fixture drains its first signal source"),
        vec![SignalEvent::Shutdown(ShutdownSignal::Terminate)]
    );
    assert_eq!(
        second
            .drain()
            .expect("fixture drains its second signal source"),
        vec![SignalEvent::TrayAction]
    );
}

#[test]
fn tokio_workers_inherit_the_calling_threads_blocked_runtime_signals() {
    let profile = SignalProfile::Daemon;
    let mask = signal_mask(profile).expect("fixture constructs the daemon signal mask");
    let previous = block_signals(&mask).expect("fixture blocks daemon signals");
    let runtime = tokio::runtime::Runtime::new().expect("fixture creates its Tokio runtime");

    let worker_membership = runtime.block_on(async move {
        tokio::spawn(async move { selected_signal_membership(profile) })
            .await
            .expect("fixture worker completes")
            .expect("fixture worker can inspect its mask")
    });

    drop(runtime);
    restore_signals(&previous).expect("fixture restores its calling-thread signal mask");
    assert!(
        worker_membership.into_iter().all(|blocked| blocked),
        "Tokio worker did not inherit the blocked daemon signal mask"
    );
}

#[test]
fn real_owner_admission_rejects_before_mask_change_and_reopens_after_drop() -> TestResult {
    if std::env::var_os(REAL_ADMISSION_CHILD_ENV).is_some() {
        return run_real_owner_admission_probe();
    }

    let output = Command::new(std::env::current_exe()?)
        .arg("--exact")
        .arg("unix_signals::tests::real_owner_admission_rejects_before_mask_change_and_reopens_after_drop")
        .arg("--nocapture")
        .env(REAL_ADMISSION_CHILD_ENV, "1")
        .output()?;
    if output.status.success() {
        return Ok(());
    }

    Err(io::Error::other(format!(
        "isolated signal-admission probe failed ({}); stdout: {}; stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
    .into())
}

const REAL_ADMISSION_CHILD_ENV: &str = "WAYSCRIBER_REAL_SIGNAL_ADMISSION_CHILD";

fn run_real_owner_admission_probe() -> TestResult {
    let original_mask = selected_signal_membership(SignalProfile::Daemon)?;
    let first = SignalOwner::install(SignalProfile::Daemon)?;
    let second = std::thread::spawn(|| -> io::Result<(io::ErrorKind, bool)> {
        let before = selected_signal_membership(SignalProfile::Overlay)?;
        let rejection = match SignalOwner::install(SignalProfile::Overlay) {
            Ok(mut unexpected) => {
                let _ = unexpected.finish();
                return Err(io::Error::other(
                    "concurrent real signal owner unexpectedly acquired admission",
                ));
            }
            Err(error) => error,
        };
        let after = selected_signal_membership(SignalProfile::Overlay)?;
        Ok((rejection.kind(), after == before))
    })
    .join();

    drop(first);
    let restored_mask = selected_signal_membership(SignalProfile::Daemon)?;
    let (kind, mask_unchanged) =
        second.map_err(|_| io::Error::other("concurrent signal-admission probe panicked"))??;
    assert_eq!(kind, io::ErrorKind::AlreadyExists);
    assert!(
        mask_unchanged,
        "rejected signal admission changed the competing thread's mask"
    );
    assert_eq!(
        restored_mask, original_mask,
        "dropping the first owner did not restore its thread's mask"
    );

    let mut replacement = SignalOwner::install(SignalProfile::Overlay)?;
    replacement.finish()?;

    let retry_original_mask = selected_signal_membership(SignalProfile::Daemon)?;
    let mut retry_owner = SignalOwner::install(SignalProfile::Daemon)?;
    let injected = retry_owner
        .finish_with(|_| Err(io::Error::other("injected signal-mask restoration failure")));
    assert!(injected.is_err());
    assert!(matches!(
        retry_owner.poll_fd(),
        Err(error) if error.kind() == io::ErrorKind::NotConnected
    ));

    let retry_rejection = match SignalOwner::install(SignalProfile::Overlay) {
        Ok(mut unexpected) => {
            let _ = unexpected.finish();
            return Err(io::Error::other(
                "restore-pending owner unexpectedly released signal admission",
            )
            .into());
        }
        Err(error) => error,
    };
    assert_eq!(retry_rejection.kind(), io::ErrorKind::AlreadyExists);

    retry_owner.finish()?;
    retry_owner.finish()?;
    assert_eq!(
        selected_signal_membership(SignalProfile::Daemon)?,
        retry_original_mask
    );
    let mut retry_replacement = SignalOwner::install(SignalProfile::Overlay)?;
    retry_replacement.finish()?;
    Ok(())
}

#[test]
fn fake_source_reports_a_typed_terminal_read_failure() {
    let mut source = FakeSignalSource::new().expect("fixture creates its signal source");
    source
        .fail_next_drain(io::ErrorKind::BrokenPipe)
        .expect("fixture wakes its injected failure source");

    assert_eq!(
        source
            .drain()
            .expect_err("fixture observes its injected source failure")
            .kind(),
        io::ErrorKind::BrokenPipe
    );
}

const REAL_PROBE_CHILD_ENV: &str = "WAYSCRIBER_REAL_SIGNAL_PROBE_CHILD";
const REAL_PROBE_READY: &str = "WAYSCRIBER_SIGNAL_PROBE_READY=";

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn real_signal_owner_subprocess_probe() -> TestResult {
    if std::env::var_os(REAL_PROBE_CHILD_ENV).is_some() {
        return run_real_probe_child();
    }

    let executable = std::env::current_exe()?;
    let mut child = Command::new(executable)
        .arg("--exact")
        .arg("unix_signals::tests::real_signal_owner_subprocess_probe")
        .arg("--nocapture")
        .env(REAL_PROBE_CHILD_ENV, "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("signal probe child stdout was not piped"))?;
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = ready_tx.send(Err(
                        "signal probe child exited before publishing readiness".to_string(),
                    ));
                    return;
                }
                Ok(_) => {
                    if let Some(marker) = line.find(REAL_PROBE_READY) {
                        let raw_tid = line[marker + REAL_PROBE_READY.len()..].trim();
                        let result = raw_tid
                            .parse::<libc::pid_t>()
                            .map(|tid| (tid, reader))
                            .map_err(|error| format!("invalid signal probe tid: {error}"));
                        let _ = ready_tx.send(result);
                        return;
                    }
                }
                Err(error) => {
                    let _ = ready_tx.send(Err(format!(
                        "failed reading signal probe readiness: {error}"
                    )));
                    return;
                }
            }
        }
    });

    let (tid, mut stdout) = match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(ready)) => ready,
        Ok(Err(error)) => {
            let _ = child.kill();
            let _ = child.wait();
            reader
                .join()
                .expect("fixture readiness reader exits after child shutdown");
            return Err(io::Error::other(error).into());
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            reader
                .join()
                .expect("fixture readiness reader exits after timeout shutdown");
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("signal probe readiness timed out: {error}"),
            )
            .into());
        }
    };
    reader
        .join()
        .expect("fixture readiness reader handed ownership back to the test");

    let signal_result = unsafe {
        // SAFETY: the child published this live Linux thread id and tgkill
        // targets only that isolated fixture process/thread.
        libc::syscall(
            libc::SYS_tgkill,
            child.id() as libc::pid_t,
            tid,
            libc::SIGUSR2,
        )
    };
    if signal_result != 0 {
        let error = io::Error::last_os_error();
        let _ = child.kill();
        let _ = child.wait();
        return Err(error.into());
    }

    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "real signal probe child did not exit",
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut remaining_stdout = String::new();
    stdout.read_to_string(&mut remaining_stdout)?;
    let mut stderr = String::new();
    if let Some(mut child_stderr) = child.stderr.take() {
        child_stderr.read_to_string(&mut stderr)?;
    }
    if !status.success() {
        return Err(io::Error::other(format!(
            "real signal probe failed ({status}); stdout: {remaining_stdout}; stderr: {stderr}"
        ))
        .into());
    }
    Ok(())
}

fn run_real_probe_child() -> TestResult {
    let profile = SignalProfile::Overlay;
    let before = selected_signal_membership(profile)?;
    let mut owner = SignalOwner::install(profile)?;
    let installed = selected_signal_membership(profile)?;
    if installed.iter().any(|blocked| !blocked) {
        return Err(io::Error::other("install did not block every overlay signal").into());
    }

    let inherited = std::thread::spawn(move || selected_signal_membership(profile))
        .join()
        .map_err(|_| io::Error::other("ordinary signal probe thread panicked"))??;
    if inherited.iter().any(|blocked| !blocked) {
        return Err(io::Error::other(
            "ordinary thread did not inherit every blocked overlay signal",
        )
        .into());
    }

    let tid = unsafe {
        // SAFETY: gettid has no preconditions and identifies this fixture
        // thread for the parent process's targeted signal.
        libc::syscall(libc::SYS_gettid) as libc::pid_t
    };
    println!("{REAL_PROBE_READY}{tid}");
    std::io::stdout().flush()?;

    let mut pollfd = libc::pollfd {
        fd: owner.poll_fd()?.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = unsafe {
        // SAFETY: owner retains its signalfd throughout this bounded poll.
        libc::poll(&mut pollfd, 1, 2_000)
    };
    if ready != 1 || pollfd.revents & libc::POLLIN == 0 {
        return Err(io::Error::other(format!(
            "real signalfd did not become readable: result={ready}, readiness={:#x}",
            pollfd.revents
        ))
        .into());
    }
    let events = owner.drain()?;
    if events != [SignalEvent::TrayAction] {
        return Err(io::Error::other(format!(
            "real signalfd returned unexpected events: {events:?}"
        ))
        .into());
    }

    owner.finish()?;
    let restored = selected_signal_membership(profile)?;
    if restored != before {
        return Err(io::Error::other(format!(
            "signal mask was not restored: before={before:?}, after={restored:?}"
        ))
        .into());
    }
    Ok(())
}

fn selected_signal_membership(profile: SignalProfile) -> io::Result<Vec<bool>> {
    let mut current = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
    let error = unsafe {
        // SAFETY: a null set queries without changing this thread's mask and
        // initializes `current` on success.
        libc::pthread_sigmask(libc::SIG_BLOCK, std::ptr::null(), current.as_mut_ptr())
    };
    if error != 0 {
        return Err(io::Error::from_raw_os_error(error));
    }
    let current = unsafe {
        // SAFETY: pthread_sigmask initialized the queried mask on success.
        current.assume_init()
    };
    signals_for_profile(profile)
        .iter()
        .map(|signal| {
            let member = unsafe {
                // SAFETY: `current` is initialized and signal is supported.
                libc::sigismember(&current, *signal)
            };
            match member {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(io::Error::last_os_error()),
            }
        })
        .collect()
}
