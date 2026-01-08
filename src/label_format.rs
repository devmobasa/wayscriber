pub(crate) const NOT_BOUND_LABEL: &str = "Not bound";
const BINDING_JOIN: &str = " / ";

pub(crate) fn join_binding_labels(labels: &[String]) -> Option<String> {
    if labels.is_empty() {
        None
    } else {
        Some(labels.join(BINDING_JOIN))
    }
}

pub(crate) fn format_binding_labels(labels: &[String]) -> String {
    join_binding_labels(labels).unwrap_or_else(|| NOT_BOUND_LABEL.to_string())
}

pub(crate) fn format_binding_labels_or(labels: &[String], fallback: &str) -> String {
    join_binding_labels(labels).unwrap_or_else(|| fallback.to_string())
}

#[allow(dead_code)]
pub(crate) fn format_binding_label(label: &str, binding: Option<&str>) -> String {
    if let Some(binding) = binding {
        format!("{label} ({binding})")
    } else {
        label.to_string()
    }
}
