mod side;
mod top;

use crate::ui::toolbar::ToolbarSnapshot;

#[derive(Debug, Clone, Copy)]
pub(in crate::backend::wayland::toolbar) struct ToolbarLayoutSpec {
    use_icons: bool,
}

impl ToolbarLayoutSpec {
    pub(in crate::backend::wayland::toolbar) fn new(snapshot: &ToolbarSnapshot) -> Self {
        Self {
            use_icons: snapshot.use_icons,
        }
    }
}
