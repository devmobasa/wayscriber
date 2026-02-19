use std::path::{Path, PathBuf};

use crate::paths::config_dir;

pub const USER_SERVICE_NAME: &str = "wayscriber.service";

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

pub fn render_user_service_unit(binary_path: &Path) -> String {
    let quoted_exec = quote_systemd_exec(binary_path);
    let binary_dir = binary_path
        .parent()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "/usr/bin".to_string());
    let escaped_path_env =
        escape_systemd_env_value(&format!("{binary_dir}:/usr/local/bin:/usr/bin:/bin"));
    format!(
        "[Unit]\nDescription=Wayscriber - Screen annotation tool for Wayland\nDocumentation=https://wayscriber.com\nPartOf=graphical-session.target\nAfter=graphical-session.target\n\n[Service]\nType=simple\nExecStartPre=/bin/sh -c '[ -n \"$WAYLAND_DISPLAY\" ] && [ -S \"$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY\" ]'\nExecStart={} --daemon\nRestart=on-failure\nRestartSec=5\nRestartPreventExitStatus=75\nSuccessExitStatus=75\nEnvironment=\"PATH={}\"\n\n[Install]\nWantedBy=graphical-session.target\n",
        quoted_exec, escaped_path_env
    )
}

#[cfg(test)]
mod tests {
    use super::{
        portal_shortcut_dropin_path_from_config_root, quote_systemd_exec, render_user_service_unit,
        user_service_unit_path_from_config_root,
    };
    use std::path::Path;

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

    #[test]
    fn render_user_service_unit_quotes_exec_path() {
        let unit = render_user_service_unit(Path::new("/tmp/My Apps/wayscriber"));
        assert!(unit.contains("ExecStart=\"/tmp/My Apps/wayscriber\" --daemon"));
    }
}
