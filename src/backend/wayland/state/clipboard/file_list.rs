use super::{
    CLIPBOARD_READ_TIMEOUT, ClipboardPasteResult, ClipboardReadError, MAX_CLIPBOARD_IMAGE_BYTES,
    decode_clipboard_image, read_pipe_with_timeout,
};
use crate::file_uri;
#[cfg(unix)]
use std::fs::OpenOptions;
use std::fs::{self, File};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const GNOME_COPIED_FILES_MIME: &str = "x-special/gnome-copied-files";
const TEXT_URI_LIST_MIME: &str = "text/uri-list";
const OCTET_STREAM_MIME: &str = "application/octet-stream";

pub(super) fn decode_clipboard_uri_list(
    mime_type: &str,
    bytes: Vec<u8>,
    offered: Vec<String>,
) -> ClipboardPasteResult {
    let uris = match parse_clipboard_file_uris(mime_type, &bytes) {
        Ok(uris) if uris.is_empty() => return ClipboardPasteResult::NoSupportedMime { offered },
        Ok(uris) => uris,
        Err(err) => return ClipboardPasteResult::DecodeFailed(err),
    };

    let mut saw_local_file = false;
    let mut last_decode_error = None;
    for uri in uris {
        let path = match file_uri::decode_file_uri(&uri) {
            Ok(path) => path,
            Err(err) => {
                log::debug!("Ignoring unsupported clipboard file URI '{}': {}", uri, err);
                continue;
            }
        };
        saw_local_file = true;

        let image_bytes = match read_clipboard_file(&path) {
            Ok(bytes) if bytes.is_empty() => {
                last_decode_error = Some(format!("clipboard file {} is empty", path.display()));
                continue;
            }
            Ok(bytes) => bytes,
            Err(err) => return map_clipboard_file_read_error(&path, err),
        };

        match decode_clipboard_image(OCTET_STREAM_MIME, image_bytes) {
            ClipboardPasteResult::DecodeFailed(err) => {
                last_decode_error = Some(format!(
                    "clipboard file {} is not a supported image: {}",
                    path.display(),
                    err
                ));
            }
            result => return result,
        }
    }

    if saw_local_file {
        ClipboardPasteResult::DecodeFailed(
            last_decode_error.unwrap_or_else(|| "no supported image file in URI list".to_string()),
        )
    } else {
        ClipboardPasteResult::NoSupportedMime { offered }
    }
}

pub(super) fn is_uri_list_mime(mime_type: &str) -> bool {
    let mime_type = mime_type.to_ascii_lowercase();
    is_gnome_copied_files_mime(&mime_type)
        || mime_type == TEXT_URI_LIST_MIME
        || mime_type.starts_with("text/uri-list;")
}

fn parse_clipboard_file_uris(mime_type: &str, bytes: &[u8]) -> Result<Vec<String>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|err| format!("clipboard URI list is not UTF-8: {err}"))?;
    let is_gnome = is_gnome_copied_files_mime(mime_type);
    let mut uris = Vec::new();

    for (index, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if is_gnome && index == 0 && matches!(line, "copy" | "cut") {
            continue;
        }
        uris.push(line.to_string());
    }

    Ok(uris)
}

fn read_clipboard_file(path: &Path) -> Result<Vec<u8>, ClipboardReadError> {
    ensure_regular_clipboard_path(path)?;
    let file = open_regular_clipboard_file(path)?;
    read_pipe_with_timeout(file, MAX_CLIPBOARD_IMAGE_BYTES, CLIPBOARD_READ_TIMEOUT)
}

fn map_clipboard_file_read_error(path: &Path, err: ClipboardReadError) -> ClipboardPasteResult {
    match err {
        ClipboardReadError::TooLarge { limit } => ClipboardPasteResult::TooLarge { limit },
        ClipboardReadError::Empty => ClipboardPasteResult::DecodeFailed(format!(
            "clipboard file {} is empty",
            path.display()
        )),
        ClipboardReadError::TimedOut => ClipboardPasteResult::DecodeFailed(format!(
            "clipboard file {} read timed out",
            path.display()
        )),
        ClipboardReadError::Unavailable(err) | ClipboardReadError::Other(err) => {
            ClipboardPasteResult::DecodeFailed(format!(
                "clipboard file {} could not be read: {}",
                path.display(),
                err
            ))
        }
    }
}

fn ensure_regular_clipboard_path(path: &Path) -> Result<(), ClipboardReadError> {
    let metadata = fs::metadata(path).map_err(|err| {
        ClipboardReadError::Other(format!(
            "Failed to inspect clipboard file {}: {}",
            path.display(),
            err
        ))
    })?;
    validate_clipboard_file_metadata(&metadata, path)
}

fn validate_clipboard_file_metadata(
    metadata: &fs::Metadata,
    path: &Path,
) -> Result<(), ClipboardReadError> {
    if !metadata.file_type().is_file() {
        return Err(ClipboardReadError::Other(format!(
            "Clipboard file {} is not a regular file",
            path.display()
        )));
    }
    if metadata.len() > MAX_CLIPBOARD_IMAGE_BYTES as u64 {
        return Err(ClipboardReadError::TooLarge {
            limit: MAX_CLIPBOARD_IMAGE_BYTES,
        });
    }
    Ok(())
}

