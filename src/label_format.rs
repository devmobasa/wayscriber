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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_binding_labels_returns_none_for_empty_input() {
        assert_eq!(join_binding_labels(&[]), None);
    }

    #[test]
    fn join_binding_labels_uses_shared_separator() {
        let labels = vec!["Ctrl+K".to_string(), "F1".to_string()];
        assert_eq!(
            join_binding_labels(&labels),
            Some("Ctrl+K / F1".to_string())
        );
    }

    #[test]
    fn format_binding_labels_uses_not_bound_fallback() {
        assert_eq!(format_binding_labels(&[]), NOT_BOUND_LABEL);
    }

    #[test]
    fn format_binding_labels_or_uses_custom_fallback() {
        assert_eq!(format_binding_labels_or(&[], "fallback"), "fallback");
    }

    #[test]
    fn format_binding_label_includes_optional_binding_text() {
        assert_eq!(
            format_binding_label("Undo", Some("Ctrl+Z")),
            "Undo (Ctrl+Z)"
        );
        assert_eq!(format_binding_label("Undo", None), "Undo");
    }
}
