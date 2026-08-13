/// Stable identity of one automatic onboarding tip.
///
/// The identity travels with the visible toast so acknowledgement cannot be
/// retargeted if the toast queue changes between pointer press and release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingTip {
    Help,
    CommandPalette,
    QuickAccess,
    StatusBar,
    CanvasPopover,
    ZoomChip,
    ShortcutCoach,
    ToolbarHidden,
}
