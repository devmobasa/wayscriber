//! Side files containing unreadable artifacts, independent of rotation backups.
use super::*;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

pub(crate) const MAX_CORRUPT_COPIES_PER_ARTIFACT: usize = 3;
const CORRUPT_COPY_MARKER: &str = ".corrupt-";

pub(crate) struct CorruptCopy {
    pub(crate) path: PathBuf,
    pub(crate) seq: u64,
    pub(crate) regular_file: bool,
}

pub(crate) fn parse_corrupt_copy_name(name: &OsStr) -> Option<(&OsStr, u64)> {
    let bytes = name.as_bytes();
    let marker = CORRUPT_COPY_MARKER.as_bytes();
    let offset = bytes
        .windows(marker.len())
        .rposition(|part| part == marker)?;
    let artifact = &bytes[..offset];
    let seq = &bytes[offset + marker.len()..];
    if artifact.is_empty()
        || seq.is_empty()
        || seq.starts_with(b"0")
        || !seq.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    Some((
        OsStr::from_bytes(artifact),
        std::str::from_utf8(seq).ok()?.parse().ok()?,
    ))
}

pub(crate) fn corrupt_copy_path(artifact: &Path, seq: u64) -> PathBuf {
    append_path_suffix(artifact, &format!("{CORRUPT_COPY_MARKER}{seq}"))
}

/// Include occupied non-regular names in sequence allocation, never in pruning.
pub(crate) fn corrupt_copies_of(artifact: &Path) -> Result<Vec<CorruptCopy>> {
    let Some(name) = artifact.file_name() else {
        return Ok(Vec::new());
    };
    let parent = artifact
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(err)
                .with_context(|| format!("failed to scan corrupt copies in {}", parent.display()));
        }
    };
    let mut copies = Vec::new();
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some((source, seq)) = parse_corrupt_copy_name(&file_name) else {
            continue;
        };
        if source != name {
            continue;
        }
        let path = entry.path();
        let regular_file = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata.file_type().is_file(),
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        copies.push(CorruptCopy {
            path,
            seq,
            regular_file,
        });
    }

    copies.sort_by_key(|copy| copy.seq);
    Ok(copies)
}

pub(super) fn collect_corrupt_copies(primary: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    for artifact in [primary.to_path_buf(), append_path_suffix(primary, ".bak")] {
        paths.extend(
            corrupt_copies_of(&artifact)?
                .into_iter()
                .map(|copy| copy.path),
        );
    }
    Ok(())
}

pub(crate) fn remove_corrupt_copies(artifact: &Path) -> Result<bool> {
    let mut removed = false;
    for copy in corrupt_copies_of(artifact)?
        .into_iter()
        .filter(|copy| copy.regular_file)
    {
        removed = remove_artifact_if_exists(&copy.path)? || removed;
    }
    Ok(removed)
}
