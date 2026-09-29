//! File saving functionality for screenshots.

use super::types::CaptureError;
use crate::durable_io::{
    AtomicWriteOptions, DurableIoError, OverwriteMode, PermissionPolicy, SymlinkPolicy,
};
use crate::paths::{expand_tilde as expand_tilde_global, home_dir, pictures_dir};
use crate::time_utils::{format_with_template, now_local};
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration for file saving.
#[derive(Debug, Clone)]
pub struct FileSaveConfig {
    /// Directory to save screenshots to.
    pub save_directory: PathBuf,
    /// Filename template (strftime-like: %Y, %m, %d, %H, %M, %S).
    pub filename_template: String,
    /// Image format extension.
    pub format: String,
}

impl Default for FileSaveConfig {
    fn default() -> Self {
        Self {
            save_directory: pictures_dir()
                .or_else(|| home_dir().map(|home| home.join("Pictures")))
                .unwrap_or_else(|| PathBuf::from("~"))
                .join("Wayscriber"),
            filename_template: "screenshot_%Y-%m-%d_%H%M%S".to_string(),
            format: "png".to_string(),
        }
    }
}

/// Generate a filename based on the template and current time.
///
/// # Arguments
/// * `template` - Template string with `%Y`, `%m`, `%d`, `%H`, `%M`, `%S`, `%%`
/// * `format` - File extension (e.g., "png")
///
/// # Returns
/// Generated filename with extension
pub fn generate_filename(template: &str, format: &str) -> String {
    let now = now_local();
    let filename = format_with_template(now, template);
    format!("{}.{}", filename, format)
}

fn sanitize_save_extension(format: &str) -> Option<String> {
    let normalized = format.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "png" | "jpg" | "jpeg" | "pdf").then_some(normalized)
}

fn save_file_name(template: &str, format: &str) -> Result<String, CaptureError> {
    let stem = format_with_template(now_local(), template);
    let extension = sanitize_save_extension(format).ok_or_else(|| {
        CaptureError::SaveError(std::io::Error::other("unsupported screenshot file format"))
    })?;
    if !crate::paths::is_single_path_component(&stem) {
        return Err(CaptureError::SaveError(std::io::Error::other(
            "filename template must expand to a single file name",
        )));
    }
    let filename = format!("{stem}.{extension}");
    if !crate::paths::is_single_path_component(&filename) {
        return Err(CaptureError::SaveError(std::io::Error::other(
            "filename template must expand to a single file name",
        )));
    }
    Ok(filename)
}

const UNIQUE_NAME_ATTEMPTS: u32 = 100;

/// Write a new file named from `template` in `directory`, never replacing one.
///
/// The template's name is tried first, then `-1` … `-100` suffixes. Names
/// already on disk are skipped cheaply, and `write_new` must refuse an
/// existing destination with [`DurableIoError::AlreadyExists`], so a name
/// taken between that check and the write moves on to the next suffix rather
/// than overwriting another screenshot.
fn save_to_free_path(
    directory: &Path,
    template: &str,
    format: &str,
    mut write_new: impl FnMut(&Path) -> Result<(), DurableIoError>,
) -> Result<PathBuf, CaptureError> {
    let filename = save_file_name(template, format)?;
    let base = Path::new(&filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            CaptureError::SaveError(std::io::Error::other(
                "filename template must expand to a single file name",
            ))
        })?;
    let extension = Path::new(&filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| {
            CaptureError::SaveError(std::io::Error::other("unsupported screenshot file format"))
        })?;
    if directory.join(&filename).parent() != Some(directory) {
        return Err(CaptureError::SaveError(std::io::Error::other(
            "filename template must expand to a single file name",
        )));
    }

    let candidates = std::iter::once(filename.clone())
        .chain((1..=UNIQUE_NAME_ATTEMPTS).map(|suffix| format!("{base}-{suffix}.{extension}")));
    for candidate in candidates {
        let path = directory.join(candidate);
        if path.parent() != Some(directory) || fs::symlink_metadata(&path).is_ok() {
            continue;
        }

        match write_new(&path) {
            Ok(()) => return Ok(path),
            Err(DurableIoError::AlreadyExists { .. }) => {
                log::debug!(
                    "Screenshot filename was taken during the save; trying the next one: {}",
                    path.display()
                );
            }
            Err(err) => return Err(CaptureError::SaveError(std::io::Error::other(err))),
        }
    }

    Err(CaptureError::SaveError(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!(
            "{filename} and its {UNIQUE_NAME_ATTEMPTS} numbered alternatives already exist in {}",
            directory.display()
        ),
    )))
}

