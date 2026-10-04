use super::*;
use std::ffi::OsStr;

/// Marks the child run of the launch test below.
const CHILD_ENV: &str = "WAYSCRIBER_TEST_LAUNCH_ENVIRONMENT_CHILD";
const LAUNCH_TOKEN: &str = "launch-environment-probe";
const LAUNCH_STARTUP_ID: &str = "launch-environment-startup-id";

#[test]
fn the_first_entry_for_a_name_is_its_value() {
    let environment = b"HOME=/home/user\0XDG_ACTIVATION_TOKEN=first\0XDG_ACTIVATION_TOKEN=second\0";

    assert_eq!(
        value_in(environment, XDG_ACTIVATION_TOKEN_ENV),
        Some(&b"first"[..])
    );
}

#[test]
fn only_a_whole_name_matches() {
    let environment = b"XDG_ACTIVATION_TOKEN_EXTRA=x\0XDG_ACTIVATION=y\0EMPTY=\0";

    assert_eq!(value_in(environment, XDG_ACTIVATION_TOKEN_ENV), None);
    assert_eq!(value_in(environment, "EMPTY"), Some(&b""[..]));
}

#[test]
fn the_startup_token_is_trimmed_and_falls_back_to_the_startup_id() {
    let token = |entries: &[(&str, &str)]| {
        startup_activation_token_from(|name| {
            entries
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        })
    };

    assert_eq!(
        token(&[(XDG_ACTIVATION_TOKEN_ENV, "  token \n")]).as_deref(),
        Some("token")
    );
    assert_eq!(
        token(&[
            (XDG_ACTIVATION_TOKEN_ENV, "first"),
            (DESKTOP_STARTUP_ID_ENV, "second")
        ])
        .as_deref(),
        Some("first")
    );
    assert_eq!(
        token(&[
            (XDG_ACTIVATION_TOKEN_ENV, " "),
            (DESKTOP_STARTUP_ID_ENV, "startup")
        ])
        .as_deref(),
        Some("startup")
    );
    assert_eq!(token(&[(DESKTOP_STARTUP_ID_ENV, "")]), None);
    assert_eq!(token(&[]), None);
}

/// A token passed at launch reaches the reader after the live environment
/// loses it. The launched child removes both variables itself, as GTK 4.16 and
/// later do before `main`, so the outcome does not depend on the build's
/// features or the system's GTK.
#[test]
fn a_launch_token_survives_its_removal_from_the_live_environment() {
    if std::env::var_os(CHILD_ENV).is_some() {
        let removed = STARTUP_NOTIFICATION_VARIABLES.map(|name| (name, None::<&OsStr>));
        crate::test_env::with_env_vars(&removed, || {
            for name in STARTUP_NOTIFICATION_VARIABLES {
                assert_eq!(std::env::var_os(name), None, "{name} is still live");
            }
            assert_eq!(startup_activation_token().as_deref(), Some(LAUNCH_TOKEN));
            assert_eq!(
                var_os(DESKTOP_STARTUP_ID_ENV).as_deref(),
                Some(OsStr::new(LAUNCH_STARTUP_ID))
            );
        });
        return;
    }

    let test_name = concat!(
        module_path!(),
        "::a_launch_token_survives_its_removal_from_the_live_environment"
    )
    .strip_prefix(concat!(env!("CARGO_CRATE_NAME"), "::"))
    .expect("test path contains the crate prefix");
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([test_name, "--exact", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env(XDG_ACTIVATION_TOKEN_ENV, LAUNCH_TOKEN)
        .env(DESKTOP_STARTUP_ID_ENV, LAUNCH_STARTUP_ID)
        .output()
        .expect("run the test binary as a launched child");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "launched child failed:\n{stdout}");
    // A filter that matched nothing would pass without running the child.
    assert!(
        stdout.contains("1 passed;"),
        "launched child ran no test:\n{stdout}"
    );
}

#[test]
fn without_a_launch_environment_the_live_one_is_read() {
    const NAME: &str = "WAYSCRIBER_TEST_LAUNCH_ENVIRONMENT_FALLBACK";

    crate::test_env::with_env_var(NAME, Some(OsStr::new("live")), || {
        assert_eq!(
            var_os_in(Path::new("/nonexistent/environ"), NAME).as_deref(),
            Some(OsStr::new("live"))
        );
    });
}
