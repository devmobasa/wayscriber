use serde::Deserialize;

use crate::process_broker::{HelperKind, ProcessBroker};
use crate::util::Rect;

use super::geometry::{correlated_output_overlap, intersect, localize};
use super::query::run;
use super::{
    WindowGeometryBackend, WindowGeometryError, WindowGeometryProvider, WindowQueryContext,
    WindowTarget,
};

const MONITORS_OUTPUT_CAP: usize = 2 * 1024 * 1024;
const WINDOWS_OUTPUT_CAP: usize = 16 * 1024 * 1024;

pub(super) struct HyprlandProvider;

impl WindowGeometryProvider for HyprlandProvider {
    fn backend(&self) -> WindowGeometryBackend {
        WindowGeometryBackend::Hyprland
    }

    fn query(
        &self,
        broker: &ProcessBroker,
        context: &WindowQueryContext,
    ) -> Result<Vec<WindowTarget>, WindowGeometryError> {
        let backend = self.backend();
        let monitors = run(
            broker,
            backend,
            HelperKind::Hyprctl,
            "hyprctl",
            &["monitors", "-j"],
            MONITORS_OUTPUT_CAP,
        )?;
        let clients = run(
            broker,
            backend,
            HelperKind::Hyprctl,
            "hyprctl",
            &["clients", "-j"],
            WINDOWS_OUTPUT_CAP,
        )?;
        parse_targets(&monitors, &clients, context)
            .map_err(|message| WindowGeometryError::InvalidResponse { backend, message })
    }
}

#[derive(Debug, Deserialize)]
struct Workspace {
    id: i64,
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Monitor {
    id: i64,
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    scale: f64,
    transform: i32,
    #[serde(default)]
    disabled: bool,
    active_workspace: Workspace,
    #[serde(default)]
    special_workspace: Option<Workspace>,
}

#[derive(Debug, Deserialize)]
struct Client {
    address: String,
    #[serde(default)]
    title: String,
    mapped: bool,
    #[serde(default)]
    hidden: bool,
    at: [f64; 2],
    size: [f64; 2],
    monitor: i64,
    workspace: Workspace,
    #[serde(default)]
    pinned: bool,
}

pub(in crate::capture) fn parse_targets(
    monitors: &[u8],
    clients: &[u8],
    context: &WindowQueryContext,
) -> Result<Vec<WindowTarget>, String> {
    let monitors: Vec<Monitor> = serde_json::from_slice(monitors)
        .map_err(|error| format!("could not parse monitors JSON: {error}"))?;
    let clients: Vec<Client> = serde_json::from_slice(clients)
        .map_err(|error| format!("could not parse clients JSON: {error}"))?;
    let monitor = monitors
        .iter()
        .find(|monitor| monitor.name == context.output_name && !monitor.disabled)
        .ok_or_else(|| format!("output {:?} is not active", context.output_name))?;
    let monitor_rect = monitor_logical_rect(monitor)
        .ok_or_else(|| format!("output {:?} has invalid geometry", context.output_name))?;
    let visible_workspace = visible_workspace(monitor);
    let clip_rect = correlated_output_overlap(monitor_rect, context.output_logical_rect)
        .ok_or_else(|| {
            format!(
                "output {:?} no longer matches the source",
                context.output_name
            )
        })?;

    let mut targets = Vec::new();
    for client in clients {
        if !client.mapped || client.hidden || client.monitor != monitor.id {
            continue;
        }
        let workspace_matches = client.pinned
            || client.workspace.id == visible_workspace.id
            || client.workspace.name == visible_workspace.name;
        if !workspace_matches {
            continue;
        }
        let Some(window_rect) = rect_from_origin_size(client.at, client.size) else {
            continue;
        };
        let Some(clipped) = intersect(window_rect, clip_rect) else {
            continue;
        };
        let Some(logical_rect) = localize(clipped, context.output_logical_rect) else {
            continue;
        };
        targets.push(WindowTarget {
            id: client.address,
            title: client.title,
            logical_rect,
        });
    }
    Ok(targets)
}

fn visible_workspace(monitor: &Monitor) -> &Workspace {
    monitor
        .special_workspace
        .as_ref()
        .filter(|workspace| workspace.id != 0 || !workspace.name.is_empty())
        .unwrap_or(&monitor.active_workspace)
}

fn monitor_logical_rect(monitor: &Monitor) -> Option<Rect> {
    if !monitor.x.is_finite()
        || !monitor.y.is_finite()
        || !monitor.width.is_finite()
        || !monitor.height.is_finite()
        || !monitor.scale.is_finite()
        || monitor.scale <= 0.0
    {
        return None;
    }
    let (mode_width, mode_height) = if monitor.transform.rem_euclid(2) == 1 {
        (monitor.height, monitor.width)
    } else {
        (monitor.width, monitor.height)
    };
    let width = checked_round_i32(mode_width / monitor.scale)?;
    let height = checked_round_i32(mode_height / monitor.scale)?;
    Rect::new(
        checked_round_i32(monitor.x)?,
        checked_round_i32(monitor.y)?,
        width,
        height,
    )
}

fn rect_from_origin_size(origin: [f64; 2], size: [f64; 2]) -> Option<Rect> {
    if origin
        .into_iter()
        .chain(size)
        .any(|value| !value.is_finite())
        || size[0] <= 0.0
        || size[1] <= 0.0
    {
        return None;
    }
    let min_x = checked_floor_i32(origin[0])?;
    let min_y = checked_floor_i32(origin[1])?;
    let max_x = checked_ceil_i32(origin[0] + size[0])?;
    let max_y = checked_ceil_i32(origin[1] + size[1])?;
    Rect::from_min_max(min_x, min_y, max_x, max_y)
}

fn checked_round_i32(value: f64) -> Option<i32> {
    checked_i32(value.round())
}

fn checked_floor_i32(value: f64) -> Option<i32> {
    checked_i32(value.floor())
}

fn checked_ceil_i32(value: f64) -> Option<i32> {
    checked_i32(value.ceil())
}

fn checked_i32(value: f64) -> Option<i32> {
    (value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX)).then_some(value as i32)
}
