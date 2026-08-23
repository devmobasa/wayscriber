//! Callback identities and configure normalization.

use crate::pin::PinId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ShellEventIdentity {
    pub(crate) pin_id: PinId,
    pub(crate) shell_generation: u64,
    pub(crate) token: u64,
}
