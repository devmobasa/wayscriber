use crate::config::{
    ToolbarItemId, ToolbarItemOrderGroup, ToolbarItemVisibilitySetting as ItemVisibilitySetting,
    TopDisplayMode,
};
use crate::ui::toolbar::{SidePane, ToolbarSideSection};

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub(crate) struct $name(pub(crate) u64);

        impl $name {
            pub(crate) const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

id_type!(ControllerId);
id_type!(AcceptedStateRevision);
id_type!(SourceMutationId);
id_type!(ControllerBarrierId);
id_type!(PersistenceIncidentId);
id_type!(RecoveryAttemptId);
id_type!(RecoveryHandleId);
id_type!(RecoveryCommandId);
id_type!(RecoveryLeaseNonce);
id_type!(FlushRequestId);
id_type!(UnsupportedResetConfirmationId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NormalizedF64(u64);

impl NormalizedF64 {
    pub(crate) fn new(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        let normalized = if value == 0.0 { 0.0 } else { value };
        Some(Self(normalized.to_bits()))
    }

    pub(crate) fn get(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolbarPositionSeed {
    pub(crate) x: NormalizedF64,
    pub(crate) y: NormalizedF64,
}

impl ToolbarPositionSeed {
    pub(crate) fn new(x: f64, y: f64) -> Option<Self> {
        Some(Self {
            x: NormalizedF64::new(x)?,
            y: NormalizedF64::new(y)?,
        })
    }
}

/// The persistable half of [`TopDisplayMode`].
///
/// `Hidden` is a runtime-only rung of the cycle action, so it never reaches
/// the override store; [`PersistedTopDisplayMode::from_display_mode`] folds it
/// to `Full` exactly like [`TopDisplayMode::persisted`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum PersistedTopDisplayMode {
    Full,
    Micro,
}

impl PersistedTopDisplayMode {
    pub(crate) fn from_display_mode(mode: TopDisplayMode) -> Self {
        match mode.persisted() {
            TopDisplayMode::Micro => Self::Micro,
            // `persisted()` already folded `Hidden` into `Full`.
            TopDisplayMode::Full | TopDisplayMode::Hidden => Self::Full,
        }
    }

    pub(crate) fn display_mode(self) -> TopDisplayMode {
        match self {
            Self::Full => TopDisplayMode::Full,
            Self::Micro => TopDisplayMode::Micro,
        }
    }

    pub(crate) fn wire_id(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Micro => "micro",
        }
    }

    pub(crate) fn from_wire_id(value: &str) -> Option<Self> {
        match value {
            "full" => Some(Self::Full),
            "micro" => Some(Self::Micro),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum InteractionSeedTarget {
    TopPinned,
    SidePinned,
    TopMinimized,
    SideMinimized,
    SidePane,
    CollapsedSection(ToolbarSideSection),
    ItemVisibility(ToolbarItemId),
    ItemOrder(ToolbarItemOrderGroup),
    BoardPin(String),
    TopPosition,
    SidePosition,
    TopDisplayMode,
    /// Whether status-bar segments respond to clicks.
    StatusBarInteractive,
    /// Whether one status-bar segment is shown.
    StatusBarItem(crate::config::StatusBarItem),
    /// The status bar as a whole.
    StatusBar,
    /// The board and page badges that float over the canvas.
    StatusBoardBadge,
    StatusPageBadge,
    FloatingBadgeAlways,
    /// Toolbar appearance and behaviour toggles.
    ToolbarIcons,
    ToolbarMoreColors,
    ToolbarContextAwareUi,
    ToolbarPresetToasts,
    ToolbarToolPreview,
    ToolbarDelaySliders,
    /// The history pane's custom-step section.
    HistoryCustomSection,
    /// The input HUD.
    InputHud,
    /// Whether one named toolbar section is shown. Distinct from
    /// `ItemVisibility`: a section's visibility has a layout-mode baseline and
    /// a legacy mirror behind it, so it is seeded and restored as its own
    /// value rather than as an individual item override.
    SectionVisibility(crate::config::ToolbarSectionFlag),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InteractionSeedValue {
    Bool(bool),
    SidePane(SidePane),
    Visibility(ItemVisibilitySetting),
    ItemOrder(Vec<ToolbarItemId>),
    Position(ToolbarPositionSeed),
    TopDisplayMode(PersistedTopDisplayMode),
}

impl InteractionSeedValue {
    pub(crate) fn matches_target(&self, target: &InteractionSeedTarget) -> bool {
        use InteractionSeedTarget as Target;
        matches!(
            (target, self),
            (
                Target::TopPinned
                    | Target::SidePinned
                    | Target::TopMinimized
                    | Target::SideMinimized
                    | Target::CollapsedSection(_)
                    | Target::BoardPin(_)
                    | Target::StatusBarInteractive
                    | Target::StatusBarItem(_)
                    | Target::StatusBar
                    | Target::StatusBoardBadge
                    | Target::StatusPageBadge
                    | Target::FloatingBadgeAlways
                    | Target::ToolbarIcons
                    | Target::ToolbarMoreColors
                    | Target::ToolbarContextAwareUi
                    | Target::ToolbarPresetToasts
                    | Target::ToolbarToolPreview
                    | Target::ToolbarDelaySliders
                    | Target::HistoryCustomSection
                    | Target::InputHud,
                Self::Bool(_),
            ) | (Target::SidePane, Self::SidePane(_))
                | (
                    Target::ItemVisibility(_) | Target::SectionVisibility(_),
                    Self::Visibility(_)
                )
                | (Target::ItemOrder(_), Self::ItemOrder(_))
                | (
                    Target::TopPosition | Target::SidePosition,
                    Self::Position(_)
                )
                | (Target::TopDisplayMode, Self::TopDisplayMode(_))
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuntimeUiFileStatus {
    Missing,
    Supported,
    UnsupportedReadOnly { version: Option<u64> },
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerBarrierOperation {
    RequestRuntimeUiReset,
    ResetSupported,
    ConfirmUnsupportedReset,
    ExternalAuthorityReconciliation,
    PersistenceFailureRecovery,
    StartupPersistenceRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryAttemptStep {
    Inspecting,
    AwaitingControllerDecision,
    SourceMutationInFlight(RecoveryCommandId),
    ProtocolFailureAwaitingSourceMutation(RecoveryCommandId),
    CleanupInFlight(RecoveryCommandId),
    CancellationPending(RecoveryCommandId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControllerBarrierPhase {
    Inspecting,
    WaitingForPrerequisite(SourceMutationId),
    Writing(SourceMutationId),
    Reinspecting,
    InstallingAuthority,
    ResolvingPreviews,
    PersistenceUnhealthy {
        incident: PersistenceIncidentId,
    },
    Recovering {
        incident: PersistenceIncidentId,
        attempt: RecoveryAttemptId,
        step: RecoveryAttemptStep,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActiveControllerBarrier {
    pub(crate) id: ControllerBarrierId,
    pub(crate) operation: ControllerBarrierOperation,
    pub(crate) phase: ControllerBarrierPhase,
}
