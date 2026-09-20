/// HTTPS with an exact trusted host, followed by a path, query, fragment, or end.
pub fn is_trusted_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    ["wayscriber.com", "www.wayscriber.com"].iter().any(|host| {
        rest.strip_prefix(host)
            .is_some_and(|tail| tail.is_empty() || tail.starts_with(['/', '?', '#']))
    })
}
