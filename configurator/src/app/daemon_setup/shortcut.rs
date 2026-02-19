use std::fs;

use crate::models::{DesktopEnvironment, ShortcutBackend};

use super::command::{command_available, run_command, run_command_checked};
use super::service::{
    escape_systemd_env_value, portal_shortcut_dropin_path, query_service_active,
    require_systemctl_available, run_systemctl_user,
};

const PORTAL_APP_ID: &str = "wayscriber";
const TOGGLE_COMMAND: &str = "pkill -SIGUSR1 wayscriber";

const GNOME_MEDIA_KEYS_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const GNOME_MEDIA_KEYS_KEY: &str = "custom-keybindings";
const GNOME_CUSTOM_KEYBINDING_SCHEMA: &str =
    "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const GNOME_WAYSCRIBER_KEYBINDING_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/";
const GNOME_SHORTCUT_NAME: &str = "Wayscriber Toggle";

pub(super) fn read_configured_shortcut(backend: ShortcutBackend) -> Option<String> {
    match backend {
        ShortcutBackend::GnomeCustomShortcut => read_gnome_shortcut_binding(),
        ShortcutBackend::PortalServiceDropIn => read_portal_shortcut_from_dropin(),
        ShortcutBackend::Manual => None,
    }
}

pub(super) fn apply_shortcut(shortcut_input: &str) -> Result<String, String> {
    let desktop = DesktopEnvironment::detect_current();
    let backend = ShortcutBackend::from_environment(
        desktop,
        command_available("gsettings"),
        command_available("systemctl"),
    );

    match backend {
        ShortcutBackend::GnomeCustomShortcut => {
            let normalized = normalize_shortcut_for_gnome(shortcut_input)?;
            apply_gnome_custom_shortcut(&normalized)?;
            Ok(format!("Configured GNOME shortcut: {normalized}"))
        }
        ShortcutBackend::PortalServiceDropIn => {
            require_systemctl_available()?;
            let normalized = normalize_shortcut_for_portal(shortcut_input)?;
            let dropin_path = write_portal_shortcut_dropin(&normalized)?;
            run_systemctl_user(&["daemon-reload"])?;
            if query_service_active() {
                run_systemctl_user(&["restart", "wayscriber.service"])?;
            }
            Ok(format!(
                "Configured portal shortcut: {normalized} (drop-in: {})",
                dropin_path.display()
            ))
        }
        ShortcutBackend::Manual => Err(
            "Automatic shortcut setup is not available in this desktop session; bind `pkill -SIGUSR1 wayscriber` manually."
                .to_string(),
        ),
    }
}

fn apply_gnome_custom_shortcut(binding: &str) -> Result<(), String> {
    require_gsettings_available()?;

    let mut bindings = read_gnome_custom_keybinding_paths()?;
    if !bindings
        .iter()
        .any(|path| path == GNOME_WAYSCRIBER_KEYBINDING_PATH)
    {
        bindings.push(GNOME_WAYSCRIBER_KEYBINDING_PATH.to_string());
    }
    let rendered_bindings = serialize_gsettings_path_list(&bindings);

    run_gsettings_command(&[
        "set",
        GNOME_MEDIA_KEYS_SCHEMA,
        GNOME_MEDIA_KEYS_KEY,
        &rendered_bindings,
    ])?;

    let schema_with_path =
        format!("{GNOME_CUSTOM_KEYBINDING_SCHEMA}:{GNOME_WAYSCRIBER_KEYBINDING_PATH}");
    run_gsettings_command(&[
        "set",
        &schema_with_path,
        "name",
        &gvariant_string_literal(GNOME_SHORTCUT_NAME),
    ])?;
    run_gsettings_command(&[
        "set",
        &schema_with_path,
        "command",
        &gvariant_string_literal(TOGGLE_COMMAND),
    ])?;
    run_gsettings_command(&[
        "set",
        &schema_with_path,
        "binding",
        &gvariant_string_literal(binding),
    ])?;

    Ok(())
}

fn read_gnome_custom_keybinding_paths() -> Result<Vec<String>, String> {
    let capture = run_command_checked(
        "gsettings",
        &["get", GNOME_MEDIA_KEYS_SCHEMA, GNOME_MEDIA_KEYS_KEY],
    )?;
    parse_gsettings_path_list(&capture.stdout)
}

