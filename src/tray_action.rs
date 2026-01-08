#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayAction {
    ToggleFreeze,
    CaptureFull,
    CaptureWindow,
    CaptureRegion,
    ToggleHelp,
}

impl TrayAction {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            TrayAction::ToggleFreeze => "toggle_freeze",
            TrayAction::CaptureFull => "capture_full",
            TrayAction::CaptureWindow => "capture_window",
            TrayAction::CaptureRegion => "capture_region",
            TrayAction::ToggleHelp => "toggle_help",
        }
    }

    pub(crate) fn parse(action: &str) -> Option<Self> {
        match action {
            "toggle_freeze" => Some(TrayAction::ToggleFreeze),
            "capture_full" => Some(TrayAction::CaptureFull),
            "capture_window" => Some(TrayAction::CaptureWindow),
            "capture_region" => Some(TrayAction::CaptureRegion),
            "toggle_help" => Some(TrayAction::ToggleHelp),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TrayAction;

    #[test]
    fn tray_action_round_trip() {
        let actions = [
            TrayAction::ToggleFreeze,
            TrayAction::CaptureFull,
            TrayAction::CaptureWindow,
            TrayAction::CaptureRegion,
            TrayAction::ToggleHelp,
        ];

        for action in actions {
            assert_eq!(TrayAction::parse(action.as_str()), Some(action));
        }

        assert_eq!(TrayAction::parse("not-a-tray-action"), None);
    }
}
