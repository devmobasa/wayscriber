//! The `IconThemePath` the tray item advertises beside its icon name.
//!
//! The StatusNotifierItem spec makes this path an addition to the host's icon
//! theme search path. Hosts already search the XDG data directories, so a
//! package installed under one of them gains nothing by advertising its icon
//! directory, and some hosts index the advertised directory recursively:
//! `/usr/share/icons` made them walk every installed icon theme (#412).

use crate::env_vars::ICON_THEME_PATH_ENV;
use crate::paths;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The `IconThemePath` to advertise.
///
/// [`ICON_THEME_PATH_ENV`] wins whenever it is set, even to an empty value.
/// Otherwise the install prefix's icon directory is advertised, but only while
/// the tray names an icon for the host to find there, and only when the host
/// does not already search that directory.
pub(super) fn resolve_icon_theme_path(names_tray_icon: bool) -> String {
    if let Ok(value) = env::var(ICON_THEME_PATH_ENV) {
        return value;
    }
    if !names_tray_icon {
        return String::new();
    }

    let Ok(exe_path) = env::current_exe() else {
        return String::new();
    };
    installed_icon_theme_path(&exe_path, &standard_icon_dirs())
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `icons` under each data directory a spec-following host searches.
fn standard_icon_dirs() -> Vec<PathBuf> {
    paths::searched_data_dirs()
        .into_iter()
        .map(|dir| dir.join("icons"))
        .collect()
}

/// `<prefix>/share/icons` for the prefix `exe_path` is installed under, unless
/// that directory is missing or is one of `standard_icon_dirs`.
fn installed_icon_theme_path(exe_path: &Path, standard_icon_dirs: &[PathBuf]) -> Option<PathBuf> {
    let install_root = exe_path.parent()?.parent()?;
    let icon_theme_path = install_root.join("share/icons");
    if !icon_theme_path.is_dir() {
        return None;
    }

    // Compare resolved paths so a symlinked data directory still matches.
    let resolved = fs::canonicalize(&icon_theme_path).ok()?;
    let already_searched = standard_icon_dirs
        .iter()
        .filter_map(|dir| fs::canonicalize(dir).ok())
        .any(|dir| dir == resolved);
    (!already_searched).then_some(icon_theme_path)
}

#[cfg(test)]
mod tests {
    use super::installed_icon_theme_path;
    use std::fs;

    #[test]
    fn installed_icon_dir_is_withheld_when_hosts_already_search_it() {
        let root = crate::test_temp::tempdir().unwrap();
        let share = root.path().join("usr/share");
        fs::create_dir_all(share.join("icons")).unwrap();
        let linked_share = root.path().join("linked-share");
        std::os::unix::fs::symlink(&share, &linked_share).unwrap();
        let exe = root.path().join("usr/bin/wayscriber");

        assert_eq!(
            installed_icon_theme_path(&exe, &[share.join("icons")]),
            None
        );
        assert_eq!(
            installed_icon_theme_path(&exe, &[linked_share.join("icons")]),
            None
        );
    }

    #[test]
    fn installed_icon_dir_is_advertised_for_a_private_prefix() {
        let root = crate::test_temp::tempdir().unwrap();
        let private_icons = root.path().join("opt/wayscriber/share/icons");
        let system_icons = root.path().join("usr/share/icons");
        fs::create_dir_all(&private_icons).unwrap();
        fs::create_dir_all(&system_icons).unwrap();
        let exe = root.path().join("opt/wayscriber/bin/wayscriber");

        assert_eq!(
            installed_icon_theme_path(
                &exe,
                &[system_icons, root.path().join("missing/share/icons")]
            ),
            Some(private_icons)
        );
    }
}
