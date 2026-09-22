use std::path::{Path, PathBuf};

use crate::env_vars::{PATH_ENV, WAYLAND_DISPLAY_ENV, XDG_RUNTIME_DIR_ENV};
use crate::paths::config_dir;

pub const USER_SERVICE_NAME: &str = "wayscriber.service";

/// A source install runs from an immutable cohort, but its service must follow
/// the public selector so the next paired app/broker update can replace it.
pub fn selected_service_executable_path(executable: &Path) -> PathBuf {
    let Some(cohort_dir) = executable.parent() else {
        return executable.to_path_buf();
    };
    let Some(cohort_root) = cohort_dir.parent() else {
        return executable.to_path_buf();
    };
    if executable
        .file_name()
        .is_none_or(|name| name != "wayscriber")
        || cohort_root
            .file_name()
            .is_none_or(|name| name != ".wayscriber-cohorts")
    {
        return executable.to_path_buf();
    }
    let Some(install_dir) = cohort_root.parent() else {
        return executable.to_path_buf();
    };
    let selector = install_dir.join("wayscriber");
    if selector.is_symlink()
        && let (Ok(selected), Ok(root)) = (selector.canonicalize(), cohort_root.canonicalize())
        && selected.file_name() == executable.file_name()
        && selected.parent().and_then(Path::parent) == Some(root.as_path())
    {
        return selector;
    }

    executable.to_path_buf()
}

pub fn user_service_unit_path() -> Option<PathBuf> {
    config_dir().map(|root| user_service_unit_path_from_config_root(&root))
}

pub fn portal_shortcut_dropin_path() -> Option<PathBuf> {
    config_dir().map(|root| portal_shortcut_dropin_path_from_config_root(&root))
}

pub fn user_service_unit_path_from_config_root(config_root: &Path) -> PathBuf {
    config_root
        .join("systemd")
        .join("user")
        .join(USER_SERVICE_NAME)
}

pub fn portal_shortcut_dropin_path_from_config_root(config_root: &Path) -> PathBuf {
    config_root
        .join("systemd")
        .join("user")
        .join(format!("{USER_SERVICE_NAME}.d"))
        .join("shortcut.conf")
}

pub fn quote_systemd_exec(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub fn escape_systemd_env_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The system directories the unit's PATH always ends with, so helper tools
/// resolve the same way regardless of how the service was installed.
const BASE_SERVICE_PATH: [&str; 3] = ["/usr/local/bin", "/usr/bin", "/bin"];

/// The single source for `wayscriber.service`.
///
/// `packaging/wayscriber.service` is this function's output for a
/// `/usr/bin` install, pinned by `packaged_service_unit_matches_the_renderer`.
/// The two used to be maintained by hand and had already drifted apart on the
/// PATH they set.
pub fn render_user_service_unit(binary_path: &Path) -> String {
    let quoted_exec = quote_systemd_exec(binary_path);
    let binary_dir = binary_path
        .parent()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "/usr/bin".to_string());
    // Prepended so an install outside the system directories still finds its
    // own helpers first, and skipped when it is already one of them rather
    // than emitting the directory twice.
    let service_path = if BASE_SERVICE_PATH.contains(&binary_dir.as_str()) {
        BASE_SERVICE_PATH.join(":")
    } else {
        std::iter::once(binary_dir.as_str())
            .chain(BASE_SERVICE_PATH)
            .collect::<Vec<_>>()
            .join(":")
    };
    let escaped_path_env = escape_systemd_env_value(&service_path);
    format!(
        "[Unit]\nDescription=Wayscriber - Screen annotation tool for Wayland\nDocumentation=https://wayscriber.com\nPartOf=graphical-session.target\nAfter=graphical-session.target\n\n[Service]\nType=simple\nExecStartPre=/bin/sh -c '[ -n \"${WAYLAND_DISPLAY_ENV}\" ] && [ -S \"${XDG_RUNTIME_DIR_ENV}/${WAYLAND_DISPLAY_ENV}\" ]'\nExecStart={} --daemon\nRestart=on-failure\nRestartSec=5\nRestartPreventExitStatus=75\nSuccessExitStatus=75\nEnvironment=\"{PATH_ENV}={}\"\n\n[Install]\nWantedBy=graphical-session.target\n",
        quoted_exec, escaped_path_env
    )
}

