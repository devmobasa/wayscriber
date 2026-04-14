use std::fs;

use crate::models::{DesktopEnvironment, ShortcutApplyCapability, ShortcutBackend};
use wayscriber::shortcut_hint::{
    GNOME_MEDIA_KEYS_KEY, GNOME_MEDIA_KEYS_SCHEMA, GNOME_WAYSCRIBER_KEYBINDING_PATH,
    PORTAL_APP_ID_ENV, PORTAL_SHORTCUT_ENV, PORTAL_SHORTCUT_OPT_IN_ENV, PortalShortcutDropInState,
    gnome_effective_shortcut, gnome_shortcut_schema_with_path, parse_gsettings_path_list,
    parse_portal_shortcut_dropin_state,
    parse_portal_shortcut_from_dropin as shared_parse_portal_shortcut_from_dropin,
};

use super::command::{command_available, run_command, run_command_checked};
use super::service::{
    escape_systemd_env_value, portal_shortcut_dropin_path, query_service_active,
    remove_portal_shortcut_dropin_if_gnome, require_systemctl_available,
    resolve_wayscriber_binary_path, run_systemctl_user,
};

const PORTAL_APP_ID: &str = "wayscriber";
const GNOME_SHORTCUT_NAME: &str = "Wayscriber Toggle";

pub(super) fn read_configured_shortcut(backend: ShortcutBackend) -> Option<String> {
    match backend {
        ShortcutBackend::GnomeCustomShortcut => read_gnome_shortcut_binding(),
        ShortcutBackend::PortalServiceDropIn => read_portal_shortcut_from_dropin(),
        ShortcutBackend::Manual => None,
    }
}

pub(super) fn read_portal_shortcut_dropin_state() -> PortalShortcutDropInState {
    let Some(path) = portal_shortcut_dropin_path() else {
        return PortalShortcutDropInState::default();
    };
    let Ok(content) = fs::read_to_string(path) else {
        return PortalShortcutDropInState::default();
    };
    parse_portal_shortcut_dropin_state(&content)
}

pub(super) fn apply_shortcut(shortcut_input: &str) -> Result<String, String> {
    let desktop = DesktopEnvironment::detect_current();
    let apply_capability = ShortcutApplyCapability::from_environment(
        desktop,
        command_available("gsettings"),
        command_available("systemctl"),
    );

    match apply_capability {
        ShortcutApplyCapability::GnomeCustomShortcut => {
            let normalized = normalize_shortcut_for_gnome(shortcut_input)?;
            apply_gnome_custom_shortcut(&normalized)?;
            let removed_dropin = remove_portal_shortcut_dropin_if_gnome(desktop)?;
            if command_available("systemctl") {
                run_systemctl_user(&["daemon-reload"])?;
                if removed_dropin && query_service_active() {
                    run_systemctl_user(&["restart", "wayscriber.service"])?;
                }
            }
            Ok(format!("Configured GNOME shortcut: {normalized}"))
        }
        ShortcutApplyCapability::PortalServiceDropIn => {
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
        ShortcutApplyCapability::Manual => Err(
            "Automatic shortcut setup is not available in this desktop session; bind `wayscriber --daemon-toggle` manually."
                .to_string(),
        ),
    }
}

fn apply_gnome_custom_shortcut(binding: &str) -> Result<(), String> {
    require_gsettings_available()?;
    let toggle_command = render_toggle_command()?;

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

    let schema_with_path = gnome_shortcut_schema_with_path();
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
        &gvariant_string_literal(&toggle_command),
    ])?;
    run_gsettings_command(&[
        "set",
        &schema_with_path,
        "binding",
        &gvariant_string_literal(binding),
    ])?;

    Ok(())
}

