use std::env;
use std::process::{Command, ExitStatus, Output};

pub(super) const CHILD_ENV: &str = "WAYSCRIBER_GTK_CAPTURE_RENDER_CHILD";
const COMPLETION_PREFIX: &str = "WAYSCRIBER_CAPTURE_RENDER_COMPLETED ";

#[derive(Debug, PartialEq)]
pub(super) enum Outcome {
    Passed,
    VulkanUnavailable,
}

pub(super) fn complete(renderer: &str, case: &str, outcome: &Outcome) {
    eprintln!("{}", completion_marker(renderer, case, outcome));
}

fn completion_marker(renderer: &str, case: &str, outcome: &Outcome) -> String {
    let outcome = match outcome {
        Outcome::Passed => "passed",
        Outcome::VulkanUnavailable => "unavailable",
    };
    format!("{COMPLETION_PREFIX}renderer={renderer} case={case} outcome={outcome}")
}

fn child(name: &str, renderer: &str, case: &str) -> Command {
    let mut command = Command::new(env::current_exe().expect("test binary"));
    command
        .args([name, "--exact", "--test-threads=1", "--nocapture"])
        .env(CHILD_ENV, case)
        .env("GSK_RENDERER", renderer)
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("G_DEBUG", "fatal-criticals");
    command
}

fn validate(output: &Output, renderer: &str, case: &str) -> Result<Outcome, String> {
    let outcome = match output.status.code() {
        Some(0) => Outcome::Passed,
        Some(77) if renderer == "vulkan" && case == "renderer-probe" => Outcome::VulkanUnavailable,
        _ => return Err(format!("child exited with {}", output.status)),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let markers: Vec<_> = stdout
        .lines()
        .chain(stderr.lines())
        .filter(|line| line.starts_with(COMPLETION_PREFIX))
        .collect();
    let expected = completion_marker(renderer, case, &outcome);
    if markers != [expected.as_str()] {
        return Err(format!(
            "expected exactly {expected:?} after assertions and cleanup; observed {markers:?}"
        ));
    }

    Ok(outcome)
}

fn run(name: &str, renderer: &str, case: &str) -> Result<Outcome, String> {
    let output = child(name, renderer, case)
        .output()
        .map_err(|error| format!("could not run child: {error}"))?;
    // Forward GTK diagnostics and the child's assertion/cleanup completion even
    // on success. A stale exact filter exits zero without that completion.
    eprint!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    validate(&output, renderer, case)
}

pub(super) fn run_cases(name: &str) {
    let mut failures = Vec::new();
    let mut renderers = vec!["cairo", "gl"];
    match run(name, "vulkan", "renderer-probe") {
        Ok(Outcome::Passed) => renderers.push("vulkan"),
        Ok(Outcome::VulkanUnavailable) => {
            eprintln!("SKIPPED: optional Vulkan capture pixels; GTK selected another renderer");
        }
        Err(error) => failures.push(format!("vulkan/renderer-probe: {error}")),
    }

    for renderer in renderers {
        for case in [
            "window",
            "wrapper",
            "wrapper-minimum",
            "picture",
            "proof-geometry",
            "universal",
            "transition",
            "animation",
            "border-image",
            "popup-none",
            "popup-small",
            "popup-default",
            "popup-large",
            "menu",
            "menu-decorated",
            "menu-default",
            "menu-long",
            "lifecycle",
        ] {
            if let Err(error) = run(name, renderer, case) {
                failures.push(format!("{renderer}/{case}: {error}"));
            }
        }
    }
    assert!(failures.is_empty(), "capture render failures: {failures:?}");
    eprintln!("EXECUTED: GTK capture root pixel assertions (Cairo and GL)");
}

#[test]
fn zero_test_child_cannot_complete_a_case_or_renderer_probe() {
    let name = concat!(module_path!(), "::__nonexistent_capture_render_child_test");
    let name = name.strip_prefix("wayscriber::").unwrap();
    let output = child(name, "cairo", "window")
        .output()
        .expect("run zero-test negative control");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("running 0 tests"));
    assert!(validate(&output, "cairo", "window").is_err());
    assert!(validate(&output, "vulkan", "renderer-probe").is_err());
}

#[test]
fn completion_requires_matching_case_renderer_outcome_and_exit_status() {
    use std::os::unix::process::ExitStatusExt;

    let mut output = Output {
        status: ExitStatus::from_raw(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
    };
    output.stderr = completion_marker("cairo", "window", &Outcome::Passed).into_bytes();
    assert_eq!(validate(&output, "cairo", "window"), Ok(Outcome::Passed));
    assert!(validate(&output, "gl", "window").is_err());
    assert!(validate(&output, "cairo", "wrapper").is_err());
    output.stdout = output.stderr.clone();
    assert!(validate(&output, "cairo", "window").is_err());
    output.stdout.clear();
    output.status = ExitStatus::from_raw(1 << 8);
    assert!(validate(&output, "cairo", "window").is_err());

    output.status = ExitStatus::from_raw(77 << 8);
    output.stderr =
        completion_marker("vulkan", "renderer-probe", &Outcome::VulkanUnavailable).into_bytes();
    assert_eq!(
        validate(&output, "vulkan", "renderer-probe"),
        Ok(Outcome::VulkanUnavailable)
    );
    assert!(validate(&output, "vulkan", "window").is_err());
    assert!(validate(&output, "gl", "renderer-probe").is_err());
    output.stderr =
        completion_marker("gl", "renderer-probe", &Outcome::VulkanUnavailable).into_bytes();
    assert!(validate(&output, "vulkan", "renderer-probe").is_err());
    output.stderr = completion_marker("vulkan", "window", &Outcome::VulkanUnavailable).into_bytes();
    assert!(validate(&output, "vulkan", "renderer-probe").is_err());
    output.stderr = completion_marker("vulkan", "renderer-probe", &Outcome::Passed).into_bytes();
    assert!(validate(&output, "vulkan", "renderer-probe").is_err());
    output.stderr.clear();
    assert!(validate(&output, "vulkan", "renderer-probe").is_err());
}
