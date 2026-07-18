use std::ffi::{OsStr, OsString};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub(super) const BROKER_FD: i32 = 3;
pub(super) const BROKER_FD_ENV: &str = "WAYSCRIBER_INTERNAL_PROCESS_BROKER_FD";
pub(super) const BROKER_SHUTDOWN_FD: i32 = 4;
pub(super) const BROKER_SHUTDOWN_FD_ENV: &str = "WAYSCRIBER_INTERNAL_PROCESS_BROKER_SHUTDOWN_FD";
pub(super) const BROKER_TOKEN_ENV: &str = "WAYSCRIBER_INTERNAL_PROCESS_BROKER_TOKEN";
pub(super) const MAX_PACKET_BYTES: usize = 64 * 1024;
pub(super) const INLINE_BLOB_BYTES: usize = 4 * 1024;
pub(super) const MAX_ARGUMENTS: usize = 64;
pub(super) const MAX_ARGUMENT_BYTES: usize = 16 * 1024;
pub(super) const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;
pub(super) const MAX_STDERR_BYTES: usize = 64 * 1024;
pub(super) const MAX_OWNED_CHILDREN: usize = 64;
pub(super) const MAX_PACKET_DESCRIPTORS: usize = 2;
pub(super) const REQUIRED_MEMFD_SEALS: i32 =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HelperKind {
    Overlay,
    InitialDetach,
    CapabilityProbe,
    Grim,
    Hyprctl,
    Slurp,
    WlPaste,
    WlCopy,
    SessionZenity,
    SessionKdialog,
    Gsettings,
    Configurator,
    About,
    DesktopOpen,
    #[cfg(test)]
    TestSleep,
    #[cfg(test)]
    TestCat,
    #[cfg(test)]
    TestShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HelperLifetime {
    OperationBound,
    OwnedChild,
    DetachedAfterExec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum OutputMode {
    /// Read the complete stream and reject output beyond the declared cap.
    Complete,
    /// Return once the declared stdout prefix is full and stop the helper.
    Prefix,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BrokerRequest {
    pub(super) token: String,
    pub(super) request_id: String,
    pub(super) operation: BrokerOperation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operation", deny_unknown_fields)]
pub(super) enum BrokerOperation {
    Ping,
    Run {
        kind: HelperKind,
        program: OsWire,
        arguments: Vec<OsWire>,
        environment: Vec<(OsWire, Option<OsWire>)>,
        input: BlobWire,
        timeout_ms: u64,
        output_cap: usize,
        output_mode: OutputMode,
    },
    Publish {
        kind: HelperKind,
        program: OsWire,
        arguments: Vec<OsWire>,
        environment: Vec<(OsWire, Option<OsWire>)>,
        input: BlobWire,
        timeout_ms: u64,
    },
    Spawn {
        kind: HelperKind,
        lifetime: HelperLifetime,
        watchdog: bool,
        program: OsWire,
        arguments: Vec<OsWire>,
        environment: Vec<(OsWire, Option<OsWire>)>,
    },
    Signal {
        handle: String,
        signal: i32,
    },
    TryWait {
        handle: String,
    },
    KillWait {
        handle: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BrokerResponse {
    pub(super) request_id: String,
    pub(super) outcome: BrokerOutcome,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome", deny_unknown_fields)]
pub(super) enum BrokerOutcome {
    Output {
        status: i32,
        stdout: BlobWire,
        stderr: BlobWire,
        timed_out: bool,
        stdout_limit_reached: bool,
    },
    Spawned {
        handle: String,
        pid: u32,
    },
    Running,
    Exited {
        status: i32,
    },
    Acknowledged,
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "storage", deny_unknown_fields)]
pub(super) enum BlobWire {
    Inline { bytes: Vec<u8> },
    SealedMemfd { length: usize },
}

pub(super) struct BrokerWireResponse {
    pub(super) outcome: BrokerOutcome,
    pub(super) descriptors: Vec<std::os::fd::OwnedFd>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct OsWire(pub(super) Vec<u8>);

impl OsWire {
    pub(super) fn from_os(value: &OsStr) -> Result<Self> {
        let bytes = std::os::unix::ffi::OsStrExt::as_bytes(value);
        if bytes.len() > MAX_ARGUMENT_BYTES || bytes.contains(&0) {
            bail!("broker argument is oversized or contains NUL");
        }
        Ok(Self(bytes.to_vec()))
    }

    pub(super) fn into_os(self) -> OsString {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(self.0)
    }
}

#[derive(Debug)]
pub(crate) struct BrokerOutput {
    pub(crate) status: i32,
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
    pub(crate) timed_out: bool,
    pub(crate) stdout_limit_reached: bool,
}
