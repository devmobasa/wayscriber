#![cfg(feature = "portal")]

use crate::capture::types::CaptureError;
use crate::env_vars::XDG_RUNTIME_DIR_ENV;
use crate::file_uri;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{thread, time::Duration};

const MAX_PORTAL_IMAGE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ATTEMPTS: usize = 60;
const ATTEMPT_DELAY_MS: u64 = 50;

/// Read image data from a portal `file://` URI.
///
/// The path must live under `$XDG_RUNTIME_DIR` or the xdg-desktop-portal cache
/// directory. The file is opened without following symlinks, must be a regular
/// file, and is deleted only after those checks pass.
pub fn read_image_from_uri(uri: &str) -> Result<Vec<u8>, CaptureError> {
    read_image_from_uri_with_limit(uri, MAX_PORTAL_IMAGE_BYTES)
}

fn read_image_from_uri_with_limit(uri: &str, max_bytes: u64) -> Result<Vec<u8>, CaptureError> {
    let path = decode_file_uri(uri)?;
    if !is_allowed_portal_path(&path) {
        return Err(CaptureError::InvalidResponse(
            "Portal screenshot path is not in a trusted directory".to_string(),
        ));
    }

    log::debug!("Reading screenshot from the portal temporary file");

    let mut opened = None;
    for attempt in 0..MAX_ATTEMPTS {
        match open_portal_image(&path, max_bytes) {
            Ok(file) => {
                opened = Some(file);
                break;
            }
            Err(PortalOpenError::Rejected { message, delete }) => {
                if delete {
                    remove_allowed_portal_file(&path);
                }
                return Err(CaptureError::ImageError(message));
            }
            Err(PortalOpenError::NotReady(err)) if attempt + 1 == MAX_ATTEMPTS => {
                return Err(CaptureError::ImageError(format!(
                    "Portal screenshot file was not ready after {MAX_ATTEMPTS} attempts: {err}"
                )));
            }
            Err(PortalOpenError::NotReady(err)) => {
                log::trace!(
                    "Portal screenshot file not ready yet (attempt {}/{MAX_ATTEMPTS}): {err}",
                    attempt + 1
                );
            }
        }
        thread::sleep(Duration::from_millis(ATTEMPT_DELAY_MS));
    }

    let file = opened.ok_or_else(|| {
        CaptureError::ImageError("Portal screenshot file was not ready".to_string())
    })?;
    let mut data = Vec::new();
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut data)
        .map_err(|err| {
            CaptureError::ImageError(format!("Failed to read portal screenshot: {err}"))
        })?;
    if data.is_empty() {
        return Err(CaptureError::ImageError(
            "Portal screenshot file was empty".to_string(),
        ));
    }
    if data.len() as u64 > max_bytes {
        remove_allowed_portal_file(&path);
        return Err(CaptureError::ImageError(
            "Portal screenshot file exceeds the size limit".to_string(),
        ));
    }

    log::info!(
        "Successfully read {} bytes from portal screenshot",
        data.len()
    );
    remove_allowed_portal_file(&path);
    Ok(data)
}

fn remove_allowed_portal_file(path: &Path) {
    if let Err(err) = fs::remove_file(path) {
        log::warn!("Failed to remove portal screenshot temporary file: {err}");
    } else {
        log::debug!("Removed portal screenshot temporary file");
    }
}

fn is_allowed_portal_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !crate::paths::is_single_path_component(name) {
        return false;
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(parent) = parent.canonicalize() else {
        return false;
    };
    allowed_portal_roots().into_iter().any(|root| {
        root.canonicalize()
            .is_ok_and(|root| parent.starts_with(root))
    })
}

fn allowed_portal_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(runtime) = std::env::var_os(XDG_RUNTIME_DIR_ENV)
        && !runtime.is_empty()
    {
        roots.push(PathBuf::from(runtime));
    }
    if let Some(cache) = crate::paths::cache_dir() {
        roots.push(cache.join("xdg-desktop-portal"));
    }
    roots
}

enum PortalOpenError {
    NotReady(String),
    Rejected { message: String, delete: bool },
}

