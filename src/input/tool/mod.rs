//! Drawing tool selection and catalog metadata.

mod catalog;
mod drawing;
mod profile;
mod settings;

#[cfg(test)]
mod tests;

pub use crate::domain::{DragBindableTool, DragTool, EraserMode, Tool};
#[expect(
    unused_imports,
    reason = "Tool::descriptor exposes this crate-visible catalog interface"
)]
pub(crate) use catalog::ToolDescriptor;
pub(crate) use catalog::{
    ToolDrawingBehavior, ToolMotionBehavior, ToolMotionSizeSource, ToolPathKind, ToolPressBehavior,
    ToolPressureBehavior,
};
#[expect(
    unused_imports,
    reason = "FinishedToolStroke exposes usage metadata to crate callers"
)]
pub(crate) use drawing::ToolUsage;
pub(crate) use drawing::{
    FinishedToolStroke, PROVISIONAL_POLYGON_DAMAGE_PADDING, PolygonProvisionalSnapshot,
    PolygonStrokeSnapshot, ProvisionalToolSnapshot, ProvisionalToolStroke, ToolStrokeSnapshot,
};
pub(crate) use profile::{ToolControlGroup, ToolProfile, ToolSettingsSlot, ToolSizeSource};
pub use settings::{PerToolDrawingSettings, ToolDrawingSettings};
