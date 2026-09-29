use std::path::{Path, PathBuf};

use crate::env_vars::{PATH_ENV, WAYLAND_DISPLAY_ENV, XDG_RUNTIME_DIR_ENV};
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

/// Quotes `path` as the command word of an `ExecStart=` line.
///
/// systemd expands `%` specifiers in the command word, but `$VAR` substitution
/// applies only to the arguments after it, so a `$` in the path stays as it is:
/// doubling it would name an executable that does not exist.
pub fn quote_systemd_exec(path: &Path) -> String {
    let mut quoted = String::from("\"");
    for character in path.to_string_lossy().chars() {
        push_quoted_char(&mut quoted, character);
    }
    quoted.push('"');
    quoted
}

/// Escapes `value` for a double-quoted `Environment=` assignment, where `$`
/// has no special meaning and stays as it is.
pub fn escape_systemd_env_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        push_quoted_char(&mut escaped, character);
    }
    escaped
}

/// Appends `character` as it must appear inside a double-quoted systemd word.
///
/// Quoting alone is not enough: `%` still starts a specifier, and a line break
/// would end the setting and start a new directive. `%` is doubled, and
/// control characters become C-style `\xNN` escapes, which systemd decodes
/// back to the original byte.
fn push_quoted_char(out: &mut String, character: char) {
    match character {
        '\\' => out.push_str("\\\\"),
        '"' => out.push_str("\\\""),
        '%' => out.push_str("%%"),
        character if character.is_ascii_control() => {
            out.push_str(&format!("\\x{:02x}", u32::from(character)));
        }
        character => out.push(character),
    }
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
        escape_systemd_env_value, portal_shortcut_dropin_path_from_config_root, quote_systemd_exec,
        render_user_service_unit, user_service_unit_path_from_config_root,
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

    #[test]
    fn exec_path_escapes_specifiers_but_keeps_dollar_literal() {
        // `systemd-analyze verify` accepts `$` as written in the command word
        // and reports `$$` as a missing executable.
        assert_eq!(
            quote_systemd_exec(Path::new("/opt/100%/$HOME/wayscriber")),
            "\"/opt/100%%/$HOME/wayscriber\""
        );
    }

    #[test]
    fn environment_value_escapes_specifiers_but_keeps_dollar_literal() {
        assert_eq!(
            escape_systemd_env_value("a\\b\"c%d$e\tf"),
            "a\\\\b\\\"c%%d$e\\x09f"
        );
    }

    #[test]
    fn a_line_break_in_the_install_path_cannot_add_a_directive() {
        let normal = render_user_service_unit(Path::new("/opt/wayscriber/wayscriber"));
        let unit =
            render_user_service_unit(Path::new("/opt/x\nExecStartPre=/bin/false\n/wayscriber"));

        assert_eq!(unit.lines().count(), normal.lines().count());
        assert!(!unit.lines().any(|line| line == "ExecStartPre=/bin/false"));
        assert!(unit.contains("ExecStart=\"/opt/x\\x0aExecStartPre=/bin/false\\x0a/wayscriber\""));
    }
}