fn open_portal_image(path: &Path, max_bytes: u64) -> Result<File, PortalOpenError> {
    let file = open_portal_file(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            PortalOpenError::NotReady(err.to_string())
        } else {
            PortalOpenError::Rejected {
                message: err.to_string(),
                delete: false,
            }
        }
    })?;
    let metadata = file.metadata().map_err(|err| PortalOpenError::Rejected {
        message: err.to_string(),
        delete: false,
    })?;
    if !metadata.file_type().is_file() {
        return Err(PortalOpenError::Rejected {
            message: "portal screenshot is not a regular file".to_string(),
            delete: false,
        });
    }
    if metadata.len() > max_bytes {
        return Err(PortalOpenError::Rejected {
            message: "portal screenshot file exceeds the size limit".to_string(),
            delete: true,
        });
    }
    if metadata.len() == 0 {
        return Err(PortalOpenError::NotReady(
            "portal screenshot file is empty".to_string(),
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn open_portal_file(path: &Path) -> std::io::Result<File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
}

#[cfg(not(unix))]
fn open_portal_file(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

fn decode_file_uri(uri: &str) -> Result<PathBuf, CaptureError> {
    file_uri::decode_file_uri(uri).map_err(CaptureError::InvalidResponse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_vars::XDG_RUNTIME_DIR_ENV;
    use crate::test_temp::TempDir;

    fn file_uri_for(path: &Path) -> String {
        format!(
            "file://{}",
            path.to_string_lossy()
                .replace('%', "%25")
                .replace(' ', "%20")
        )
    }

    fn with_runtime_dir<T>(dir: &Path, body: impl FnOnce() -> T) -> T {
        crate::test_env::with_env_var(XDG_RUNTIME_DIR_ENV, Some(dir.as_os_str()), body)
    }

    #[test]
    fn reads_and_removes_file() {
        let temp = TempDir::new().unwrap();
        with_runtime_dir(temp.path(), || {
            let file_path = temp.path().join("capture file.png");
            std::fs::write(&file_path, b"portal-bytes").unwrap();

            let data = read_image_from_uri(&file_uri_for(&file_path)).expect("read succeeds");
            assert_eq!(data, b"portal-bytes");
            assert!(
                !file_path.exists(),
                "read_image_from_uri should delete the portal temp file"
            );
        });
    }

    #[test]
    fn rejects_paths_outside_trusted_directories() {
        let temp = TempDir::new().unwrap();
        let outside = temp.path().join("outside.png");
        std::fs::write(&outside, b"secret").unwrap();
        let trusted = temp.path().join("runtime");
        std::fs::create_dir(&trusted).unwrap();

        with_runtime_dir(&trusted, || {
            let err = read_image_from_uri(&file_uri_for(&outside)).expect_err("outside path");
            match err {
                CaptureError::InvalidResponse(msg) => {
                    assert!(msg.contains("trusted directory"), "{msg}");
                }
                other => panic!("unexpected error variant: {other:?}"),
            }
            assert!(
                outside.exists(),
                "rejected portal URIs must not delete the target file"
            );
        });
    }

    #[test]
    fn rejects_files_over_the_size_limit_and_still_deletes_the_temp() {
        let temp = TempDir::new().unwrap();
        with_runtime_dir(temp.path(), || {
            let file_path = temp.path().join("huge.png");
            std::fs::write(&file_path, b"12345").unwrap();
            let err = read_image_from_uri_with_limit(&file_uri_for(&file_path), 4)
                .expect_err("oversize file");
            match err {
                CaptureError::ImageError(msg) => {
                    assert!(msg.contains("size limit"), "{msg}");
                }
                other => panic!("unexpected error variant: {other:?}"),
            }
            assert!(
                !file_path.exists(),
                "an oversized portal temp file should still be removed"
            );
        });
    }

    #[test]
    #[cfg(unix)]
    fn rejects_symlinks_without_following_them() {
        let temp = TempDir::new().unwrap();
        with_runtime_dir(temp.path(), || {
            let target = temp.path().join("target.png");
            std::fs::write(&target, b"secret").unwrap();
            let link = temp.path().join("link.png");
            std::os::unix::fs::symlink(&target, &link).unwrap();

            read_image_from_uri(&file_uri_for(&link)).expect_err("symlink");
            assert!(target.exists());
            assert!(
                link.exists(),
                "failed symlink reads must not delete the link"
            );
        });
    }

    #[test]
    fn decode_file_uri_maps_errors_for_portal_reader() {
        let err = decode_file_uri("http://example.com/file.png").expect_err("expected error");
        match err {
            CaptureError::InvalidResponse(msg) => assert!(msg.contains("Invalid file URI")),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn allowed_portal_path_requires_runtime_or_cache_root() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("shot.png");
        std::fs::write(&file_path, b"x").unwrap();
        with_runtime_dir(temp.path(), || {
            assert!(is_allowed_portal_path(&file_path));
        });
        crate::test_env::with_env_var(XDG_RUNTIME_DIR_ENV, None, || {
            assert!(!is_allowed_portal_path(&file_path));
        });
    }
}
