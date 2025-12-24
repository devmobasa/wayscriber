//! File saving functionality for screenshots.

use super::types::CaptureError;
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
    // Ensure directory exists
    let directory = ensure_directory_exists(&config.save_directory)?;

    // Generate filename
    let filename = generate_filename(&config.filename_template, &config.format);
    let file_path = directory.join(&filename);

    log::info!(
        "Saving screenshot to: {} ({} bytes)",
        file_path.display(),
        image_data.len()
    );

    // Write file
    fs::write(&file_path, image_data)?;

    // Verify the write
    let written_size = fs::metadata(&file_path)?.len();
    log::debug!("File written: {} bytes", written_size);

    // Set permissions to user read/write only (security)
    #[cfg(unix)]
    {
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file_path, Permissions::from_mode(0o600))?;
    }

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
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("nested").join("shots");

        let resolved = ensure_directory_exists(&target).expect("ensure_directory_exists");
        assert!(target.exists());
        assert_eq!(resolved, target.canonicalize().unwrap());
    }
}
