use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SessionSaveReason {
    Autosave,
    Shutdown,
}

impl SessionSaveReason {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Autosave => "autosave",
            Self::Shutdown => "shutdown",
        }
    }
}

pub(super) fn log_snapshot_capture(
    reason: SessionSaveReason,
    options: &session::SessionOptions,
    snapshot: Option<&session::SessionSnapshot>,
    elapsed: Duration,
) {
    let Some(_snapshot) = snapshot else {
        log::info!(
            "Captured {} session snapshot for {} in {:?}: no persistable data",
            reason.label(),
            options.session_file_path().display(),
            elapsed
        );
        return;
    };

    log::info!(
        "Captured {} session snapshot for {} in {:?}; diagnostics and payload preparation will run on the persistence worker",
        reason.label(),
        options.session_file_path().display(),
        elapsed
    );
}

pub(super) fn log_session_save_result(
    reason: SessionSaveReason,
    report: Option<&SaveSnapshotReport>,
    elapsed: Duration,
) {
    let Some(report) = report else {
        log::info!(
            "Finished {} session persistence in {:?}: no file write needed",
            reason.label(),
            elapsed
        );
        return;
    };

    log::info!(
        "Finished {} session persistence in {:?}: outcome={:?}, written={} bytes, raw={} bytes, compression={}, path={}",
        reason.label(),
        elapsed,
        report.outcome,
        report.written_size,
        report.raw_size,
        report.compressed,
        report.path.display()
    );
}