fn read_gnome_shortcut_binding() -> Option<String> {
    if !command_available("gsettings") {
        return None;
    }
    let schema_with_path =
        format!("{GNOME_CUSTOM_KEYBINDING_SCHEMA}:{GNOME_WAYSCRIBER_KEYBINDING_PATH}");
    let capture = run_command("gsettings", &["get", &schema_with_path, "binding"]).ok()?;
    if !capture.success {
        return None;
    }
    parse_gsettings_string_value(capture.stdout.trim())
}

fn require_gsettings_available() -> Result<(), String> {
    if command_available("gsettings") {
        Ok(())
    } else {
        Err("gsettings is not available in PATH.".to_string())
    }
}

fn run_gsettings_command(args: &[&str]) -> Result<(), String> {
    let _ = run_command_checked("gsettings", args)?;
    Ok(())
}

fn write_portal_shortcut_dropin(shortcut: &str) -> Result<std::path::PathBuf, String> {
    let dropin_path = portal_shortcut_dropin_path().ok_or_else(|| {
        "Cannot resolve home directory; failed to determine systemd drop-in path.".to_string()
    })?;
    let dropin_dir = dropin_path
        .parent()
        .ok_or_else(|| "Invalid drop-in path".to_string())?;
    fs::create_dir_all(dropin_dir).map_err(|err| {
        format!(
            "Failed to create service drop-in directory {}: {}",
            dropin_dir.display(),
            err
        )
    })?;

    let escaped_shortcut = escape_systemd_env_value(shortcut);
    let escaped_app_id = escape_systemd_env_value(PORTAL_APP_ID);
    let contents = format!(
        "[Service]\nEnvironment=\"WAYSCRIBER_PORTAL_SHORTCUT={escaped_shortcut}\"\nEnvironment=\"WAYSCRIBER_PORTAL_APP_ID={escaped_app_id}\"\n"
    );
    fs::write(&dropin_path, contents).map_err(|err| {
        format!(
            "Failed to write portal shortcut drop-in {}: {}",
            dropin_path.display(),
            err
        )
    })?;
    Ok(dropin_path)
}

fn read_portal_shortcut_from_dropin() -> Option<String> {
    let path = portal_shortcut_dropin_path()?;
    let content = fs::read_to_string(path).ok()?;
    parse_portal_shortcut_from_dropin(&content)
}

fn parse_portal_shortcut_from_dropin(content: &str) -> Option<String> {
    content.lines().find_map(|line| {
        let trimmed = line.trim();
        let prefix = "Environment=\"WAYSCRIBER_PORTAL_SHORTCUT=";
        if !trimmed.starts_with(prefix) || !trimmed.ends_with('"') {
            return None;
        }
        let inner = &trimmed[prefix.len()..trimmed.len() - 1];
        if inner.is_empty() {
            return None;
        }
        Some(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
    })
}

fn parse_gsettings_path_list(raw: &str) -> Result<Vec<String>, String> {
    let trimmed = raw.trim();
    let list_literal = trimmed.strip_prefix("@as ").map_or(trimmed, str::trim);
    if !list_literal.starts_with('[') || !list_literal.ends_with(']') {
        return Err(format!(
            "Unexpected gsettings list format: `{}`",
            raw.trim()
        ));
    }
    let inner = list_literal[1..list_literal.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    let mut values = Vec::new();
    for chunk in inner.split(',') {
        let value = chunk.trim().trim_matches('\'').trim_matches('"').trim();
        if value.is_empty() {
            continue;
        }
        values.push(value.to_string());
    }
    Ok(values)
}

fn serialize_gsettings_path_list(paths: &[String]) -> String {
    let mut rendered = String::from("[");
    for (index, path) in paths.iter().enumerate() {
        if index > 0 {
            rendered.push_str(", ");
        }
        rendered.push('\'');
        rendered.push_str(path);
        rendered.push('\'');
    }
    rendered.push(']');
    rendered
}

fn gvariant_string_literal(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

fn parse_gsettings_string_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "''" {
        return None;
    }
    let unquoted = trimmed
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
        .unwrap_or(trimmed);
    if unquoted.is_empty() {
        return None;
    }
    Some(unquoted.replace("\\'", "'").replace("\\\\", "\\"))
}

fn normalize_shortcut_for_gnome(input: &str) -> Result<String, String> {
    normalize_shortcut(input, true)
}

fn normalize_shortcut_for_portal(input: &str) -> Result<String, String> {
    normalize_shortcut(input, false)
}

fn normalize_shortcut(input: &str, gnome_style: bool) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(if gnome_style {
            "<Super>g".to_string()
        } else {
            "<Ctrl><Shift>g".to_string()
        });
    }
    if trimmed.contains('<') && trimmed.contains('>') {
        return Ok(trimmed.to_string());
    }

    let parts: Vec<&str> = trimmed
        .split('+')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect();
    if parts.is_empty() {
        return Err("Shortcut cannot be empty.".to_string());
    }
    let key = normalize_key_name(parts[parts.len() - 1])?;
    let modifiers = &parts[..parts.len() - 1];

    let mut rendered_modifiers: Vec<&'static str> = Vec::new();
    for modifier in modifiers {
        let normalized = normalize_modifier(modifier, gnome_style).ok_or_else(|| {
            format!(
                "Unsupported modifier `{}`. Supported: Ctrl, Shift, Alt, Super/Meta.",
                modifier
            )
        })?;
        if !rendered_modifiers.contains(&normalized) {
            rendered_modifiers.push(normalized);
        }
    }

    let mut normalized = String::new();
    for modifier in rendered_modifiers {
        normalized.push_str(modifier);
    }
    normalized.push_str(&key);
    Ok(normalized)
}