/// The error for a save that has no directory to write into.
pub(crate) fn no_save_directory_error() -> CaptureError {
    CaptureError::SaveError(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "no save directory configured",
    ))
}

/// Ensure the save directory exists, creating it if necessary.
///
/// # Arguments
/// * `directory` - Path to the directory
///
/// # Returns
/// The canonicalized path to the directory
pub fn ensure_directory_exists(directory: &Path) -> Result<PathBuf, CaptureError> {
    if !directory.exists() {
        log::info!("Creating screenshot directory: {}", directory.display());
        fs::create_dir_all(directory)?;
    }

    // Canonicalize to resolve ~ and relative paths
    let canonical = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_path_buf());

    Ok(canonical)
}

/// Save image data to a file.
///
/// # Arguments
/// * `image_data` - Raw image bytes (PNG format)
/// * `config` - File save configuration
///
/// # Returns
/// Path to the saved file
pub fn save_screenshot(
    image_data: &[u8],
    config: &FileSaveConfig,
) -> Result<PathBuf, CaptureError> {
    // An empty directory would resolve file names against the working
    // directory of the process.
    if config.save_directory.as_os_str().is_empty() {
        return Err(no_save_directory_error());
    }

    // Ensure directory exists
    let directory = ensure_directory_exists(&config.save_directory)?;

    // Write under the first free file name
    let file_path = save_to_free_path(
        &directory,
        &config.filename_template,
        &config.format,
        |path| {
            log::info!(
                "Saving screenshot to: {} ({} bytes)",
                path.display(),
                image_data.len()
            );
            crate::durable_io::write_atomic(
                path,
                image_data,
                AtomicWriteOptions {
                    overwrite: OverwriteMode::CreateNew,
                    permissions: PermissionPolicy::FixedMode(0o600),
                    symlink: SymlinkPolicy::Reject,
                    sync_file: true,
                    sync_parent: true,
                },
            )
        },
    )?;

    // Verify the write
    let written_size = fs::metadata(&file_path)?.len();
    log::debug!("File written: {} bytes", written_size);

    log::info!("Screenshot saved successfully: {}", file_path.display());

    Ok(file_path)
}

