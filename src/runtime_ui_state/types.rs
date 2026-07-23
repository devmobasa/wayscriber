use crate::config::{
    ToolbarItemId, ToolbarItemOrderGroup, ToolbarItemVisibilitySetting as ItemVisibilitySetting,
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
}

impl InteractionSeedTarget {
    pub(crate) fn is_runtime_owned(&self) -> bool {
        !matches!(self, Self::TopPosition | Self::SidePosition)
    }

    pub(crate) fn is_config_position(&self) -> bool {
        matches!(self, Self::TopPosition | Self::SidePosition)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InteractionSeedValue {
    Bool(bool),
    SidePane(SidePane),
    Visibility(ItemVisibilitySetting),
    ItemOrder(Vec<ToolbarItemId>),
    Position(ToolbarPositionSeed),
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
                    | Target::BoardPin(_),
                Self::Bool(_),
            ) | (Target::SidePane, Self::SidePane(_))
                | (Target::ItemVisibility(_), Self::Visibility(_))
                | (Target::ItemOrder(_), Self::ItemOrder(_))
                | (
                    Target::TopPosition | Target::SidePosition,
                    Self::Position(_)
                )
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
