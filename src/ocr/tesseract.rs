//! Production adapters: local Tesseract for recognition, `wl-copy` for publication.

use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use crate::process_broker::{HelperKind, STDOUT_CAP_EXCEEDED};

use super::{
    OcrFailure, OcrLanguages, OcrTextPublisher, RecognizedOutput, RecognizedText, TextRecognizer,
};

pub(super) const TESSERACT_PROGRAM: &str = "tesseract";
const TESSERACT_TIMEOUT: Duration = Duration::from_secs(30);
/// Recognized text is plain UTF-8; 4 MiB is far past any plausible screen
/// region and keeps a runaway engine from filling memory.
pub(super) const TESSERACT_STDOUT_CAP: usize = 4 * 1024 * 1024;

pub(crate) struct TesseractRecognizer;

impl TextRecognizer for TesseractRecognizer {
    fn recognize(
        &self,
        png: &[u8],
        languages: &OcrLanguages,
    ) -> Result<RecognizedOutput, OcrFailure> {
        if !program_on_path(TESSERACT_PROGRAM) {
            return Err(OcrFailure::EngineMissing);
        }

        with_temporary_png(png, |input| run_tesseract(input, languages))
    }
}

/// Run one OCR operation with a securely created PNG that cannot outlive the
/// stack frame, including while unwinding from a panic.
pub(super) fn with_temporary_png<T>(
    png: &[u8],
    operation: impl FnOnce(&Path) -> Result<T, OcrFailure>,
) -> Result<T, OcrFailure> {
    with_temporary_png_in(png, &std::env::temp_dir(), operation)
}

fn with_temporary_png_in<T>(
    png: &[u8],
    directory: &Path,
    operation: impl FnOnce(&Path) -> Result<T, OcrFailure>,
) -> Result<T, OcrFailure> {
    // A file rather than broker stdin because the broker's input cap is 16 MiB
    // and a lossless desktop crop can exceed it. NamedTempFile's Drop removes
    // the path during unwinding; explicit close makes ordinary cleanup errors
    // observable before the operation returns.
    let mut input = tempfile::Builder::new()
        .prefix("wayscriber-ocr-")
        .suffix(".png")
        .tempfile_in(directory)
        .map_err(|err| {
            log::warn!("OCR temporary file creation failed: {err}");
            OcrFailure::TemporaryFileFailed
        })?;
    input
        .write_all(png)
        .and_then(|()| input.as_file_mut().sync_all())
        .map_err(|err| {
            log::warn!("OCR temporary file write failed: {err}");
            OcrFailure::TemporaryFileFailed
        })?;

    let output = operation(input.path());
    if let Err(err) = input.close() {
        log::warn!("OCR temporary file cleanup failed: {err}");
    }
    output
}

fn run_tesseract(input: &Path, languages: &OcrLanguages) -> Result<RecognizedOutput, OcrFailure> {
    let output = crate::process_broker::current()
        .and_then(|broker| {
            broker.run(
                HelperKind::Tesseract,
                OsStr::new(TESSERACT_PROGRAM),
                tesseract_arguments(input, languages),
                Vec::new(),
                TESSERACT_TIMEOUT,
                TESSERACT_STDOUT_CAP,
            )
        })
        .map_err(|err| {
            log::warn!("Failed to run tesseract: {err:#}");
            classify_broker_error(&err)
        })?;

    if output.timed_out {
        return Err(OcrFailure::TimedOut);
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Engine detail is a debug/warning concern; the user sees a stable
        // category message instead.
        log::warn!(
            "tesseract exited with status {}: {}",
            output.status,
            stderr.trim()
        );
        return Err(classify_stderr(&stderr, languages));
    }

    // Never log stdout: it is the recognized screen content.
    let replaced_invalid_utf8 = std::str::from_utf8(&output.stdout).is_err();
    let text = RecognizedText::trimmed(&String::from_utf8_lossy(&output.stdout));
    Ok(RecognizedOutput {
        text,
        replaced_invalid_utf8,
    })
}

/// The invocation itself: an explicit argument vector, never a shell line.
fn tesseract_arguments<'a>(input: &'a Path, languages: &'a OcrLanguages) -> Vec<&'a OsStr> {
    vec![
        input.as_os_str(),
        OsStr::new("stdout"),
        OsStr::new("--oem"),
        OsStr::new("1"),
        OsStr::new("--psm"),
        OsStr::new("6"),
        OsStr::new("-l"),
        OsStr::new(languages.as_str()),
        OsStr::new("--dpi"),
        OsStr::new("300"),
        OsStr::new("-c"),
        OsStr::new("preserve_interword_spaces=1"),
    ]
}

/// A run that outgrows the stdout cap is rejected by the broker rather than
/// truncated, so "too much text" arrives as a transport error and has to be
/// separated from a broker that is simply unavailable.
pub(super) fn classify_broker_error(error: &anyhow::Error) -> OcrFailure {
    if format!("{error:#}").contains(STDOUT_CAP_EXCEEDED) {
        OcrFailure::OutputTooLarge
    } else {
        OcrFailure::EngineUnavailable
    }
}

pub(super) fn classify_stderr(stderr: &str, languages: &OcrLanguages) -> OcrFailure {
    let lowered = stderr.to_ascii_lowercase();
    if lowered.contains("failed loading language")
        || lowered.contains("could not load any languages")
        || lowered.contains("couldn't load any languages")
        || lowered.contains("error opening data file")
    {
        OcrFailure::LanguageMissing {
            languages: languages.as_str().to_string(),
        }
    } else {
        OcrFailure::EngineFailed
    }
}

/// Whether `program` resolves to an executable file on `PATH`.
///
/// Checked before invoking so "not installed" is its own actionable message
/// rather than a generic spawn failure.
pub(super) fn program_on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| program_in_search_path(program, &path))
}

