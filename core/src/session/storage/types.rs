#[derive(Debug, Clone, Copy)]
pub struct ClearOutcome {
    pub removed_session: bool,
    pub removed_backup: bool,
    pub removed_recovery: bool,
    pub removed_lock: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearToolStateOutcome {
    NoSession,
    NoToolState,
    Cleared { preserved_board_data: bool },
}
