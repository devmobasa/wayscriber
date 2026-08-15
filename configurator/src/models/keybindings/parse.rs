use wayscriber::config::Shortcut;

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
        Shortcut::parse(part)?;
        entries.push(part.to_string());
    }
    Ok(entries)
}

pub(crate) fn parse_keybindings(value: &str) -> Result<Vec<Shortcut>, String> {
    let mut entries = Vec::new();
    for part in authored_shortcut_parts(value) {
        entries.push(Shortcut::parse(part)?);
    }
    Ok(entries)
}