#[cfg(unix)]
fn open_regular_clipboard_file(path: &Path) -> Result<File, ClipboardReadError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|err| {
            ClipboardReadError::Other(format!(
                "Failed to open clipboard file {}: {}",
                path.display(),
                err
            ))
        })?;
    ensure_regular_clipboard_file(&file, path)?;
    Ok(file)
}

#[cfg(not(unix))]
fn open_regular_clipboard_file(path: &Path) -> Result<File, ClipboardReadError> {
    let file = File::open(path).map_err(|err| {
        ClipboardReadError::Other(format!(
            "Failed to open clipboard file {}: {}",
            path.display(),
            err
        ))
    })?;
    ensure_regular_clipboard_file(&file, path)?;
    Ok(file)
}

fn ensure_regular_clipboard_file(file: &File, path: &Path) -> Result<(), ClipboardReadError> {
    let metadata = file.metadata().map_err(|err| {
        ClipboardReadError::Other(format!(
            "Failed to inspect clipboard file {}: {}",
            path.display(),
            err
        ))
    })?;
    validate_clipboard_file_metadata(&metadata, path)
}

fn is_gnome_copied_files_mime(mime_type: &str) -> bool {
    mime_type.eq_ignore_ascii_case(GNOME_COPIED_FILES_MIME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::state::clipboard::choose_supported_mime;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn choose_supported_mime_accepts_file_manager_uri_lists() {
        let offered = vec![
            "text/plain".to_string(),
            GNOME_COPIED_FILES_MIME.to_string(),
            "text/uri-list;charset=utf-8".to_string(),
        ];

        assert_eq!(
            choose_supported_mime(&offered),
            Some(GNOME_COPIED_FILES_MIME.to_string())
        );
    }

    #[test]
    fn gnome_copied_files_parser_ignores_copy_action() {
        let uris = parse_clipboard_file_uris(
            GNOME_COPIED_FILES_MIME,
            b"copy\nfile:///tmp/cat.jpg\nfile:///tmp/dog.png\n",
        )
        .expect("parse URI list");

        assert_eq!(uris, vec!["file:///tmp/cat.jpg", "file:///tmp/dog.png"]);
    }

    #[test]
    fn uri_list_paste_decodes_copied_image_file_without_deleting_it() {
        let temp = TempDir::new().unwrap();
        let image_path = temp.path().join("cat.png");
        fs::write(&image_path, tiny_png()).unwrap();
        let uri = file_uri_for_path(&image_path);
        let offered = vec![TEXT_URI_LIST_MIME.to_string()];

        let result = decode_clipboard_uri_list(TEXT_URI_LIST_MIME, uri.into_bytes(), offered);

        match result {
            ClipboardPasteResult::Image(image) => {
                assert_eq!(image.mime_type, "image/png");
                assert_eq!(image.width, 1);
                assert_eq!(image.height, 1);
            }
            other => panic!("expected image result, got {other:?}"),
        }
        assert!(
            image_path.exists(),
            "clipboard paste must not delete copied files"
        );
    }

    #[test]
    #[cfg(unix)]
    fn uri_list_paste_rejects_fifo_without_blocking() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let temp = TempDir::new().unwrap();
        let fifo_path = temp.path().join("not-an-image");
        let c_path = CString::new(fifo_path.as_os_str().as_bytes()).unwrap();
        let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
        assert_eq!(
            result,
            0,
            "mkfifo failed: {}",
            std::io::Error::last_os_error()
        );
        let uri = file_uri_for_path(&fifo_path);
        let offered = vec![TEXT_URI_LIST_MIME.to_string()];

        let result = decode_clipboard_uri_list(TEXT_URI_LIST_MIME, uri.into_bytes(), offered);

        match result {
            ClipboardPasteResult::DecodeFailed(err) => {
                assert!(err.contains("not a regular file"));
            }
            other => panic!("expected decode failure, got {other:?}"),
        }
    }

    #[test]
    fn uri_list_paste_treats_missing_file_as_decode_failure() {
        let temp = TempDir::new().unwrap();
        let image_path = temp.path().join("missing.png");
        let uri = file_uri_for_path(&image_path);
        let offered = vec![TEXT_URI_LIST_MIME.to_string()];

        let result = decode_clipboard_uri_list(TEXT_URI_LIST_MIME, uri.into_bytes(), offered);

        match result {
            ClipboardPasteResult::DecodeFailed(err) => {
                assert!(err.contains("could not be read") || err.contains("Failed to inspect"));
            }
            other => panic!("expected decode failure, got {other:?}"),
        }
    }

    fn file_uri_for_path(path: &Path) -> String {
        format!(
            "file://{}",
            path.to_string_lossy()
                .replace('%', "%25")
                .replace(' ', "%20")
        )
    }

    fn tiny_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&[255, 0, 0, 255]).unwrap();
        }
        bytes
    }
}