fn render_toggle_command() -> Result<String, String> {
    let binary_path = resolve_wayscriber_binary_path()?;
    Ok(format!(
        "{} --daemon-toggle",
        shell_quote(binary_path.to_string_lossy().as_ref())
    ))
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
    let custom_keybindings = run_command(
        "gsettings",
        &["get", GNOME_MEDIA_KEYS_SCHEMA, GNOME_MEDIA_KEYS_KEY],
    )
    .ok()?;
    if !custom_keybindings.success {
        return None;
    }

    let schema_with_path = gnome_shortcut_schema_with_path();
    let binding = run_command("gsettings", &["get", &schema_with_path, "binding"]).ok()?;
    if !binding.success {
        return None;
    }
    resolve_gnome_shortcut_from_gsettings(&custom_keybindings.stdout, &binding.stdout)
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
    let contents = render_portal_shortcut_dropin(&escaped_shortcut, &escaped_app_id);
    fs::write(&dropin_path, contents).map_err(|err| {
        format!(
            "Failed to write portal shortcut drop-in {}: {}",
            dropin_path.display(),
            err
        )
    })?;
    Ok(dropin_path)
}

fn render_portal_shortcut_dropin(escaped_shortcut: &str, escaped_app_id: &str) -> String {
    format!(
        "[Service]\nEnvironment=\"{PORTAL_SHORTCUT_OPT_IN_ENV}=1\"\nEnvironment=\"{PORTAL_SHORTCUT_ENV}={escaped_shortcut}\"\nEnvironment=\"{PORTAL_APP_ID_ENV}={escaped_app_id}\"\n"
    )
}

fn read_portal_shortcut_from_dropin() -> Option<String> {
    let path = portal_shortcut_dropin_path()?;
    let content = fs::read_to_string(path).ok()?;
    parse_portal_shortcut_from_dropin(&content)
}

fn parse_portal_shortcut_from_dropin(content: &str) -> Option<String> {
    shared_parse_portal_shortcut_from_dropin(content)
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

fn resolve_gnome_shortcut_from_gsettings(
    custom_keybindings_output: &str,
    binding_output: &str,
) -> Option<String> {
    gnome_effective_shortcut(custom_keybindings_output, binding_output)
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
    fn parse_portal_shortcut_ignores_blank_value() {
        let content = "[Service]\nEnvironment=\"WAYSCRIBER_PORTAL_SHORTCUT=   \"\n";
        assert_eq!(parse_portal_shortcut_from_dropin(content), None);
    }

    #[test]
    fn render_portal_shortcut_dropin_includes_explicit_opt_in_marker() {
        let rendered = render_portal_shortcut_dropin("<Ctrl><Shift>g", PORTAL_APP_ID);
        assert!(rendered.contains("Environment=\"WAYSCRIBER_ENABLE_PORTAL_SHORTCUTS=1\""));
        assert!(rendered.contains("Environment=\"WAYSCRIBER_PORTAL_SHORTCUT=<Ctrl><Shift>g\""));
        assert!(rendered.contains("Environment=\"WAYSCRIBER_PORTAL_APP_ID=wayscriber\""));
    }

    #[test]
    fn resolve_gnome_shortcut_requires_registered_path() {
        let custom_keybindings =
            "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/not-wayscriber/']";
        assert_eq!(
            resolve_gnome_shortcut_from_gsettings(custom_keybindings, "'<Super>g'"),
            None
        );
    }

    #[test]
    fn resolve_gnome_shortcut_rejects_disabled_binding() {
        let custom_keybindings = "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/']";
        assert_eq!(
            resolve_gnome_shortcut_from_gsettings(custom_keybindings, "'disabled'"),
            None
        );
        assert_eq!(
            resolve_gnome_shortcut_from_gsettings(custom_keybindings, "''"),
            None
        );
    }

    #[test]
    fn resolve_gnome_shortcut_accepts_registered_binding() {
        let custom_keybindings = "['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/wayscriber-toggle/']";
        assert_eq!(
            resolve_gnome_shortcut_from_gsettings(custom_keybindings, "'<Super>g'"),
            Some("<Super>g".to_string())
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

    #[test]
    fn shell_quote_leaves_simple_paths_unquoted() {
        assert_eq!(shell_quote("/usr/bin/wayscriber"), "/usr/bin/wayscriber");
    }

    #[test]
    fn shell_quote_escapes_spaces_and_single_quotes() {
        assert_eq!(
            shell_quote("/tmp/My App/way'scriber"),
            "'/tmp/My App/way'\\''scriber'"
        );
    }
}
