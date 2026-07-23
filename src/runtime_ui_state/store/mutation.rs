use super::{RuntimeUiStateInspection, RuntimeUiStateStore, fs as store_fs, inspection};
use crate::runtime_ui_state::{
    AcceptedStateRevision, RuntimeStateFailurePathEffect, RuntimeStateIoError,
    RuntimeStateObservedPathEffect, RuntimeStatePathIdentity, RuntimeStatePostClaimPathEffect,
    RuntimeStateRecoveryArtifact, RuntimeStateSourceObservation, RuntimeStateSourceRevision,
    RuntimeUiFileStatus, SourceMutationId, SourceMutationKind, SourceMutationRequest,
    SourceMutationResult, encode_runtime_ui_file,
};

mod conflict;
mod replace;
mod reset;
mod result;

use result::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutationMode {
    Replace,
    ResetSupported,
    ResetPreservingUnsupported,
    ResetPreservingInvalid,
}

struct ClaimedSource {
    quarantine: store_fs::PinnedPath,
    inspection: RuntimeUiStateInspection,
}

struct MutationTarget {
    operation_path: store_fs::PinnedPath,
    expected_path: RuntimeStatePathIdentity,
}

struct PreparedReplacement<'a> {
    temp: &'a mut store_fs::CleanupPath,
    bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MutationPoint {
    AfterClaim,
    BeforeInstall,
    AfterInstall,
    BeforeFinalInspection,
}

impl RuntimeUiStateStore {
    pub(crate) fn execute_source_mutation(
        &self,
        request: SourceMutationRequest,
    ) -> SourceMutationResult {
        self.execute_source_mutation_inner(request, &mut |_| {})
    }

    pub(in crate::runtime_ui_state) fn execute_preserve_invalid(
        &self,
        mutation_id: SourceMutationId,
        expected: RuntimeStateSourceRevision,
    ) -> SourceMutationResult {
        self.execute_conditional(
            mutation_id,
            AcceptedStateRevision(0),
            expected,
            MutationMode::ResetPreservingInvalid,
            None,
            &mut |_| {},
        )
    }

    #[cfg(test)]
    pub(super) fn execute_source_mutation_with_hook<F>(
        &self,
        request: SourceMutationRequest,
        hook: &mut F,
    ) -> SourceMutationResult
    where
        F: FnMut(MutationPoint),
    {
        self.execute_source_mutation_inner(request, hook)
    }