fn normalize_modifier(modifier: &str, gnome_style: bool) -> Option<&'static str> {
    match modifier.to_ascii_lowercase().as_str() {
        "ctrl" | "control" | "primary" => Some(if gnome_style { "<Primary>" } else { "<Ctrl>" }),
        "shift" => Some("<Shift>"),
        "alt" | "option" => Some("<Alt>"),
        "super" | "meta" | "win" | "windows" => Some("<Super>"),
        _ => None,
    }
}

fn normalize_key_name(key: &str) -> Result<String, String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("Shortcut key is empty.".to_string());
    }
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with('F')
        && upper.len() > 1
        && upper[1..]
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Ok(upper);
    }
    if trimmed.chars().count() == 1 {
        return Ok(trimmed.to_ascii_lowercase());
    }
    Ok(trimmed.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gsettings_paths_handles_empty_variants() {
        assert_eq!(
            parse_gsettings_path_list("@as []").unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            parse_gsettings_path_list("[]").unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn parse_and_serialize_gsettings_paths_round_trip() {
        let raw = "['/org/one/', '/org/two/']";
        let parsed = parse_gsettings_path_list(raw).expect("parse gsettings path list");
        assert_eq!(
            parsed,
            vec!["/org/one/".to_string(), "/org/two/".to_string()]
        );
        assert_eq!(serialize_gsettings_path_list(&parsed), raw);
    }

    #[test]
    fn parse_portal_shortcut_reads_dropin_value() {
        let content = "[Service]\nEnvironment=\"WAYSCRIBER_PORTAL_SHORTCUT=<Ctrl><Shift>g\"\n";
        assert_eq!(
            parse_portal_shortcut_from_dropin(content),
            Some("<Ctrl><Shift>g".to_string())
        );
    }

    #[test]
    fn normalize_shortcut_supports_human_readable_input() {
        assert_eq!(
            normalize_shortcut_for_gnome("Super+G").unwrap(),
            "<Super>g".to_string()
        );
        assert_eq!(
            normalize_shortcut_for_portal("Ctrl+Shift+G").unwrap(),
            "<Ctrl><Shift>g".to_string()
        );
        assert_eq!(
            normalize_shortcut_for_portal("<Ctrl><Shift>g").unwrap(),
            "<Ctrl><Shift>g".to_string()
        );
    }

    #[test]
    fn normalize_shortcut_rejects_unknown_modifier() {
        let error =
            normalize_shortcut_for_portal("Hyper+G").expect_err("expected invalid shortcut");
        assert!(error.contains("Unsupported modifier"));
    }
}
