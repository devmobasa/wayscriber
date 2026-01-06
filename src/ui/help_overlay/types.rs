pub(crate) type IconFn = fn(&cairo::Context, f64, f64, f64);

#[derive(Clone)]
pub(crate) struct Row {
    pub(crate) key: String,
    pub(crate) action: &'static str,
}

#[derive(Clone)]
pub(crate) struct Badge {
    pub(crate) label: &'static str,
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
    pub(crate) key_column_width: f64,
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
    }
}
