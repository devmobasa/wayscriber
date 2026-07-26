//! Fetching the release manifest through the system HTTP client.
//!
//! Wayscriber links no HTTP or TLS stack: a screen annotation tool that talks
//! to the network exactly once a day should not carry a rustls tree, and
//! vendored distro builds should not have to audit one. The process broker runs
//! `curl` (or `wget`), which also means the check uses the system CA store and
//! degrades to "no check" on machines that have neither. Broker ownership is
//! important in the threaded daemon, and its bounded output path cuts off an
//! oversized or endless response while it is still streaming.
//!
//! The request is a plain GET of a static file: no query string, no headers
//! identifying the user, no cookies.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::manifest::MAX_MANIFEST_BYTES;

/// Programs tried in order; the first one present on the system wins.
const FETCHERS: [&str; 2] = ["curl", "wget"];

/// Download `url` and return its body.
pub(crate) fn fetch(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    url: &str,
    timeout: Duration,
) -> Result<String, String> {
    fetch_with(url, timeout, |program, url, timeout| {
        run_fetcher(process_broker, program, url, timeout)
    })
}

fn fetch_with(
    url: &str,
    timeout: Duration,
    mut run: impl FnMut(&str, &str, Duration) -> Result<String, FetchError>,
) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err("refusing to fetch a non-HTTPS update manifest".to_string());
    }

    for program in FETCHERS {
        match run(program, url, timeout) {
            Ok(body) => return Ok(body),
            // Not installed: fall through to the next client.
            Err(FetchError::Unavailable) => continue,
            // Installed but it failed: that is the answer. Asking a second
            // client would issue a second request for the same check and
            // double how long the About window blocks.
            Err(FetchError::Failed(message)) => return Err(message),
        }
    }

    Err("no HTTP client available (install curl or wget)".to_string())
}

enum FetchError {
    /// The program is not installed; try the next one.
    Unavailable,
    Failed(String),
}

