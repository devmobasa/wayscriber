//! Frozen, std-only Wayscriber daemon-v1 compatibility fixture.
//!
//! This source deliberately does not import the current Wayscriber crate. It
//! models the baseline v1 client's permissive runtime parser, exact request
//! publication, SIGUSR1 wake, and response wait so v2 compatibility tests
//! cannot accidentally share new wire types.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static SIGNAL_COUNT: AtomicUsize = AtomicUsize::new(0);

const SIGUSR1: i32 = 10;

unsafe extern "C" {
    fn kill(pid: i32, signal: i32) -> i32;
    fn signal(signal: i32, handler: extern "C" fn(i32)) -> usize;
}

extern "C" fn record_signal(_signal: i32) {
    SIGNAL_COUNT.fetch_add(1, Ordering::Relaxed);
}

fn runtime_file(root: &Path) -> PathBuf {
    root.join("wayscriber.pid")
}

fn command_dir(root: &Path) -> PathBuf {
    root.join("daemon-commands")
}

fn json_u32(input: &str, key: &str) -> Option<u32> {
    let marker = format!("\"{key}\"");
    let suffix = input.split_once(&marker)?.1;
    let suffix = suffix.trim_start().strip_prefix(':')?.trim_start();
    let digits = suffix
        .as_bytes()
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    suffix[..digits].parse().ok()
}

fn json_string(input: &str, key: &str) -> Option<String> {
    let marker = format!("\"{key}\"");
    let suffix = input.split_once(&marker)?.1;
    let suffix = suffix.trim_start().strip_prefix(':')?.trim_start();
    let suffix = suffix.strip_prefix('"')?;
    let end = suffix.find('"')?;
    Some(suffix[..end].to_owned())
}

fn atomic_write(path: &Path, payload: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = fs::File::create(&temp)?;
    file.write_all(payload)?;
    drop(file);
    fs::rename(temp, path)
}

#[derive(Clone, Copy)]
enum Effect {
    Freeze,
    ExitAfterCapture,
    NoExitAfterCapture,
    ResumeSession,
    NoResumeSession,
}

impl Effect {
    fn flags(self) -> [bool; 5] {
        match self {
            Self::Freeze => [true, false, false, false, false],
            Self::ExitAfterCapture => [false, true, false, false, false],
            Self::NoExitAfterCapture => [false, false, true, false, false],
            Self::ResumeSession => [false, false, false, true, false],
            Self::NoResumeSession => [false, false, false, false, true],
        }
    }
}

fn request_payload(token: &str, requested_at_unix_ms: u64, effect: Effect) -> String {
    let [freeze, exit_after_capture, no_exit_after_capture, resume_session, no_resume_session] =
        effect.flags();
    let mut payload = format!(
        "{{\"daemon_token\":\"{token}\",\"requested_at_unix_ms\":{requested_at_unix_ms},\"canceled\":false,\"request\":{{\"freeze\":{freeze},\"exit_after_capture\":{exit_after_capture},\"no_exit_after_capture\":{no_exit_after_capture},\"resume_session\":{resume_session},\"no_resume_session\":{no_resume_session}"
    );
    payload.push_str("}}");
    payload
}

fn request_path(root: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    command_dir(root).join(format!(
        "{stamp:032x}-{:08x}.json",
        std::process::id()
    ))
}

