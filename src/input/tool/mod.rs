//! Drawing tool selection and catalog metadata.

mod catalog;
mod drag;
mod drawing;
mod kind;
mod profile;
mod settings;

#[cfg(test)]
mod tests;

#[expect(
    unused_imports,
    reason = "Tool::descriptor exposes this crate-visible catalog interface"
)]
pub(crate) use catalog::ToolDescriptor;
pub(crate) use catalog::{
    ToolDrawingBehavior, ToolMotionBehavior, ToolMotionSizeSource, ToolPathKind, ToolPressBehavior,
    ToolPressureBehavior,
};
pub use drag::{DragBindableTool, DragTool};
#[expect(
    unused_imports,
    reason = "FinishedToolStroke exposes usage metadata to crate callers"
)]
pub(crate) use drawing::ToolUsage;
pub(crate) use drawing::{
    FinishedToolStroke, PolygonProvisionalSnapshot, PolygonStrokeSnapshot, ProvisionalToolSnapshot,
    ProvisionalToolStroke, ToolStrokeSnapshot,
};
pub use kind::Tool;
pub(crate) use profile::{ToolControlGroup, ToolProfile, ToolSettingsSlot, ToolSizeSource};
pub use settings::{EraserMode, PerToolDrawingSettings, ToolDrawingSettings};
