pub fn parse_keybinding_list(value: &str) -> Result<Vec<String>, String> {
    let mut entries = Vec::new();

    for part in value.split(',') {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            entries.push(trimmed.to_string());
        }
    }

    Ok(entries)
}
