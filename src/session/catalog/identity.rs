use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPathIdentity {
    pub exact_path: PathBuf,
    pub canonical_path: Option<PathBuf>,
}

/// Compute the exact/canonical identity used for dedupe without creating files.
pub fn session_path_identity(path: &Path) -> CatalogPathIdentity {
    let exact_path = normalize_exact_path(path);
    let canonical_path = existing_or_parent_canonical_path(&exact_path);
    CatalogPathIdentity {
        exact_path,
        canonical_path,
    }
}

/// Return true when two paths identify the same session target by exact or
/// canonical identity without requiring the primary file to exist.
pub fn session_paths_match(a: &Path, b: &Path) -> bool {
    let a = session_path_identity(a);
    let b = session_path_identity(b);
    catalog_identities_match(&a, &b)
}

fn catalog_identities_match(a: &CatalogPathIdentity, b: &CatalogPathIdentity) -> bool {
    if a.exact_path == b.exact_path {
        return true;
    }
    match (a.canonical_path.as_deref(), b.canonical_path.as_deref()) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

pub(super) fn entry_matches_identity(entry: &CatalogEntry, identity: &CatalogPathIdentity) -> bool {
    if path_string_matches(&entry.path, &identity.exact_path) {
        return true;
    }
    match (
        entry.canonical_path.as_deref(),
        identity.canonical_path.as_deref(),
    ) {
        (Some(existing), Some(candidate)) => path_string_matches(existing, candidate),
        _ => false,
    }
}

fn path_string_matches(existing: &str, candidate: &Path) -> bool {
    Path::new(existing) == candidate
}

fn existing_or_parent_canonical_path(path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = path.canonicalize() {
        return Some(canonical);
    }
    let parent = path.parent()?;
    let file_name = path.file_name()?;
    parent
        .canonicalize()
        .ok()
        .map(|parent| parent.join(file_name))
}

pub(super) fn normalize_exact_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => normalized.push(".."),
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

pub(super) fn path_to_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_string).ok_or_else(|| {
        anyhow!(
            "session catalog path must be valid UTF-8: {}",
            path.display()
        )
    })
}

pub(super) fn optional_path_to_string(path: Option<&Path>) -> Result<Option<String>> {
    path.map(path_to_string).transpose()
}

pub(super) fn display_name_for_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("session")
        .to_string()
}
