use serde::Deserialize;

use crate::process_broker::{HelperKind, ProcessBroker};
use crate::util::Rect;

use super::geometry::{correlated_output_overlap, intersect, localize};
use super::query::run;
use super::{
    WindowGeometryBackend, WindowGeometryError, WindowGeometryProvider, WindowQueryContext,
    WindowTarget,
};

const WINDOWS_OUTPUT_CAP: usize = 16 * 1024 * 1024;

pub(super) struct SwayProvider;

impl WindowGeometryProvider for SwayProvider {
    fn backend(&self) -> WindowGeometryBackend {
        WindowGeometryBackend::Sway
    }

    fn query(
        &self,
        broker: &ProcessBroker,
        context: &WindowQueryContext,
    ) -> Result<Vec<WindowTarget>, WindowGeometryError> {
        let backend = self.backend();
        let tree = run(
            broker,
            backend,
            HelperKind::Swaymsg,
            "swaymsg",
            &["-t", "get_tree", "-r"],
            WINDOWS_OUTPUT_CAP,
        )?;
        parse_targets(&tree, context)
            .map_err(|message| WindowGeometryError::InvalidResponse { backend, message })
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct SwayRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl SwayRect {
    fn checked(self) -> Option<Rect> {
        Rect::new(self.x, self.y, self.width, self.height)
    }
}

#[derive(Debug, Deserialize)]
struct Node {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    visible: bool,
    rect: SwayRect,
    #[serde(default)]
    nodes: Vec<Node>,
    #[serde(default)]
    floating_nodes: Vec<Node>,
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    window: Option<i64>,
    #[serde(default)]
    pid: Option<i64>,
    #[serde(default)]
    window_properties: Option<serde_json::Value>,
}

pub(in crate::capture) fn parse_targets(
    tree: &[u8],
    context: &WindowQueryContext,
) -> Result<Vec<WindowTarget>, String> {
    let tree: Node = serde_json::from_slice(tree)
        .map_err(|error| format!("could not parse get_tree JSON: {error}"))?;
    let output = tree
        .nodes
        .iter()
        .find(|node| {
            node.kind == "output" && node.name.as_deref() == Some(context.output_name.as_str())
        })
        .ok_or_else(|| format!("output {:?} is not in the Sway tree", context.output_name))?;
    let output_rect = output
        .rect
        .checked()
        .ok_or_else(|| format!("output {:?} has invalid geometry", context.output_name))?;
    let clip_rect = correlated_output_overlap(output_rect, context.output_logical_rect)
        .ok_or_else(|| {
            format!(
                "output {:?} no longer matches the source",
                context.output_name
            )
        })?;

    let workspace = output
        .nodes
        .iter()
        .find(|node| node.kind == "workspace" && node.visible);
    let workspace = workspace.ok_or_else(|| {
        format!(
            "output {:?} has no matching visible workspace",
            context.output_name
        )
    })?;

    let mut targets = Vec::new();
    collect_visible_leaves(&workspace.nodes, clip_rect, context, &mut targets);
    collect_visible_leaves(&workspace.floating_nodes, clip_rect, context, &mut targets);
    Ok(targets)
}

fn collect_visible_leaves(
    nodes: &[Node],
    clip_rect: Rect,
    context: &WindowQueryContext,
    targets: &mut Vec<WindowTarget>,
) {
    for node in nodes {
        if !node.visible {
            continue;
        }
        let is_container = matches!(node.kind.as_str(), "con" | "floating_con");
        let is_window = node.app_id.is_some()
            || node.window.is_some()
            || node.pid.is_some()
            || node.window_properties.is_some();
        if node.nodes.is_empty() && node.floating_nodes.is_empty() && is_container && is_window {
            let Some(rect) = node.rect.checked() else {
                continue;
            };
            let Some(clipped) = intersect(rect, clip_rect) else {
                continue;
            };
            let Some(logical_rect) = localize(clipped, context.output_logical_rect) else {
                continue;
            };
            targets.push(WindowTarget {
                id: node.id.to_string(),
                title: node.name.clone().unwrap_or_default(),
                logical_rect,
            });
            continue;
        }
        collect_visible_leaves(&node.nodes, clip_rect, context, targets);
        collect_visible_leaves(&node.floating_nodes, clip_rect, context, targets);
    }
}
