use crate::i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionAction {
    Copy,
    Save,
    Both,
    Board,
    CutBand,
    UndoCut,
    RedoCut,
    ResetCuts,
    ToggleIncludeDrawings,
}

impl RegionAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Copy => tr("Copy"),
            Self::Save => tr("Save"),
            Self::Both => tr("Both"),
            Self::Board => tr("Board"),
            Self::CutBand => tr("Cut"),
            Self::UndoCut => tr("Undo"),
            Self::RedoCut => tr("Redo"),
            Self::ResetCuts => tr("Reset"),
            Self::ToggleIncludeDrawings => tr("Include drawings in exports"),
        }
    }

    pub(crate) const fn shortcut(self) -> &'static str {
        match self {
            Self::Copy => "Ctrl+C",
            Self::Save => "Ctrl+S",
            Self::Both => "Enter",
            Self::Board => "B",
            Self::CutBand => "X",
            Self::UndoCut => "Ctrl+Z",
            Self::RedoCut => "Ctrl+Y",
            Self::ResetCuts => "",
            Self::ToggleIncludeDrawings => "D",
        }
    }

    /// Destinations that leave Review. Edit controls stay in the picker.
    pub(crate) const fn is_terminal(self) -> bool {
        matches!(self, Self::Copy | Self::Save | Self::Both | Self::Board)
    }

    /// The accented default action: the one `Enter` submits.
    pub(super) const fn is_primary(self) -> bool {
        matches!(self, Self::Both)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionActionAvailability {
    pub terminal: bool,
    pub cut: bool,
    pub undo: bool,
    pub redo: bool,
    pub reset: bool,
}

impl RegionActionAvailability {
    /// Resting Review bar: terminals and Cut enabled, history empty.
    pub(crate) const DEFAULT: Self = Self {
        terminal: true,
        cut: true,
        undo: false,
        redo: false,
        reset: false,
    };

    pub(crate) const fn allows(self, action: RegionAction) -> bool {
        match action {
            RegionAction::Copy | RegionAction::Save | RegionAction::Both | RegionAction::Board => {
                self.terminal
            }
            RegionAction::CutBand => self.cut,
            RegionAction::UndoCut => self.undo,
            RegionAction::RedoCut => self.redo,
            RegionAction::ResetCuts => self.reset,
            RegionAction::ToggleIncludeDrawings => true,
        }
    }
}

impl Default for RegionActionAvailability {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegionCutStatus {
    Updating,
    Failed,
}

impl RegionCutStatus {
    pub(super) const fn message(self) -> &'static str {
        match self {
            Self::Updating => "Updating cut preview…",
            Self::Failed => "Cut preview failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RegionActionBarVisual {
    pub hovered: Option<RegionAction>,
    pub include_drawings: bool,
    pub availability: RegionActionAvailability,
    pub cut_armed: bool,
    pub status: Option<RegionCutStatus>,
}

#[cfg(test)]
impl RegionActionBarVisual {
    pub(crate) const fn simple(hovered: Option<RegionAction>, include_drawings: bool) -> Self {
        Self {
            hovered,
            include_drawings,
            availability: RegionActionAvailability::DEFAULT,
            cut_armed: false,
            status: None,
        }
    }
}
