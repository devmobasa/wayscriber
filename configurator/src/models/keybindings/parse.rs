#[cfg(test)]
thread_local! { static PARSE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(test)]
pub(crate) fn take_parse_calls() -> usize {
    PARSE_CALLS.with(|calls| calls.replace(0))
}

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
    #[cfg(test)]
    PARSE_CALLS.with(|calls| calls.set(calls.get() + 1));
    let mut entries = Vec::new();
    for part in authored_shortcut_parts(value) {
        entries.push(Shortcut::parse(part)?);
    }
    Ok(entries)
}