    fn execute_source_mutation_inner(
        &self,
        request: SourceMutationRequest,
        hook: &mut dyn FnMut(MutationPoint),
    ) -> SourceMutationResult {
        let SourceMutationRequest {
            id,
            accepted_through,
            expected_source,
            kind,
            ..
        } = request;
        match kind {
            SourceMutationKind::Replace(wire) => {
                let bytes = match encode_runtime_ui_file(&wire) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        return self.failed_untouched(
                            id,
                            format!("could not encode runtime state: {}", error.message()),
                        );
                    }
                };
                self.execute_conditional(
                    id,
                    accepted_through,
                    expected_source,
                    MutationMode::Replace,
                    Some(bytes),
                    hook,
                )
            }
            SourceMutationKind::ResetSupported { .. } => self.execute_conditional(
                id,
                accepted_through,
                expected_source,
                MutationMode::ResetSupported,
                None,
                hook,
            ),
            SourceMutationKind::ResetUnsupportedIfUnchanged {
                confirmation_revision,
                ..
            } => {
                if confirmation_revision != expected_source {
                    return self.failed_untouched(
                        id,
                        "unsupported reset confirmation does not match its expected source",
                    );
                }
                self.execute_conditional(
                    id,
                    accepted_through,
                    expected_source,
                    MutationMode::ResetPreservingUnsupported,
                    None,
                    hook,
                )
            }
        }
    }

    fn execute_conditional(
        &self,
        id: SourceMutationId,
        applied_through: AcceptedStateRevision,
        expected: RuntimeStateSourceRevision,
        mode: MutationMode,
        replacement: Option<Vec<u8>>,
        hook: &mut dyn FnMut(MutationPoint),
    ) -> SourceMutationResult {
        if expected.path_identity().source_path() != self.path() {
            return self.failed_untouched(
                id,
                "mutation source identity does not name the managed path",
            );
        }

        let expected_path = expected.path_identity().clone();
        let Some(resolved_parent) = expected_path.resolved_parent() else {
            return self.failed_untouched(
                id,
                "mutation source identity has no observed parent directory",
            );
        };
        let parent = match store_fs::PinnedParent::open(resolved_parent) {
            Ok(parent) => parent,
            Err(error) => {
                return match self.inspect() {
                    Ok(active) if active.observation.revision != expected => {
                        SourceMutationResult::SourceChangedBeforeMutation {
                            id,
                            active: active.observation,
                        }
                    }
                    Ok(active) => failed(
                        id,
                        format!("could not pin runtime-state parent: {error}"),
                        Some(active.observation),
                        Vec::new(),
                        untouched(),
                    ),
                    Err(inspect_error) => failed(
                        id,
                        format!(
                            "could not pin runtime-state parent ({error}) or inspect the active path: {}",
                            inspect_error.message()
                        ),
                        None,
                        Vec::new(),
                        untouched(),
                    ),
                };
            }
        };
        let operation_path = match self
            .path()
            .file_name()
            .map(|leaf| parent.join(leaf.to_os_string()))
        {
            Some(Ok(path)) => path,
            Some(Err(error)) => {
                return self
                    .failed_untouched(id, format!("invalid runtime-state path leaf: {error}"));
            }
            None => return self.failed_untouched(id, "runtime-state path has no file name"),
        };
        let target = MutationTarget {
            operation_path,
            expected_path,
        };

        let mut temp = match replacement.as_deref() {
            Some(bytes) => match store_fs::create_synced_temp(&target.operation_path, bytes) {
                Ok(temp) => Some(temp),
                Err(error) => {
                    return self.failed_untouched(
                        id,
                        format!("could not prepare runtime-state write: {error}"),
                    );
                }
            },
            None => None,
        };
        let active = match self.inspect() {
            Ok(active) => active,
            Err(error) => return failed(id, error.message(), None, Vec::new(), untouched()),
        };
        if active.observation.revision != expected {
            return SourceMutationResult::SourceChangedBeforeMutation {
                id,
                active: active.observation,
            };
        }
        if !status_allowed(mode, &active.status) {
            return failed(
                id,
                "runtime-state command is not permitted for the observed file status",
                Some(active.observation),
                Vec::new(),
                untouched(),
            );
        }

        if expected.bytes().is_none() {
            return match mode {
                MutationMode::Replace => self.install_at_missing(
                    id,
                    applied_through,
                    &target,
                    PreparedReplacement {
                        temp: temp.as_mut().expect("replacement temp"),
                        bytes: replacement.as_deref().expect("replacement bytes"),
                    },
                    hook,
                ),
                MutationMode::ResetSupported => applied(id, applied_through, expected, Vec::new()),
                MutationMode::ResetPreservingUnsupported | MutationMode::ResetPreservingInvalid => {
                    failed(
                        id,
                        "preserving reset requires a present source",
                        Some(active.observation),
                        Vec::new(),
                        untouched(),
                    )
                }
            };
        }

        let quarantine = match store_fs::unique_recovery_path(&target.operation_path) {
            Ok(path) => path,
            Err(error) => {
                return failed(
                    id,
                    format!("could not reserve recovery path: {error}"),
                    Some(active.observation),
                    Vec::new(),
                    untouched(),
                );
            }
        };
        if let Err(error) = store_fs::rename_noreplace(&target.operation_path, &quarantine) {
            let current = self.inspect().ok().map(|inspection| inspection.observation);
            if let Some(ref current) = current
                && current.revision != expected
            {
                return SourceMutationResult::SourceChangedBeforeMutation {
                    id,
                    active: current.clone(),
                };
            }
            return failed(
                id,
                format!("could not claim runtime-state source: {error}"),
                current,
                Vec::new(),
                untouched(),
            );
        }
        hook(MutationPoint::AfterClaim);

        let mut claimed = match inspection::inspect_pinned(&quarantine) {
            Ok(claimed) => claimed,
            Err(error) => {
                return failed(
                    id,
                    format!(
                        "could not inspect claimed runtime state: {}",
                        error.message()
                    ),
                    self.inspect().ok().map(|inspection| inspection.observation),
                    Vec::new(),
                    RuntimeStateFailurePathEffect::UnknownAfterMutation,
                );
            }
        };
        let claimed_as_source = claimed
            .observation
            .revision
            .with_path_identity(expected.path_identity().clone());
        if claimed_as_source != expected {
            claimed.observation.revision = claimed_as_source;
            return self.restore_or_retain_changed_claim(
                id,
                &target,
                ClaimedSource {
                    quarantine,
                    inspection: claimed,
                },
            );
        }
        claimed.observation.revision = claimed_as_source;

        match mode {
            MutationMode::Replace => self.install_after_claim(
                id,
                applied_through,
                &target,
                ClaimedSource {
                    quarantine,
                    inspection: claimed,
                },
                PreparedReplacement {
                    temp: temp.as_mut().expect("replacement temp"),
                    bytes: replacement.as_deref().expect("replacement bytes"),
                },
                hook,
            ),
            MutationMode::ResetSupported => self.finish_reset_after_claim(
                id,
                applied_through,
                &target,
                ClaimedSource {
                    quarantine,
                    inspection: claimed,
                },
                false,
                hook,
            ),
            MutationMode::ResetPreservingUnsupported | MutationMode::ResetPreservingInvalid => self
                .finish_reset_after_claim(
                    id,
                    applied_through,
                    &target,
                    ClaimedSource {
                        quarantine,
                        inspection: claimed,
                    },
                    true,
                    hook,
                ),
        }
    }

    fn failed_untouched(
        &self,
        id: SourceMutationId,
        message: impl Into<String>,
    ) -> SourceMutationResult {
        failed(
            id,
            message,
            self.inspect().ok().map(|inspection| inspection.observation),
            Vec::new(),
            untouched(),
        )
    }
}