fn run_fetcher(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    program: &str,
    url: &str,
    timeout: Duration,
) -> Result<String, FetchError> {
    let Some(program_path) = find_in_path(program) else {
        return Err(FetchError::Unavailable);
    };

    let arguments = fetch_arguments(program, url, timeout);
    let output = process_broker
        .run(
            crate::process_broker::HelperKind::UpdateFetcher,
            program_path.as_os_str(),
            &arguments,
            Vec::new(),
            timeout.max(Duration::from_secs(1)),
            MAX_MANIFEST_BYTES + 1,
        )
        .map_err(|err| {
            let message = err.to_string();
            if message.contains("stdout exceeded output cap") {
                FetchError::Failed(OVERSIZED.to_string())
            } else {
                FetchError::Failed(format!("failed to run {program}: {message}"))
            }
        })?;

    if output.timed_out {
        return Err(FetchError::Failed("request timed out".to_string()));
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(FetchError::Failed(format!(
            "{program} failed: {}",
            failure_detail(program, stderr.trim(), Some(output.status))
        )));
    }

    validate_body(&output.stdout).map_err(FetchError::Failed)
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_else(|| "/bin:/usr/bin".into());
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find(|candidate| is_executable_file(candidate))
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

/// Turn a fetcher failure into something a user can act on. The program's own
/// message is best; failing that, its exit code is translated, because "exit
/// status 8" tells nobody that the file is missing on the server.
fn failure_detail(program: &str, stderr: &str, code: Option<i32>) -> String {
    if !stderr.is_empty() {
        return stderr.to_string();
    }
    match (program, code) {
        (_, None) => "killed by a signal".to_string(),
        ("curl", Some(6)) => "could not resolve wayscriber.com".to_string(),
        ("curl", Some(7)) => "could not connect".to_string(),
        ("curl", Some(22)) => "server returned an error response".to_string(),
        ("curl", Some(28)) => "request timed out".to_string(),
        ("curl", Some(35) | Some(60)) => "TLS verification failed".to_string(),
        ("curl", Some(63)) => "response larger than expected".to_string(),
        (_, Some(4)) => "network failure".to_string(),
        (_, Some(5)) => "TLS verification failed".to_string(),
        (_, Some(8)) => "server returned an error response".to_string(),
        (_, Some(code)) => format!("exit status {code}"),
    }
}

/// Command line for one fetcher. HTTPS is enforced across redirects, the
/// response is size-capped, and the whole request is time-boxed so a hung
/// server cannot pin a thread.
fn fetch_arguments(program: &str, url: &str, timeout: Duration) -> Vec<String> {
    let seconds = timeout.as_secs().max(1).to_string();
    match program {
        "curl" => vec![
            // Must come first for curl to honor it. Without it a `~/.curlrc`
            // could add headers, cookies, `--user-agent`, extra URLs, an
            // `--output` file, or `--verbose` noise — none of which belong in a
            // request documented as carrying nothing and happening once.
            "--disable".into(),
            "--fail".into(),
            "--silent".into(),
            "--show-error".into(),
            // Do not turn the system client's version into request metadata.
            "--user-agent".into(),
            String::new(),
            "--location".into(),
            "--proto".into(),
            "=https".into(),
            "--proto-redir".into(),
            "=https".into(),
            "--tlsv1.2".into(),
            "--max-redirs".into(),
            "3".into(),
            "--max-time".into(),
            seconds,
            "--max-filesize".into(),
            MAX_MANIFEST_BYTES.to_string(),
            url.to_string(),
        ],
        // `--no-verbose` rather than `--quiet`: progress noise is gone either
        // way, but this keeps the error line that explains a failure.
        _ => vec![
            // Same reasoning as curl's `--disable`: ignore `/etc/wgetrc` and
            // `~/.wgetrc`, which can otherwise add headers, cookie jars, output
            // files, or verbosity.
            "--no-config".into(),
            // Wget otherwise identifies itself as Wget/VERSION and can consult
            // ~/.netrc for credentials even when its config files are off.
            "--user-agent=".into(),
            "--no-netrc".into(),
            "--no-verbose".into(),
            "--output-document=-".into(),
            "--https-only".into(),
            "--max-redirect=3".into(),
            "--tries=1".into(),
            format!("--timeout={seconds}"),
            url.to_string(),
        ],
    }
}

/// What an over-cap response reports, whichever client produced it.
const OVERSIZED: &str = "release manifest is implausibly large";

/// Reject oversized or non-UTF-8 bodies before they reach the JSON parser.
fn validate_body(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(OVERSIZED.to_string());
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| "release manifest is not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_https_urls_without_spawning_anything() {
        let err = fetch_with(
            "http://wayscriber.com/latest.json",
            Duration::from_secs(5),
            |_, _, _| Err(FetchError::Unavailable),
        )
        .expect_err("non-HTTPS fixture URL is rejected before transport");
        assert!(err.contains("non-HTTPS"), "unexpected error: {err}");
    }

    #[test]
    fn curl_arguments_pin_https_and_bound_the_request() {
        let args = fetch_arguments(
            "curl",
            "https://wayscriber.com/latest.json",
            Duration::from_secs(5),
        );

        // User config must be ignored, and curl only honors this as arg one.
        assert_eq!(
            args.first()
                .expect("curl fixture always emits its required first argument"),
            "--disable"
        );
        assert!(args.contains(&"--fail".to_string()));
        assert!(args.windows(2).any(|pair| pair == ["--proto", "=https"]));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--proto-redir", "=https"])
        );
        assert!(args.windows(2).any(|pair| pair == ["--max-time", "5"]));
        assert!(
            args.windows(2).any(
                |pair| pair[0] == "--max-filesize" && pair[1] == MAX_MANIFEST_BYTES.to_string()
            )
        );
        assert_eq!(
            args.last()
                .expect("curl fixture always emits its requested URL"),
            "https://wayscriber.com/latest.json"
        );
    }

    #[test]
    fn wget_arguments_pin_https_and_bound_the_request() {
        let args = fetch_arguments(
            "wget",
            "https://wayscriber.com/latest.json",
            Duration::from_secs(7),
        );

        assert!(args.contains(&"--no-config".to_string()));
        assert!(args.contains(&"--https-only".to_string()));
        assert!(args.contains(&"--timeout=7".to_string()));
        assert!(args.contains(&"--tries=1".to_string()));
        // Errors must survive: only progress output is suppressed.
        assert!(args.contains(&"--no-verbose".to_string()));
        assert!(!args.contains(&"--quiet".to_string()));
        assert_eq!(
            args.last()
                .expect("wget fixture always emits its requested URL"),
            "https://wayscriber.com/latest.json"
        );
    }

    #[test]
    fn failures_are_explained_without_leaning_on_exit_codes() {
        assert_eq!(
            failure_detail("wget", "ERROR 404: Not Found.", Some(8)),
            "ERROR 404: Not Found."
        );
        assert_eq!(
            failure_detail("wget", "", Some(8)),
            "server returned an error response"
        );
        assert_eq!(failure_detail("curl", "", Some(28)), "request timed out");
        assert_eq!(failure_detail("curl", "", Some(99)), "exit status 99");
        assert_eq!(failure_detail("curl", "", None), "killed by a signal");
    }

    /// Every client must ignore user configuration: a `.curlrc`/`.wgetrc` could
    /// otherwise attach identifiers, fetch extra URLs, redirect the body to a
    /// file, or turn on the verbose output the stderr handling assumes is small.
    #[test]
    fn user_configuration_is_suppressed_for_every_client() {
        for program in FETCHERS {
            let args = fetch_arguments(
                program,
                "https://wayscriber.com/latest.json",
                Duration::from_secs(5),
            );
            let suppressor = if program == "curl" {
                "--disable"
            } else {
                "--no-config"
            };
            assert!(
                args.contains(&suppressor.to_string()),
                "{program} must be told to ignore user config"
            );
        }
    }

    #[test]
    fn clients_do_not_advertise_their_version_or_read_netrc_credentials() {
        let curl = fetch_arguments(
            "curl",
            "https://wayscriber.com/latest.json",
            Duration::from_secs(5),
        );
        assert!(
            curl.windows(2).any(|pair| pair == ["--user-agent", ""]),
            "curl must send no default user-agent"
        );

        let wget = fetch_arguments(
            "wget",
            "https://wayscriber.com/latest.json",
            Duration::from_secs(5),
        );
        assert!(wget.contains(&"--user-agent=".to_string()));
        assert!(wget.contains(&"--no-netrc".to_string()));
    }

    #[test]
    fn timeout_never_degenerates_to_zero() {
        let args = fetch_arguments("curl", "https://wayscriber.com/latest.json", Duration::ZERO);
        assert!(args.windows(2).any(|pair| pair == ["--max-time", "1"]));
    }

    #[test]
    fn body_validation_guards_size_and_encoding() {
        assert_eq!(
            validate_body(b"{}").expect("UTF-8 fixture body is within the manifest cap"),
            "{}"
        );
        assert!(validate_body(&vec![b'x'; MAX_MANIFEST_BYTES + 1]).is_err());
        assert!(validate_body(&[0xff, 0xfe]).is_err());
    }

    #[test]
    fn an_installed_client_that_fails_is_not_retried_with_another() {
        let mut calls = Vec::new();
        let err = fetch_with(
            "https://wayscriber.com/latest.json",
            Duration::ZERO,
            |program, _, _| {
                calls.push(program.to_string());
                Err(FetchError::Failed("offline".to_string()))
            },
        )
        .expect_err("an installed client failure should end the check");
        assert_eq!(err, "offline");
        assert_eq!(calls, ["curl"]);
    }

    #[test]
    fn a_missing_client_falls_through_to_the_next_one() {
        let mut calls = Vec::new();
        let body = fetch_with(
            "https://wayscriber.com/latest.json",
            Duration::ZERO,
            |program, _, _| {
                calls.push(program.to_string());
                if program == "curl" {
                    Err(FetchError::Unavailable)
                } else {
                    Ok("{}".to_string())
                }
            },
        )
        .expect("missing-curl fixture falls through to its successful wget transport");
        assert_eq!(body, "{}");
        assert_eq!(calls, ["curl", "wget"]);
    }
}
