use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use super::command::{command_available, run_command};
use super::service::resolve_wayscriber_binary_path;

const HYPRLAND_DIR: &str = "hypr";
const MAIN_CONFIG: &str = "hyprland.conf";
const LIGHT_CONTROLS_INCLUDE: &str = "wayscriber-light.conf";
const LIGHT_CONTROLS_COMMENT: &str = "# Wayscriber light passthrough controls";

#[derive(Debug, Clone)]
pub(super) struct HyprlandLightControlsStatus {
    pub(super) include_path: Option<PathBuf>,
    pub(super) include_exists: bool,
    pub(super) source_present: bool,
}

impl HyprlandLightControlsStatus {
    pub(super) fn configured(&self) -> bool {
        self.include_exists && self.source_present
    }
}

#[derive(Debug, Clone)]
pub(super) struct HyprlandLightControlsInstallResult {
    pub(super) include_path: PathBuf,
    pub(super) main_config_path: PathBuf,
    pub(super) source_line: String,
    pub(super) source_configured: bool,
    pub(super) source_updated: bool,
    pub(super) reload_attempted: bool,
    pub(super) reload_succeeded: bool,
    pub(super) reload_error: Option<String>,
}

impl HyprlandLightControlsInstallResult {
    pub(super) fn summary(&self) -> String {
        let mut parts = vec![format!(
            "Wrote Hyprland light controls to {}",
            self.include_path.display()
        )];
        parts.push("clears the default light keys before rebinding them".to_string());

        if self.source_configured {
            if self.source_updated {
                parts.push(format!(
                    "added source line to {}",
                    self.main_config_path.display()
                ));
            } else {
                parts.push(format!(
                    "source line already present in {}",
                    self.main_config_path.display()
                ));
            }
        } else {
            parts.push(format!(
                "add `{}` to your Hyprland config to enable it",
                self.source_line
            ));
        }

        if self.reload_attempted {
            if self.reload_succeeded {
                parts.push("reloaded Hyprland".to_string());
            } else if let Some(error) = self.reload_error.as_deref() {
                parts.push(format!("hyprctl reload failed: {error}"));
            }
        } else if self.source_configured {
            parts.push("run `hyprctl reload` to apply it".to_string());
        }

        parts.join("; ")
    }
}

pub(super) fn read_light_controls_status() -> HyprlandLightControlsStatus {
    let Some(config_root) = wayscriber::paths::config_dir() else {
        return HyprlandLightControlsStatus {
            include_path: None,
            include_exists: false,
            source_present: false,
        };
    };
    read_light_controls_status_from_config_root(&config_root)
}

fn read_light_controls_status_from_config_root(config_root: &Path) -> HyprlandLightControlsStatus {
    let include_path = light_controls_include_path(config_root);
    let main_config_path = main_config_path(config_root);
    let source_line = source_line_for_include(&include_path);
    let source_present = fs::read_to_string(&main_config_path)
        .map(|content| has_source_line(&content, &source_line))
        .unwrap_or(false);

    HyprlandLightControlsStatus {
        include_exists: include_path.exists(),
        include_path: Some(include_path),
        source_present,
    }
}

pub(super) fn install_light_controls() -> Result<HyprlandLightControlsInstallResult, String> {
    let config_root = wayscriber::paths::config_dir().ok_or_else(|| {
        "Cannot resolve config directory; failed to determine Hyprland config path.".to_string()
    })?;
    let binary_path = resolve_wayscriber_binary_path()?;
    let mut result = write_light_controls(&config_root, &binary_path)?;

    if result.source_configured && command_available("hyprctl") {
        result.reload_attempted = true;
        match run_command("hyprctl", &["reload"]) {
            Ok(capture) if capture.success => {
                result.reload_succeeded = true;
            }
            Ok(capture) => {
                result.reload_error = Some(format!(
                    "stdout: {}; stderr: {}",
                    capture.stdout.trim(),
                    capture.stderr.trim()
                ));
            }
            Err(err) => {
                result.reload_error = Some(err);
            }
        }
    }

    Ok(result)
}

fn write_light_controls(
    config_root: &Path,
    binary_path: &Path,
) -> Result<HyprlandLightControlsInstallResult, String> {
    let hyprland_dir = config_root.join(HYPRLAND_DIR);
    fs::create_dir_all(&hyprland_dir).map_err(|err| {
        format!(
            "Failed to create Hyprland config directory {}: {}",
            hyprland_dir.display(),
            err
        )
    })?;

    let include_path = light_controls_include_path(config_root);
    fs::write(&include_path, render_light_controls(binary_path)).map_err(|err| {
        format!(
            "Failed to write Hyprland light controls {}: {}",
            include_path.display(),
            err
        )
    })?;

    let main_config_path = main_config_path(config_root);
    let source_line = source_line_for_include(&include_path);
    let mut source_configured = false;
    let mut source_updated = false;

    match fs::read_to_string(&main_config_path) {
        Ok(content) => {
            let (updated_content, changed) = ensure_source_line(&content, &source_line);
            source_configured = true;
            source_updated = changed;
            if changed {
                fs::write(&main_config_path, updated_content).map_err(|err| {
                    format!(
                        "Failed to update Hyprland config {}: {}",
                        main_config_path.display(),
                        err
                    )
                })?;
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "Failed to read Hyprland config {}: {}",
                main_config_path.display(),
                err
            ));
        }
    }

    Ok(HyprlandLightControlsInstallResult {
        include_path,
        main_config_path,
        source_line,
        source_configured,
        source_updated,
        reload_attempted: false,
        reload_succeeded: false,
        reload_error: None,
    })
}