#[cfg(test)]
mod tests {
    use super::{
        portal_shortcut_dropin_path_from_config_root, quote_systemd_exec, render_user_service_unit,
        selected_service_executable_path, user_service_unit_path_from_config_root,
    };
    use std::path::Path;

    #[test]
    fn installed_cohort_service_uses_public_selector() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let temp = crate::test_temp::tempdir().expect("temporary install root");
        let cohort = temp.path().join(".wayscriber-cohorts/abcd1234");
        fs::create_dir_all(&cohort).expect("cohort directory");
        let executable = cohort.join("wayscriber");
        fs::write(&executable, b"app").expect("cohort executable");
        let selector = temp.path().join("wayscriber");
        symlink(".wayscriber-cohorts/abcd1234/wayscriber", &selector).expect("public selector");

        let service_executable = selected_service_executable_path(&executable);
        assert_eq!(service_executable, selector);
        let unit = render_user_service_unit(&service_executable);
        assert!(unit.contains(&format!("ExecStart=\"{}\" --daemon", selector.display())));

        let next_cohort = temp.path().join(".wayscriber-cohorts/efgh5678");
        fs::create_dir_all(&next_cohort).expect("next cohort directory");
        fs::write(next_cohort.join("wayscriber"), b"next app").expect("next cohort executable");
        fs::remove_file(&selector).expect("remove old selector");
        symlink(".wayscriber-cohorts/efgh5678/wayscriber", &selector).expect("updated selector");
        assert_eq!(selected_service_executable_path(&executable), selector);

        fs::remove_file(&selector).expect("remove selector");
        assert_eq!(selected_service_executable_path(&executable), executable);

        let unrelated = temp.path().join("unrelated-wayscriber");
        fs::write(&unrelated, b"unrelated app").expect("unrelated executable");
        symlink(&unrelated, &selector).expect("unrelated selector");
        assert_eq!(selected_service_executable_path(&executable), executable);
    }

    #[test]
    fn service_paths_are_derived_from_xdg_config_root() {
        let root = Path::new("/tmp/xdg-config");
        assert_eq!(
            user_service_unit_path_from_config_root(root),
            Path::new("/tmp/xdg-config/systemd/user/wayscriber.service")
        );
        assert_eq!(
            portal_shortcut_dropin_path_from_config_root(root),
            Path::new("/tmp/xdg-config/systemd/user/wayscriber.service.d/shortcut.conf")
        );
    }

    #[test]
    fn quote_systemd_exec_supports_whitespace() {
        assert_eq!(
            quote_systemd_exec(Path::new("/tmp/My Apps/wayscriber")),
            "\"/tmp/My Apps/wayscriber\""
        );
    }

    /// `packaging/wayscriber.service` is the renderer's output for the
    /// packaged install path, not a second hand-maintained copy. The two had
    /// already drifted - the packaged unit set a PATH without the binary
    /// directory the renderer prepends - which is the class of bug this pins.
    #[test]
    fn packaged_service_unit_matches_the_renderer() {
        let packaged = include_str!("../packaging/wayscriber.service");
        let rendered = render_user_service_unit(Path::new("/usr/bin/wayscriber"));
        assert_eq!(
            packaged, rendered,
            "packaging/wayscriber.service is generated; write this instead:\n{rendered}"
        );
    }

    /// An install outside the system directories puts its own directory first
    /// so helper lookups find it, and a system install does not repeat one.
    #[test]
    fn service_path_lists_the_binary_directory_once() {
        let system = render_user_service_unit(Path::new("/usr/bin/wayscriber"));
        assert!(
            system.contains("Environment=\"PATH=/usr/local/bin:/usr/bin:/bin\""),
            "unexpected system PATH: {system}"
        );

        let local = render_user_service_unit(Path::new("/home/u/.local/bin/wayscriber"));
        assert!(
            local.contains("Environment=\"PATH=/home/u/.local/bin:/usr/local/bin:/usr/bin:/bin\""),
            "unexpected local PATH: {local}"
        );
    }

    #[test]
    fn render_user_service_unit_quotes_exec_path() {
        let unit = render_user_service_unit(Path::new("/tmp/My Apps/wayscriber"));
        assert!(unit.contains("ExecStart=\"/tmp/My Apps/wayscriber\" --daemon"));
    }
}