fn send_signal(pid: u32, signal_number: i32) -> io::Result<()> {
    let pid = i32::try_from(pid)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "pid does not fit i32"))?;
    // SAFETY: pid came from the runtime record and signal_number is a fixed
    // fixture constant.
    if unsafe { kill(pid, signal_number) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn run_client(root: &Path, effect: Option<Effect>) -> Result<(), String> {
    let runtime = fs::read_to_string(runtime_file(root))
        .map_err(|error| format!("failed to read runtime: {error}"))?;
    let pid = json_u32(&runtime, "pid").ok_or_else(|| "runtime has no pid".to_owned())?;
    let token = json_string(&runtime, "token");
    let command = if let Some(effect) = effect {
        let token = token.ok_or_else(|| {
            "running daemon does not support typed control; restart wayscriber daemon".to_owned()
        })?;
        let path = request_path(root);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        atomic_write(&path, request_payload(&token, now, effect).as_bytes())
            .map_err(|error| format!("failed to publish request: {error}"))?;
        Some(path)
    } else {
        None
    };

    send_signal(pid, SIGUSR1).map_err(|error| format!("failed to signal daemon: {error}"))?;
    let Some(command) = command else {
        return Ok(());
    };

    let response = command_dir(root)
        .join("responses")
        .join(command.file_name().unwrap());
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match fs::read_to_string(&response) {
            Ok(payload) => {
                let _ = fs::remove_file(response);
                if let Some(error) = json_string(&payload, "error") {
                    return Err(error);
                }
                return Ok(());
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if Instant::now() >= deadline {
                    return Err("timed out waiting for daemon response".to_owned());
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(format!("failed to read response: {error}")),
        }
    }
}

fn canonical_request_for_report(payload: &str) -> String {
    let Some((prefix, suffix)) = payload.split_once("\"requested_at_unix_ms\":") else {
        return payload.to_owned();
    };
    let digits = suffix.bytes().take_while(u8::is_ascii_digit).count();
    format!("{prefix}\"requested_at_unix_ms\":0{}", &suffix[digits..])
}

fn process_requests(root: &Path) -> io::Result<usize> {
    let dir = command_dir(root);
    let mut paths = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| {
                // Atomic publication uses a sibling temporary file. Only the
                // final `.json` name is a request the daemon may consume.
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
            .filter_map(|entry| {
                entry
                    .file_type()
                    .ok()
                    .filter(|kind| kind.is_file())
                    .map(|_| entry.path())
            })
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    paths.sort();

    let mut processed = 0;
    for path in paths {
        let payload = fs::read_to_string(&path)?;
        if !payload.contains("\"daemon_token\":\"frozen-v1-token\"") {
            continue;
        }
        let mut request_log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("fixture-requests"))?;
        writeln!(
            request_log,
            "{} {}",
            path.file_name().unwrap().to_string_lossy(),
            canonical_request_for_report(&payload)
        )?;
        fs::remove_file(&path)?;
        let response = dir.join("responses").join(path.file_name().unwrap());
        atomic_write(&response, b"{}")?;
        processed += 1;
    }
    Ok(processed)
}

fn write_report(root: &Path, typed_count: usize) -> io::Result<()> {
    fs::write(
        root.join("fixture-report"),
        format!(
            "signals={}\ntyped={}\n",
            SIGNAL_COUNT.load(Ordering::Relaxed),
            typed_count
        ),
    )
}

fn run_fake_daemon(root: &Path, v2_record: bool) -> io::Result<()> {
    fs::create_dir_all(root)?;
    // SAFETY: the fixture process owns this signal disposition for its entire
    // short test lifetime.
    unsafe {
        signal(SIGUSR1, record_signal);
    }
    let pid = std::process::id();
    let runtime = if v2_record {
        format!(
            "{{\"pid\":{pid},\"runtime_record_version\":2,\"typed_control_protocol_version\":2,\"boot_id\":\"00000000-0000-0000-0000-000000000000\",\"time_namespace\":{{\"dev\":1,\"ino\":1}},\"pid_namespace\":{{\"dev\":1,\"ino\":1}},\"process_start_ticks\":1,\"v2_instance_token\":\"{}\"}}",
            "0".repeat(64)
        )
    } else {
        format!("{{\"pid\":{pid},\"token\":\"frozen-v1-token\"}}")
    };
    atomic_write(&runtime_file(root), runtime.as_bytes())?;

    let mut typed_count = 0;
    loop {
        typed_count += process_requests(root)?;
        write_report(root, typed_count)?;
        thread::sleep(Duration::from_millis(2));
    }
}

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let mode = arguments.next().and_then(|value| value.into_string().ok());
    let root = arguments.next().map(PathBuf::from);
    let Some(root) = root else {
        eprintln!("usage: frozen-daemon-v1 <daemon-v1|daemon-v2|client-*> <root>");
        return ExitCode::from(64);
    };

    match mode.as_deref() {
        Some("daemon-v1") => match run_fake_daemon(&root, false) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Some("daemon-v2") => match run_fake_daemon(&root, true) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Some("client-empty")
        | Some("client-freeze")
        | Some("client-exit-after-capture")
        | Some("client-no-exit-after-capture")
        | Some("client-resume-session")
        | Some("client-no-resume-session") => {
            let effect = match mode.as_deref() {
                Some("client-freeze") => Some(Effect::Freeze),
                Some("client-exit-after-capture") => Some(Effect::ExitAfterCapture),
                Some("client-no-exit-after-capture") => Some(Effect::NoExitAfterCapture),
                Some("client-resume-session") => Some(Effect::ResumeSession),
                Some("client-no-resume-session") => Some(Effect::NoResumeSession),
                _ => None,
            };
            match run_client(&root, effect) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!("unknown fixture mode");
            ExitCode::from(64)
        }
    }
}