fn light_controls_include_path(config_root: &Path) -> PathBuf {
    config_root.join(HYPRLAND_DIR).join(LIGHT_CONTROLS_INCLUDE)
}

fn main_config_path(config_root: &Path) -> PathBuf {
    config_root.join(HYPRLAND_DIR).join(MAIN_CONFIG)
}

fn source_line_for_include(include_path: &Path) -> String {
    format!("source = {}", include_path.display())
}

fn render_light_controls(binary_path: &Path) -> String {
    let binary = shell_quote(binary_path.to_string_lossy().as_ref());
    format!(
        "# Generated by wayscriber. Edit shortcuts if needed.\n\
{LIGHT_CONTROLS_COMMENT}\n\
unbind = SUPER ALT, L\n\
bind = SUPER ALT, L, exec, {binary} --light-toggle\n\
unbind = SUPER ALT, D\n\
bind = SUPER ALT, D, exec, {binary} --light-draw-toggle\n\
unbind = SUPER ALT, F\n\
bind = SUPER ALT, F, exec, {binary} --light-draw-on\n\
bindr = SUPER ALT, F, exec, {binary} --light-draw-off\n"
    )
}

fn ensure_source_line(content: &str, source_line: &str) -> (String, bool) {
    if has_source_line(content, source_line) {
        return (content.to_string(), false);
    }

    let mut updated = content.to_string();
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    if !updated.trim_end().is_empty() {
        updated.push('\n');
    }
    updated.push_str(LIGHT_CONTROLS_COMMENT);
    updated.push('\n');
    updated.push_str(source_line);
    updated.push('\n');
    (updated, true)
}

fn has_source_line(content: &str, source_line: &str) -> bool {
    let Some(expected_target) =
        source_target(source_line).map(|target| normalize_source_target(&target))
    else {
        return false;
    };
    content
        .lines()
        .filter_map(source_target)
        .any(|target| normalize_source_target(&target) == expected_target)
}

fn source_target(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
        return None;
    }
    let rest = trimmed.strip_prefix("source")?.trim_start();
    let rest = rest.strip_prefix('=')?.trim();
    let rest = strip_inline_comment(rest);
    let rest = strip_matching_quotes(rest.trim());
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

