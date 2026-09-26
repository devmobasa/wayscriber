pub(crate) type IconFn = fn(&cairo::Context, f64, f64, f64);

#[derive(Clone)]
pub(crate) struct Row {
    pub(crate) key: String,
    pub(crate) action: &'static str,
    /// The action a click on this row executes (rows describing gestures or
    /// multi-action pairs stay non-clickable).
    pub(crate) action_id: Option<crate::config::Action>,
}

impl Row {
    pub(crate) fn with_action(mut self, action: crate::config::Action) -> Self {
        self.action_id = Some(action);
        self
    }

    /// Whether this row only documents an action that has no binding. Such
    /// rows are hidden by default and shown on request or by search.
    pub(crate) fn is_unbound(&self) -> bool {
        self.key == crate::label_format::NOT_BOUND_LABEL
    }
}

#[derive(Clone)]
pub(crate) struct Badge {
    pub(crate) label: String,
    pub(crate) color: [f64; 3],
}

#[derive(Clone)]
pub(crate) struct Section {
    pub(crate) title: &'static str,
    pub(crate) rows: Vec<Row>,
    pub(crate) badges: Vec<Badge>,
    pub(crate) icon: Option<IconFn>,
}

#[derive(Clone)]
pub(crate) struct MeasuredSection {
    pub(crate) section: Section,
    pub(crate) width: f64,
    pub(crate) height: f64,
    /// Width of the widest action label. Key chips start just past it, so
    /// every row reads "label … keys" with the keys in one aligned column.
    pub(crate) label_column_width: f64,
    pub(crate) badge_text_metrics: Vec<BadgeTextMetrics>,
}

#[derive(Clone)]
pub(crate) struct BadgeTextMetrics {
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) y_bearing: f64,
}

pub(crate) fn row<T: Into<String>>(key: T, action: &'static str) -> Row {
    Row {
        key: key.into(),
        action,
        action_id: None,
    }
}

/// Screen-space rectangle of a clickable help row (or the "Replay tour" footer
/// entry), collected while the grid renders and fed into the pointer hit map so
/// clicks and cursor hints test the real drawn layout, not an approximation.
#[derive(Clone, Copy)]
pub(crate) struct HelpRowHit {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
    pub(crate) action: crate::config::Action,
}
