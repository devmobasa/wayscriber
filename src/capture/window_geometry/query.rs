use std::ffi::OsStr;
use std::time::Duration;

use crate::process_broker::{HelperKind, ProcessBroker, current};

use super::{
    WindowGeometryBackend, WindowGeometryError, WindowGeometryProvider, WindowQueryContext,
    WindowQueryResult, detect_backend,
};

const QUERY_TIMEOUT: Duration = Duration::from_secs(5);

static HYPRLAND_PROVIDER: super::hyprland::HyprlandProvider = super::hyprland::HyprlandProvider;
static SWAY_PROVIDER: super::sway::SwayProvider = super::sway::SwayProvider;

pub(crate) fn query_window_targets(
    context: &WindowQueryContext,
) -> Result<WindowQueryResult, WindowGeometryError> {
    let backend = detect_backend().ok_or(WindowGeometryError::Unsupported)?;
    let broker = current().map_err(|error| WindowGeometryError::Broker(format!("{error:#}")))?;
    let provider = provider_for_backend(backend);
    let targets = provider.query(&broker, context)?;
    Ok(WindowQueryResult {
        backend: provider.backend(),
        targets,
    })
}

fn provider_for_backend(backend: WindowGeometryBackend) -> &'static dyn WindowGeometryProvider {
    match backend {
        WindowGeometryBackend::Hyprland => &HYPRLAND_PROVIDER,
        WindowGeometryBackend::Sway => &SWAY_PROVIDER,
    }
}

pub(super) fn run(
    broker: &ProcessBroker,
    backend: WindowGeometryBackend,
    kind: HelperKind,
    program: &str,
    arguments: &[&str],
    output_cap: usize,
) -> Result<Vec<u8>, WindowGeometryError> {
    let output = broker
        .run(
            kind,
            OsStr::new(program),
            arguments.iter().map(OsStr::new),
            Vec::new(),
            QUERY_TIMEOUT,
            output_cap,
        )
        .map_err(|error| WindowGeometryError::Broker(format!("{error:#}")))?;
    if output.timed_out {
        return Err(WindowGeometryError::CommandFailed {
            backend,
            message: format!("{program} timed out"),
        });
    }
    if output.status != 0 {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(WindowGeometryError::CommandFailed {
            backend,
            message: if detail.is_empty() {
                format!("{program} exited with status {}", output.status)
            } else {
                detail.to_owned()
            },
        });
    }
    if output.stdout_limit_reached {
        return Err(WindowGeometryError::CommandFailed {
            backend,
            message: format!("{program} response exceeded its size limit"),
        });
    }
    Ok(output.stdout)
}
