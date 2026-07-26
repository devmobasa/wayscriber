use super::{
    CLIPBOARD_READ_TIMEOUT, ClipboardPasteResult, ClipboardReadError, MAX_CLIPBOARD_IMAGE_BYTES,
    image::decode_clipboard_image,
};
use crate::file_uri;
use std::path::Path;

const GNOME_COPIED_FILES_MIME: &str = "x-special/gnome-copied-files";
const TEXT_URI_LIST_MIME: &str = "text/uri-list";
const OCTET_STREAM_MIME: &str = "application/octet-stream";

pub(super) fn decode_clipboard_uri_list(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    cancellation: &mut super::ClipboardCancellation,
    mime_type: &str,
    bytes: Vec<u8>,
    offered: Vec<String>,
) -> ClipboardPasteResult {
    decode_clipboard_uri_list_with_reader(mime_type, bytes, offered, |path| {
        if cancellation.is_cancelled() {
            return Err(ClipboardReadError::Other(
                "clipboard operation cancelled".to_string(),
            ));
        }
        let result = read_clipboard_file(process_broker, path);
        if cancellation.is_cancelled() {
            Err(ClipboardReadError::Other(
                "clipboard operation cancelled".to_string(),
            ))
        } else {
            result
        }
    })
}

fn decode_clipboard_uri_list_with_reader(
    mime_type: &str,
    bytes: Vec<u8>,
    offered: Vec<String>,
    mut read_file: impl FnMut(&Path) -> Result<Vec<u8>, ClipboardReadError>,
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

        let image_bytes = match read_file(&path) {
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

fn read_clipboard_file(
    process_broker: &crate::process_broker::ProcessBrokerHandle,
    path: &Path,
) -> Result<Vec<u8>, ClipboardReadError> {
    match process_broker.read_regular_file(path, MAX_CLIPBOARD_IMAGE_BYTES, CLIPBOARD_READ_TIMEOUT)
    {
        Ok(crate::process_broker::BrokerFileRead::Ready(bytes)) => Ok(bytes),
        Ok(crate::process_broker::BrokerFileRead::Empty) => Err(ClipboardReadError::Empty),
        Ok(crate::process_broker::BrokerFileRead::TooLarge { limit }) => {
            Err(ClipboardReadError::TooLarge { limit })
        }
        Ok(crate::process_broker::BrokerFileRead::NotRegular) => Err(ClipboardReadError::Other(
            "clipboard URI does not identify a regular, non-symlink file".to_string(),
        )),
        Ok(crate::process_broker::BrokerFileRead::TimedOut) => Err(ClipboardReadError::TimedOut),
        Ok(crate::process_broker::BrokerFileRead::ReadFailed { reason }) => {
            Err(ClipboardReadError::Other(reason))
        }
        Err(error) => Err(ClipboardReadError::Unavailable(format!(
            "clipboard file broker failed: {error:#}"
        ))),
    }
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

fn is_gnome_copied_files_mime(mime_type: &str) -> bool {
    mime_type.eq_ignore_ascii_case(GNOME_COPIED_FILES_MIME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::wayland::clipboard::image::choose_supported_mime;
    use crate::test_temp::TempDir;
    use std::fs;

    fn decode_fixture_uri_list(
        mime_type: &str,
        bytes: Vec<u8>,
        offered: Vec<String>,
    ) -> ClipboardPasteResult {
        decode_clipboard_uri_list_with_reader(mime_type, bytes, offered, |path| {
            let metadata = fs::symlink_metadata(path).map_err(|error| {
                ClipboardReadError::Other(format!("fixture file inspection failed: {error}"))
            })?;
            if !metadata.file_type().is_file() {
                return Err(ClipboardReadError::Other(
                    "fixture path is not a regular file".to_string(),
                ));
            }
            if metadata.len() > MAX_CLIPBOARD_IMAGE_BYTES as u64 {
                return Err(ClipboardReadError::TooLarge {
                    limit: MAX_CLIPBOARD_IMAGE_BYTES,
                });
            }
            fs::read(path).map_err(|error| {
                ClipboardReadError::Other(format!("fixture file read failed: {error}"))
            })
        })
    }

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
    fn uri_list_paste_decodes_copied_image_file_without_deleting_it() -> Result<(), String> {
        let temp = TempDir::new().expect("URI-list fixture creates an isolated directory");
        let image_path = temp.path().join("cat.png");
        fs::write(&image_path, tiny_png()).expect("URI-list fixture writes its image");
        let uri = file_uri_for_path(&image_path);
        let offered = vec![TEXT_URI_LIST_MIME.to_string()];

        let result = decode_fixture_uri_list(TEXT_URI_LIST_MIME, uri.into_bytes(), offered);

        let image = match result {
            ClipboardPasteResult::Image(image) => image,
            other => return Err(format!("URI-list fixture expected an image, got {other:?}")),
        };
        assert_eq!(image.mime_type, "image/png");
        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert!(
            image_path.exists(),
            "clipboard paste must not delete copied files"
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn uri_list_paste_rejects_fifo_without_blocking() -> Result<(), String> {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let temp = TempDir::new().expect("FIFO fixture creates an isolated directory");
        let fifo_path = temp.path().join("not-an-image");
        let c_path = CString::new(fifo_path.as_os_str().as_bytes())
            .expect("FIFO fixture path contains no interior NUL");
        let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
        assert_eq!(
            result,
            0,
            "mkfifo failed: {}",
            std::io::Error::last_os_error()
        );
        let uri = file_uri_for_path(&fifo_path);
        let offered = vec![TEXT_URI_LIST_MIME.to_string()];

        let result = decode_fixture_uri_list(TEXT_URI_LIST_MIME, uri.into_bytes(), offered);

        let error = match result {
            ClipboardPasteResult::DecodeFailed(error) => error,
            other => return Err(format!("FIFO fixture expected a failure, got {other:?}")),
        };
        assert!(error.contains("not a regular file"));
        Ok(())
    }

    #[test]
    fn uri_list_paste_treats_missing_file_as_decode_failure() -> Result<(), String> {
        let temp = TempDir::new().expect("missing-file fixture creates an isolated directory");
        let image_path = temp.path().join("missing.png");
        let uri = file_uri_for_path(&image_path);
        let offered = vec![TEXT_URI_LIST_MIME.to_string()];

        let result = decode_fixture_uri_list(TEXT_URI_LIST_MIME, uri.into_bytes(), offered);

        let error = match result {
            ClipboardPasteResult::DecodeFailed(error) => error,
            other => {
                return Err(format!(
                    "missing-file fixture expected a failure, got {other:?}"
                ));
            }
        };
        assert!(error.contains("could not be read"));
        Ok(())
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
            let mut writer = encoder
                .write_header()
                .expect("PNG fixture writes its header");
            writer
                .write_image_data(&[255, 0, 0, 255])
                .expect("PNG fixture writes its single pixel");
        }
        bytes
    }
}
