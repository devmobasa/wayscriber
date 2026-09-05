//! A popover and its capture resources have one lifecycle.
use super::*;

#[derive(Clone)]
pub(super) struct PopoverResources {
    pub(super) popover: gtk4::Popover,
    pub(super) capture_surface: CaptureSurfaceContent,
}

pub(super) struct PopoverOwner<K> {
    pub(super) mounted: Option<PopoverResources>,
    pub(super) expected_open: Rc<Cell<bool>>,
    pub(super) content_key: Option<K>,
    pub(super) updaters: Vec<Updater>,
}

impl<K> Default for PopoverOwner<K> {
    fn default() -> Self {
        Self {
            mounted: None,
            expected_open: Rc::new(Cell::new(false)),
            content_key: None,
            updaters: Vec::new(),
        }
    }
}

impl<K> PopoverOwner<K> {
    pub(super) fn install(
        &mut self,
        popover: gtk4::Popover,
        capture_surface: CaptureSurfaceContent,
    ) {
        self.clear();
        self.mounted = Some(PopoverResources {
            popover,
            capture_surface,
        });
    }

    pub(super) fn clear(&mut self) {
        self.expected_open.set(false);
        if let Some(resources) = self.mounted.take() {
            resources.popover.unparent();
        }
        self.content_key = None;
        self.updaters.clear();
    }

    pub(super) fn set_open(&self, open: bool) {
        self.expected_open.set(open && self.mounted.is_some());
        let Some(resources) = &self.mounted else {
            return;
        };
        if open && !resources.popover.is_visible() {
            resources.popover.popup();
        } else if !open && resources.popover.is_visible() {
            resources.popover.popdown();
        }
    }
}

impl PopoverResources {
    pub(super) fn set_capture_transparent(&self, transparent: bool) {
        if transparent && !self.popover.is_visible() {
            return;
        }
        super::popovers::set_popover_capture_transparent(
            &self.popover,
            &self.capture_surface,
            transparent,
            !transparent,
        );
    }
}