fn strip_inline_comment(value: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(quote_character) = quote {
            if character == quote_character {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        if character == '#' {
            return value[..index].trim_end();
        }
    }
    value
}

fn strip_matching_quotes(value: &str) -> &str {
    if value.len() < 2 {
        return value;
    }
    let first = value.as_bytes()[0];
    let last = value.as_bytes()[value.len() - 1];
    if matches!(first, b'\'' | b'"') && first == last {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn normalize_source_target(target: &str) -> PathBuf {
    let expanded = if let Some(stripped) = target.strip_prefix("~/") {
        wayscriber::paths::home_dir()
            .map(|home| home.join(stripped))
            .unwrap_or_else(|| PathBuf::from(target))
    } else {
        PathBuf::from(target)
    };
    lexical_normalize(&expanded)
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value.bytes().all(
        |byte| matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'/' | b'.' | b'_' | b'-'),
    ) {
        return value.to_string();
    }

    let mut quoted = String::from("'");
    for character in value.chars() {
        if character == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(character);
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Mutex;
    use wayscriber::env_vars::HOME_ENV;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn render_light_controls_quotes_binary_with_spaces() {
        let rendered = render_light_controls(Path::new("/tmp/My Apps/wayscriber"));
        assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-toggle"));
        assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-draw-toggle"));
        assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-draw-on"));
        assert!(rendered.contains("'/tmp/My Apps/wayscriber' --light-draw-off"));
    }

    #[test]
    fn render_light_controls_escapes_single_quotes() {
        let rendered = render_light_controls(Path::new("/tmp/O'Brien/wayscriber"));
        assert!(rendered.contains("'/tmp/O'\\''Brien/wayscriber' --light-toggle"));
    }

    #[test]
    fn render_light_controls_unbinds_default_keys_before_binding() {
        let rendered = render_light_controls(Path::new("/tmp/wayscriber"));

        let unbind_l = rendered.find("unbind = SUPER ALT, L").unwrap();
        let bind_l = rendered.find("\nbind = SUPER ALT, L").unwrap();
        assert!(unbind_l < bind_l);

        let unbind_d = rendered.find("unbind = SUPER ALT, D").unwrap();
        let bind_d = rendered.find("\nbind = SUPER ALT, D").unwrap();
        assert!(unbind_d < bind_d);

        let unbind_f = rendered.find("unbind = SUPER ALT, F").unwrap();
        let bind_f = rendered.find("\nbind = SUPER ALT, F").unwrap();
        let bindr_f = rendered.find("bindr = SUPER ALT, F").unwrap();
        assert!(unbind_f < bind_f);
        assert!(unbind_f < bindr_f);
    }

    #[test]
    fn ensure_source_line_appends_once() {
        let source_line = "source = /tmp/hypr/wayscriber-light.conf";
        let (updated, changed) =
            ensure_source_line("source = ~/.config/hypr/base.conf\n", source_line);
        assert!(changed);
        assert!(updated.contains(LIGHT_CONTROLS_COMMENT));
        assert!(updated.contains(source_line));

        let (again, changed_again) = ensure_source_line(&updated, source_line);
        assert!(!changed_again);
        assert_eq!(again.matches(source_line).count(), 1);
    }

    #[test]
    fn has_source_line_ignores_comments_and_spacing() {
        let source_line = "source = /tmp/hypr/wayscriber-light.conf";
        assert!(!has_source_line(
            "# source = /tmp/hypr/wayscriber-light.conf\n",
            source_line
        ));
        assert!(has_source_line(
            "  source   =   /tmp/hypr/wayscriber-light.conf  \n",
            source_line
        ));
    }

    #[test]
    fn has_source_line_matches_quoted_and_inline_commented_targets() {
        let source_line = "source = /tmp/hypr/wayscriber-light.conf";
        assert!(has_source_line(
            "source = '/tmp/hypr/wayscriber-light.conf' # installed by wayscriber\n",
            source_line
        ));
        assert!(has_source_line(
            "source = \"/tmp/hypr/wayscriber-light.conf\" # installed by wayscriber\n",
            source_line
        ));
    }

    #[test]
    fn has_source_line_matches_tilde_target() {
        let _guard = ENV_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = crate::test_temp::tempdir().unwrap();
        let home = tmp.path();
        let prev_home = env::var_os(HOME_ENV);
        unsafe {
            env::set_var(HOME_ENV, home);
        }

        let absolute = home
            .join(".config")
            .join("hypr")
            .join(LIGHT_CONTROLS_INCLUDE);
        let source_line = format!("source = {}", absolute.display());
        assert!(has_source_line(
            "source = ~/.config/hypr/wayscriber-light.conf # already sourced\n",
            &source_line
        ));

        match prev_home {
            Some(value) => unsafe { env::set_var(HOME_ENV, value) },
            None => unsafe { env::remove_var(HOME_ENV) },
        }
    }

    #[test]
    fn write_light_controls_writes_include_and_sources_existing_main() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let hypr_dir = tmp.path().join(HYPRLAND_DIR);
        fs::create_dir_all(&hypr_dir).unwrap();
        let main = hypr_dir.join(MAIN_CONFIG);
        fs::write(&main, "source = ~/.config/hypr/base.conf\n").unwrap();

        let result =
            write_light_controls(tmp.path(), Path::new("/tmp/My Apps/wayscriber")).unwrap();

        assert!(result.source_configured);
        assert!(result.source_updated);
        assert!(result.include_path.exists());
        let include = fs::read_to_string(&result.include_path).unwrap();
        assert!(include.contains("'/tmp/My Apps/wayscriber' --light-toggle"));
        assert!(include.contains("unbind = SUPER ALT, L"));
        let main_content = fs::read_to_string(&main).unwrap();
        assert!(main_content.contains(&result.source_line));
    }

    #[test]
    fn write_light_controls_is_idempotent_for_existing_source() {
        let tmp = crate::test_temp::tempdir().unwrap();
        let hypr_dir = tmp.path().join(HYPRLAND_DIR);
        fs::create_dir_all(&hypr_dir).unwrap();
        let main = hypr_dir.join(MAIN_CONFIG);
        let include = light_controls_include_path(tmp.path());
        let source_line = source_line_for_include(&include);
        fs::write(&main, format!("{source_line}\n")).unwrap();

        let result = write_light_controls(tmp.path(), Path::new("/tmp/wayscriber")).unwrap();

        assert!(result.source_configured);
        assert!(!result.source_updated);
        let main_content = fs::read_to_string(&main).unwrap();
        assert_eq!(main_content.matches(&source_line).count(), 1);
    }

    #[test]
    fn write_light_controls_handles_missing_main_config() {
        let tmp = crate::test_temp::tempdir().unwrap();

        let result = write_light_controls(tmp.path(), Path::new("/tmp/wayscriber")).unwrap();

        assert!(result.include_path.exists());
        assert!(!result.source_configured);
        assert!(!result.source_updated);
    }
}