/// Expand tilde (~) in path strings.
pub fn expand_tilde(path: &str) -> PathBuf {
    expand_tilde_global(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_filename() {
        let filename = generate_filename("test_%Y%m%d", "png");
        assert!(filename.starts_with("test_"));
        assert!(filename.ends_with(".png"));
        // Check that it contains a valid date (4 digits for year)
        assert!(filename.contains("202")); // Assuming we're in the 2020s
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/Pictures");
        assert!(!expanded.to_string_lossy().starts_with("~"));

        let no_tilde = expand_tilde("/absolute/path");
        assert_eq!(no_tilde, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_default_config() {
        let config = FileSaveConfig::default();
        assert_eq!(config.format, "png");
        assert!(
            config
                .save_directory
                .to_string_lossy()
                .contains("Wayscriber")
        );
    }

    #[test]
    fn ensure_directory_exists_creates_missing_path() {
        let temp = crate::test_temp::tempdir().unwrap();
        let target = temp.path().join("nested").join("shots");

        let resolved = ensure_directory_exists(&target).expect("ensure_directory_exists");
        assert!(target.exists());
        assert_eq!(resolved, target.canonicalize().unwrap());
    }

    #[test]
    fn save_file_name_rejects_path_escapes_and_unknown_formats() {
        for template in ["../evil", "foo/bar", "/tmp/x", "..", ".", ""] {
            assert!(
                save_file_name(template, "png").is_err(),
                "{template:?} must stay inside the save directory"
            );
        }
        assert!(save_file_name("shot", "png/../../x").is_err());
        assert!(save_file_name("shot", "exe").is_err());
        assert_eq!(save_file_name("shot", "png").unwrap(), "shot.png");
        assert_eq!(save_file_name("shot", "JPEG").unwrap(), "shot.jpeg");
    }

    #[test]
    fn save_to_free_path_stays_inside_the_save_directory() {
        let temp = crate::test_temp::tempdir().unwrap();
        let directory = temp.path();
        let path = save_to_free_path(directory, "shot", "png", |_| Ok(())).expect("safe name");
        assert_eq!(path.parent(), Some(directory));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("shot.png")
        );
        assert!(save_to_free_path(directory, "../evil", "png", |_| Ok(())).is_err());
    }

    #[test]
    fn save_to_free_path_moves_on_when_a_name_is_taken_during_the_write() {
        let temp = crate::test_temp::tempdir().unwrap();
        let directory = temp.path();
        let mut attempts = Vec::new();

        let path = save_to_free_path(directory, "shot", "png", |path| {
            attempts.push(path.to_path_buf());
            if attempts.len() == 1 {
                Err(DurableIoError::AlreadyExists {
                    path: path.to_path_buf(),
                })
            } else {
                Ok(())
            }
        })
        .expect("next free name");

        assert_eq!(path, directory.join("shot-1.png"));
        assert_eq!(
            attempts,
            vec![directory.join("shot.png"), directory.join("shot-1.png")]
        );
    }

    fn save_to(directory: &Path, bytes: &[u8]) -> Result<PathBuf, CaptureError> {
        save_screenshot(
            bytes,
            &FileSaveConfig {
                save_directory: directory.to_path_buf(),
                filename_template: "shot".to_string(),
                format: "png".to_string(),
            },
        )
    }

    #[test]
    fn save_screenshot_refuses_an_empty_save_directory() {
        let error = save_to(Path::new(""), b"image").expect_err("no directory");

        assert!(
            matches!(&error, CaptureError::SaveError(io) if io.kind() == std::io::ErrorKind::InvalidInput),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn save_screenshot_takes_the_next_free_suffix() {
        let temp = crate::test_temp::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap();
        fs::write(directory.join("shot.png"), b"first").unwrap();

        let path = save_to(&directory, b"second").expect("free suffix");

        assert_eq!(path, directory.join("shot-1.png"));
        assert_eq!(fs::read(directory.join("shot.png")).unwrap(), b"first");
        assert_eq!(fs::read(&path).unwrap(), b"second");
    }

    #[test]
    fn save_screenshot_fails_instead_of_overwriting_when_every_name_is_taken() {
        let temp = crate::test_temp::tempdir().unwrap();
        let directory = temp.path().canonicalize().unwrap();
        fs::write(directory.join("shot.png"), b"kept").unwrap();
        for suffix in 1..=UNIQUE_NAME_ATTEMPTS {
            fs::write(directory.join(format!("shot-{suffix}.png")), b"kept").unwrap();
        }

        let error = save_to(&directory, b"new").expect_err("no free name");

        assert!(
            matches!(&error, CaptureError::SaveError(io) if io.kind() == std::io::ErrorKind::AlreadyExists),
            "unexpected error: {error}"
        );
        assert_eq!(fs::read(directory.join("shot.png")).unwrap(), b"kept");
    }
}
