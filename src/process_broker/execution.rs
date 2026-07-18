use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};

use super::transport::shutdown_requested;
use super::wire::{HelperKind, MAX_STDERR_BYTES, OutputMode};

pub(super) struct BoundedOutput {
    pub(super) status: ExitStatus,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) timed_out: bool,
    pub(super) stdout_limit_reached: bool,
}

pub(super) struct PublishOutput {
    pub(super) status: i32,
    pub(super) timed_out: bool,
    /// Unreaped successful leader that pins the retained provider's process group.
    pub(super) retained: Option<Child>,
}

pub(super) fn supports_retained_publication(kind: HelperKind) -> bool {
    if matches!(kind, HelperKind::WlCopy) {
        return true;
    }
    #[cfg(test)]
    if matches!(kind, HelperKind::TestShell) {
        return true;
    }
    false
}

pub(super) fn terminate_owned_children(
    children: &mut std::collections::BTreeMap<String, std::process::Child>,
) {
    for (_, mut child) in std::mem::take(children) {
        kill_child_process_group(&mut child);
        let _ = child.wait();
    }
}

pub(super) fn kill_child_process_group(child: &mut std::process::Child) {
    let pid = child.id();
    if let Ok(group) = i32::try_from(pid) {
        // SAFETY: broker children use a fresh process group equal to their PID.
        if unsafe { libc::kill(-group, libc::SIGKILL) } == 0 {
            return;
        }
    }
    let _ = child.kill();
}

#[derive(Clone, Copy)]
enum KillScope {
    ProcessGroup,
    Process,
}

pub(super) struct OwnedProcess {
    child: Option<Child>,
    kill_scope: KillScope,
}

impl OwnedProcess {
    pub(super) fn process_group(child: Child) -> Self {
        Self {
            child: Some(child),
            kill_scope: KillScope::ProcessGroup,
        }
    }

    pub(super) fn process(child: Child) -> Self {
        Self {
            child: Some(child),
            kill_scope: KillScope::Process,
        }
    }

    pub(super) fn id(&self) -> u32 {
        self.child().id()
    }

    pub(super) fn into_child(mut self) -> Child {
        self.child.take().expect("broker child remains owned")
    }

    fn child(&self) -> &Child {
        self.child.as_ref().expect("broker child remains owned")
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("broker child remains owned")
    }

    fn terminate(&mut self) {
        match self.kill_scope {
            KillScope::ProcessGroup => kill_child_process_group(self.child_mut()),
            KillScope::Process => {
                let _ = self.child_mut().kill();
            }
        }
    }

    pub(super) fn wait(&mut self) -> io::Result<ExitStatus> {
        let result = self.child_mut().wait();
        if result.is_ok() {
            self.child = None;
        }
        result
    }
}

impl Drop for OwnedProcess {
    fn drop(&mut self) {
        if self.child.is_some() {
            self.terminate();
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.wait();
        }
    }
}

fn child_status_unreaped(child: &std::process::Child) -> io::Result<Option<i32>> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    // WNOWAIT pins the zombie leader and therefore its process-group identity.
    // SAFETY: info is writable and child is owned by this broker.
    if unsafe {
        libc::waitid(
            libc::P_PID,
            child.id(),
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: waitid initialized info after success.
    let info = unsafe { info.assume_init() };
    if unsafe { info.si_pid() } == 0 {
        return Ok(None);
    }
    // SAFETY: CLD terminal records initialize si_status.
    let detail = unsafe { info.si_status() };
    let status = match info.si_code {
        libc::CLD_EXITED => detail,
        libc::CLD_KILLED | libc::CLD_DUMPED => 128_i32.saturating_add(detail),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "waitid returned a nonterminal child status",
            ));
        }
    };
    Ok(Some(status))
}

