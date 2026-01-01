mod side;
mod top;

use crate::config::ToolbarLayoutMode;
use crate::ui::toolbar::ToolbarSnapshot;

#[derive(Debug, Clone, Copy)]
pub(in crate::backend::wayland::toolbar) struct ToolbarLayoutSpec {
    use_icons: bool,
    layout_mode: ToolbarLayoutMode,
    shape_picker_open: bool,
}

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) fn new(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            use_icons: snapshot.use_icons,
            layout_mode: snapshot.layout_mode,
            shape_picker_open: snapshot.shape_picker_open,
        }
    }

    pub(in crate::backend::wayland::toolbar) fn use_icons(&self) -> bool {
        self.use_icons
    }
}
