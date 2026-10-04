//! External helper programs, played by this test binary.
//!
//! A test links a helper's name to the test binary with [`link`], then sets the
//! [`environment`] for that link while it launches the helper. When this binary
//! starts under exactly that path, the constructor below plays the requested
//! [`Role`] and exits before libtest runs. Any other start, such as another
//! test's child launched while the variables are set, runs the tests as usual,
//! so no test writes a script or relies on a system program.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The path a helper start must have been launched under to play a role.
const LINK_ENV: &str = "WAYSCRIBER_TEST_FAKE_HELPER";
const ROLE_ENV: &str = "WAYSCRIBER_TEST_FAKE_HELPER_ROLE";
/// Where a role that reports writes its report.
const OUTPUT_ENV: &str = "WAYSCRIBER_TEST_FAKE_HELPER_OUTPUT";

#[derive(Clone, Copy, Debug)]
pub(crate) enum Role {
    /// Exits successfully.
    Exit,
    /// Reports `process-group-leader` if it leads its process group, else
    /// `session-eligible`: only a non-leader may call `setsid()`.
    ReportProcessGroup,
    /// Reads its standard input to the end and reports the byte count.
    CountInput,
    /// Reports its arguments, one per line.
    RecordArguments,
}

impl Role {
    const ALL: [Self; 4] = [
        Self::Exit,
        Self::ReportProcessGroup,
        Self::CountInput,
        Self::RecordArguments,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Exit => "exit",
            Self::ReportProcessGroup => "report-process-group",
            Self::CountInput => "count-input",
            Self::RecordArguments => "record-arguments",
        }
    }
}

/// A link named `name` in `directory` to this test binary.
pub(crate) fn link(directory: &Path, name: &str) -> PathBuf {
    let link = directory.join(name);
    std::os::unix::fs::symlink(std::env::current_exe().expect("test binary"), &link)
        .expect("link the test binary under the helper's name");
    link
}

/// The variables that make a start under `link` play `role`, reporting to
/// `output`. Set them, for example with `test_env::with_env_vars`, while the
/// helper is launched.
pub(crate) fn environment<'a>(
    link: &'a Path,
    role: Role,
    output: &'a Path,
) -> [(&'static str, Option<&'a OsStr>); 3] {
    [
        (LINK_ENV, Some(link.as_os_str())),
        (ROLE_ENV, Some(OsStr::new(role.name()))),
        (OUTPUT_ENV, Some(output.as_os_str())),
    ]
}

// SAFETY: the loader calls each `.init_array` entry once, before `main`, with
// the C calling convention. glibc passes `argc`, `argv`, and `envp`; under the
// C ABI a function that declares no parameters ignores extra arguments, so an
// `extern "C" fn()` is sound to register here. Unless this start is a helper
// start, the function only reads the environment and `/proc` and returns. A
// helper start never returns into `main`: it exits, and a panic aborts instead
// of unwinding through the loader because the function is `extern "C"`.
#[used]
#[unsafe(link_section = ".init_array")]
static PLAY_FAKE_HELPER: extern "C" fn() = play_fake_helper;

extern "C" fn play_fake_helper() {
    let Some(link) = std::env::var_os(LINK_ENV) else {
        return;
    };
    let Ok(mut arguments) = launch_arguments() else {
        return;
    };
    if arguments.is_empty() || OsStr::new(&arguments.remove(0)) != link {
        return;
    }

    let status = match play(&arguments) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("fake helper failed: {error:#}");
            1
        }
    };
    std::process::exit(status);
}

fn play(arguments: &[String]) -> Result<()> {
    let role = std::env::var(ROLE_ENV).context("fake helper role")?;
    let role = Role::ALL
        .into_iter()
        .find(|known| known.name() == role)
        .with_context(|| format!("unknown fake helper role {role:?}"))?;
    let output = std::env::var_os(OUTPUT_ENV).context("fake helper output")?;

    let report = match role {
        Role::Exit => return Ok(()),
        Role::ReportProcessGroup => {
            // SAFETY: getpgrp has no preconditions and cannot fail.
            let leader = unsafe { libc::getpgrp() } == std::process::id() as libc::pid_t;
            if leader {
                "process-group-leader"
            } else {
                "session-eligible"
            }
            .to_owned()
        }
        Role::CountInput => {
            let mut input = Vec::new();
            std::io::stdin()
                .read_to_end(&mut input)
                .context("read the helper's input")?;
            format!("{}\n", input.len())
        }
        Role::RecordArguments => arguments
            .iter()
            .map(|argument| format!("{argument}\n"))
            .collect(),
    };

    crate::durable_io::write_atomic(
        Path::new(&output),
        report.as_bytes(),
        crate::durable_io::AtomicWriteOptions::private_runtime_file(),
    )?;
    Ok(())
}

/// The arguments this process was launched with, `argv[0]` first: std's own
/// argument capture may not have run before this constructor.
fn launch_arguments() -> Result<Vec<String>> {
    let raw = std::fs::read("/proc/self/cmdline").context("read /proc/self/cmdline")?;
    let raw = raw.strip_suffix(b"\0").unwrap_or(&raw);

    raw.split(|byte| *byte == 0)
        .map(|argument| String::from_utf8(argument.to_vec()).context("non-UTF-8 argument"))
        .collect()
}
