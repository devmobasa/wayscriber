use wayscriber::config::KeyBinding;

pub(crate) fn authored_shortcut_parts(value: &str) -> Vec<&str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

pub(crate) fn parse_keybinding_list(value: &str) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();
    for part in authored_shortcut_parts(value) {
        KeyBinding::parse(part)?;
        entries.push(part.to_string());
    }
    Ok(entries)
}

pub(crate) fn parse_keybindings(value: &str) -> Result<Vec<KeyBinding>, String> {
    let mut entries = Vec::new();
    for part in authored_shortcut_parts(value) {
        entries.push(KeyBinding::parse(part)?);
    }
    Ok(entries)
}