pub(super) fn publish_bounded(
    mut command: Command,
    input: Vec<u8>,
    timeout: Duration,
    shutdown_fd: std::os::fd::RawFd,
) -> Result<PublishOutput> {
    if shutdown_requested(shutdown_fd)? {
        return Err(anyhow!("broker publication cancelled during shutdown"));
    }
    let deadline = crate::daemon::protocol_v2::BootClock::now()?
        .checked_add(timeout.max(Duration::from_millis(1)))?;
    command
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = OwnedProcess::process_group(
        command
            .spawn()
            .context("broker publication helper spawn failed")?,
    );
    let stdin = child.child_mut().stdin.take();
    let stdin_writer = std::thread::spawn(move || match stdin {
        Some(stdin) => write_input_until(stdin, &input, deadline, shutdown_fd),
        None => Ok(()),
    });
    loop {
        if let Some(unreaped_status) = child_status_unreaped(child.child())? {
            let stdin_result = stdin_writer
                .join()
                .map_err(|_| anyhow!("broker publication stdin writer panicked"))?;
            if unreaped_status != 0 || stdin_result.is_err() {
                child.terminate();
                let status = status_code(child.wait()?);
                stdin_result.context("broker publication stdin write failed")?;
                return Ok(PublishOutput {
                    status,
                    timed_out: false,
                    retained: None,
                });
            }
            stdin_result.context("broker publication stdin write failed")?;
            return Ok(PublishOutput {
                status: unreaped_status,
                timed_out: false,
                retained: Some(child.into_child()),
            });
        }
        if crate::daemon::protocol_v2::BootClock::now()? >= deadline {
            child.terminate();
            let status = status_code(child.wait()?);
            let _ = stdin_writer.join();
            return Ok(PublishOutput {
                status,
                timed_out: true,
                retained: None,
            });
        }
        if shutdown_requested(shutdown_fd)? {
            child.terminate();
            let _ = child.wait();
            let _ = stdin_writer.join();
            return Err(anyhow!("broker publication cancelled during shutdown"));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn write_input_until(
    mut stdin: std::process::ChildStdin,
    input: &[u8],
    deadline: crate::daemon::protocol_v2::BootDeadline,
    shutdown_fd: std::os::fd::RawFd,
) -> io::Result<()> {
    let descriptor = stdin.as_raw_fd();
    // SAFETY: fcntl reads and updates descriptor-local flags.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let mut offset = 0;
    while offset < input.len() {
        if shutdown_requested(shutdown_fd)? {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "publication input cancelled during shutdown",
            ));
        }
        match stdin.write(&input[offset..]) {
            Ok(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
            Ok(written) => offset += written,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if crate::daemon::protocol_v2::BootClock::now().map_err(io::Error::other)?
                    >= deadline
                {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "publication input deadline expired",
                    ));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(super) fn run_bounded(
    mut command: Command,
    input: Vec<u8>,
    timeout: Duration,
    output_cap: usize,
    output_mode: OutputMode,
    shutdown_fd: std::os::fd::RawFd,
) -> Result<BoundedOutput> {
    if shutdown_requested(shutdown_fd)? {
        return Err(anyhow!("broker helper cancelled during shutdown"));
    }
    let deadline = crate::daemon::protocol_v2::BootClock::now()?
        .checked_add(timeout.max(Duration::from_millis(1)))?;
    command.process_group(0);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child =
        OwnedProcess::process_group(command.spawn().context("broker helper spawn failed")?);
    let mut stdin = child.child_mut().stdin.take();
    let stdout = child
        .child_mut()
        .stdout
        .take()
        .context("broker stdout pipe missing")?;
    let stderr = child
        .child_mut()
        .stderr
        .take()
        .context("broker stderr pipe missing")?;
    let stdout_limit_reached = Arc::new(AtomicBool::new(false));
    let stderr_overflow = Arc::new(AtomicBool::new(false));
    let stdout_reader = {
        let limit_reached = Arc::clone(&stdout_limit_reached);
        std::thread::spawn(move || match output_mode {
            OutputMode::Complete => read_capped(stdout, output_cap, &limit_reached),
            OutputMode::Prefix => read_prefix(stdout, output_cap, &limit_reached),
        })
    };
    let stderr_reader = {
        let overflow = Arc::clone(&stderr_overflow);
        std::thread::spawn(move || read_capped(stderr, output_cap.min(MAX_STDERR_BYTES), &overflow))
    };
    let stdin_writer = std::thread::spawn(move || {
        stdin.take().and_then(|mut stdin| {
            stdin
                .write_all(&input)
                .err()
                .filter(|error| error.kind() != io::ErrorKind::BrokenPipe)
        })
    });
    let (status, timed_out, cancelled) = loop {
        if child_status_unreaped(child.child())?.is_some() {
            child.terminate();
            break (child.wait()?, false, false);
        }
        if crate::daemon::protocol_v2::BootClock::now()? >= deadline {
            child.terminate();
            break (child.wait()?, true, false);
        }
        let cancelled = shutdown_requested(shutdown_fd)?;
        if stdout_limit_reached.load(Ordering::Acquire)
            || stderr_overflow.load(Ordering::Acquire)
            || cancelled
        {
            child.terminate();
            break (child.wait()?, false, cancelled);
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow!("broker stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow!("broker stderr reader panicked"))??;
    let stdin_error = stdin_writer
        .join()
        .map_err(|_| anyhow!("broker stdin writer panicked"))?;
    if let Some(error) = stdin_error {
        return Err(error).context("broker helper stdin write failed");
    }
    if cancelled {
        return Err(anyhow!("broker helper cancelled during shutdown"));
    }
    let stdout_limit_reached = stdout_limit_reached.load(Ordering::Acquire);
    if output_mode == OutputMode::Complete && stdout_limit_reached {
        return Err(anyhow!("broker helper stdout exceeded output cap"));
    }
    if stderr_overflow.load(Ordering::Acquire) {
        return Err(anyhow!("broker helper stderr exceeded output cap"));
    }
    Ok(BoundedOutput {
        status,
        stdout,
        stderr,
        timed_out,
        stdout_limit_reached,
    })
}

fn read_prefix(
    mut reader: impl Read,
    cap: usize,
    limit_reached: &AtomicBool,
) -> io::Result<Vec<u8>> {
    let mut retained = Vec::with_capacity(cap.min(8192));
    if cap == 0 {
        limit_reached.store(true, Ordering::Release);
        return Ok(retained);
    }
    let mut buffer = [0_u8; 8192];
    loop {
        let remaining = cap.saturating_sub(retained.len());
        let read_cap = buffer.len().min(remaining);
        let read = reader.read(&mut buffer[..read_cap])?;
        if read == 0 {
            return Ok(retained);
        }
        retained.extend_from_slice(&buffer[..read]);
        if retained.len() == cap {
            limit_reached.store(true, Ordering::Release);
            return Ok(retained);
        }
    }
}

fn read_capped(mut reader: impl Read, cap: usize, overflow: &AtomicBool) -> io::Result<Vec<u8>> {
    let mut retained = Vec::with_capacity(cap.min(8192));
    let mut buffer = [0_u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(retained);
        }
        let remaining = cap.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..read.min(remaining)]);
        if read > remaining {
            overflow.store(true, Ordering::Release);
        }
    }
}

pub(super) fn status_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}