/// The search itself, over an explicit path list. Split out so tests can probe
/// a temporary directory without mutating the process-wide `PATH`, which every
/// concurrently running test shares.
fn program_in_search_path(program: &str, search_path: &OsStr) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::env::split_paths(search_path).any(|directory| {
        if directory.as_os_str().is_empty() {
            return false;
        }
        std::fs::metadata(directory.join(program))
            .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    })
}

pub(crate) struct WlCopyPublisher;

impl OcrTextPublisher for WlCopyPublisher {
    fn publish(&self, text: &str) -> Result<(), OcrFailure> {
        crate::clipboard_text::copy_text_via_command(text).map_err(|err| {
            log::warn!("wl-copy failed for recognized text: {err}");
            OcrFailure::ClipboardFailed
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn languages() -> OcrLanguages {
        OcrLanguages::from_validated("eng+deu".to_string())
    }

    #[test]
    fn invocation_matches_the_documented_argument_vector() {
        let path = Path::new("/tmp/wayscriber-ocr-test.png");
        let arguments: Vec<_> = tesseract_arguments(path, &languages())
            .into_iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            arguments,
            [
                "/tmp/wayscriber-ocr-test.png",
                "stdout",
                "--oem",
                "1",
                "--psm",
                "6",
                "-l",
                "eng+deu",
                "--dpi",
                "300",
                "-c",
                "preserve_interword_spaces=1",
            ]
        );
    }

    #[test]
    fn missing_language_data_is_distinguished_from_a_generic_engine_failure() {
        assert_eq!(
            classify_stderr("Failed loading language 'deu'\n", &languages()),
            OcrFailure::LanguageMissing {
                languages: "eng+deu".to_string(),
            }
        );
        assert_eq!(
            classify_stderr(
                "Error opening data file /usr/share/tessdata/deu.traineddata",
                &languages()
            ),
            OcrFailure::LanguageMissing {
                languages: "eng+deu".to_string(),
            }
        );
        assert_eq!(
            classify_stderr("Image too large to process", &languages()),
            OcrFailure::EngineFailed
        );
    }

    #[test]
    fn path_probe_accepts_only_executable_files() {
        use std::os::unix::fs::PermissionsExt;

        let directory = crate::test_temp::tempdir().unwrap();
        let program = directory.path().join("wayscriber-fake-ocr");
        std::fs::write(&program, b"#!/bin/sh\n").unwrap();
        let search_path = directory.path().as_os_str();

        assert!(
            !program_in_search_path("wayscriber-fake-ocr", search_path),
            "a non-executable file is not a usable engine"
        );

        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(program_in_search_path("wayscriber-fake-ocr", search_path));
        assert!(!program_in_search_path(
            "wayscriber-absent-ocr",
            search_path
        ));
    }

    #[test]
    fn path_probe_skips_empty_entries_and_searches_every_directory() {
        use std::os::unix::fs::PermissionsExt;

        let directory = crate::test_temp::tempdir().unwrap();
        let program = directory.path().join("wayscriber-fake-ocr");
        std::fs::write(&program, b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();

        // An empty entry means "current directory" to some shells; treating it
        // as one would make the probe depend on the working directory.
        let search_path =
            std::env::join_paths(["".as_ref(), Path::new("/nonexistent"), directory.path()])
                .unwrap();

        assert!(program_in_search_path("wayscriber-fake-ocr", &search_path));
    }

    #[test]
    fn an_oversized_result_is_distinguished_from_an_unavailable_broker() {
        assert_eq!(
            classify_broker_error(&anyhow::anyhow!(STDOUT_CAP_EXCEEDED)),
            OcrFailure::OutputTooLarge
        );
        assert_eq!(
            classify_broker_error(
                &anyhow::anyhow!(STDOUT_CAP_EXCEEDED).context("process broker rejected request")
            ),
            OcrFailure::OutputTooLarge
        );
        assert_eq!(
            classify_broker_error(&anyhow::anyhow!("runtime process broker is not active")),
            OcrFailure::EngineUnavailable
        );
    }

    #[test]
    fn temporary_input_is_removed_after_success() {
        let directory = crate::test_temp::tempdir().unwrap();

        let result = with_temporary_png_in(b"png", directory.path(), |path| {
            assert!(path.exists());
            assert_eq!(std::fs::read(path).unwrap(), b"png");
            Ok("recognized")
        });

        assert_eq!(result.unwrap(), "recognized");
        assert!(temporary_ocr_files(directory.path()).is_empty());
    }

    #[test]
    fn temporary_input_is_removed_after_operation_failure() {
        let directory = crate::test_temp::tempdir().unwrap();

        let result = with_temporary_png_in(b"png", directory.path(), |path| {
            assert!(path.exists());
            Err::<(), _>(OcrFailure::EngineFailed)
        });

        assert!(matches!(result, Err(OcrFailure::EngineFailed)));
        assert!(temporary_ocr_files(directory.path()).is_empty());
    }

    #[test]
    fn temporary_input_is_removed_while_unwinding_from_a_panic() {
        let directory = crate::test_temp::tempdir().unwrap();

        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), OcrFailure> =
                with_temporary_png_in(b"png", directory.path(), |path| {
                    assert!(path.exists());
                    panic!("simulated OCR adapter panic");
                });
        }));

        assert!(panic.is_err());
        assert!(temporary_ocr_files(directory.path()).is_empty());
    }

    fn temporary_ocr_files(directory: &Path) -> Vec<std::path::PathBuf> {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut paths: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("wayscriber-ocr-"))
            })
            .collect();
        paths.sort();
        paths
    }
}
