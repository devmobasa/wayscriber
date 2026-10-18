//! Capture suppression for GTK-owned `GtkPopover` surfaces.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::glib::variant::ToVariant;
use gtk4::prelude::*;

use super::set_native_widget_input_enabled;
use crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS;
use crate::toolbar_gtk::view::{CaptureProofTarget, CaptureSurfaceContent};

const CAPTURE_MENU_PROOF_ID: &str = "wayscriber-capture-proof";

#[derive(Default)]
pub(super) struct NativePopoverCapture {
    captures: RefCell<Vec<NativePopoverSurface>>,
}

impl NativePopoverCapture {
    pub(super) fn enroll(
        &self,
        popover: &gtk4::Popover,
        selected_for_capture: bool,
        input_enabled: bool,
    ) {
        let mut captures = self.captures.borrow_mut();
        captures.retain(NativePopoverSurface::is_attached);
        if let Some(capture) = captures.iter().find(|capture| capture.matches(popover)) {
            capture.set_selected(selected_for_capture);
            if selected_for_capture {
                capture.set_capture_state(true, input_enabled);
            }
            return;
        }

        if !selected_for_capture {
            return;
        }

        if popover
            .child()
            .as_ref()
            .is_some_and(CaptureSurfaceContent::is_wrapper)
        {
            // Wayscriber's explicit popovers already own a proof wrapper and
            // are enrolled separately by TopBar.
            return;
        }

        let capture = NativePopoverSurface::new(popover);
        capture.set_selected(true);
        capture.set_capture_state(true, input_enabled);
        captures.push(capture);
    }

    pub(super) fn set_suppressed(&self, suppressed: bool) {
        if !suppressed {
            for capture in self.captures.take() {
                capture.restore();
            }
            return;
        }

        let mut captures = self.captures.borrow_mut();
        captures.retain(NativePopoverSurface::is_attached);
        for capture in captures.iter() {
            let selected = capture
                .popover()
                .is_some_and(|popover| popover.is_visible() || popover.is_mapped());
            capture.set_selected(selected);
            // Once enrolled, keep the last buffer transparent even while the
            // popup is unmapped. A later remap is then safe while its fresh
            // proof is being enrolled and presented.
            capture.set_capture_state(true, false);
        }
    }

    pub(super) fn set_input_enabled(&self, enabled: bool) {
        for capture in self.captures.borrow().iter() {
            if let Some(popover) = capture.popover() {
                set_native_widget_input_enabled(popover.upcast_ref(), enabled);
            }
        }
    }

    pub(super) fn capture_targets(&self) -> Vec<CaptureProofTarget> {
        self.captures
            .borrow()
            .iter()
            .filter_map(NativePopoverSurface::capture_target)
            .collect()
    }

    pub(super) fn pending_capture_targets(&self) -> Vec<CaptureProofTarget> {
        self.captures
            .borrow()
            .iter()
            .filter_map(NativePopoverSurface::pending_capture_target)
            .collect()
    }

    pub(super) fn mark_proven(&self) {
        for capture in self.captures.borrow().iter() {
            capture.mark_in_flight_proven();
        }
    }
}

struct NativePopoverSurface {
    popover: gtk4::glib::WeakRef<gtk4::Popover>,
    content: NativePopoverContent,
    selected_for_capture: Rc<Cell<bool>>,
    proof_epoch: Rc<Cell<u64>>,
    proven_epoch: Cell<u64>,
    in_flight_epoch: Cell<Option<u64>>,
    map_handler: gtk4::glib::SignalHandlerId,
    unmap_handler: gtk4::glib::SignalHandlerId,
}

impl NativePopoverSurface {
    fn new(popover: &gtk4::Popover) -> Self {
        let content = NativePopoverContent::new(popover);
        let selected_for_capture = Rc::new(Cell::new(false));
        let proof_epoch = Rc::new(Cell::new(0));

        let mapped_selected = Rc::clone(&selected_for_capture);
        let mapped_epoch = Rc::clone(&proof_epoch);
        let map_handler = popover.connect_map(move |_| {
            mapped_selected.set(true);
            rearm_proof(&mapped_epoch);
        });

        let unmapped_selected = Rc::clone(&selected_for_capture);
        let unmap_handler = popover.connect_unmap(move |_| {
            unmapped_selected.set(false);
        });

        Self {
            popover: popover.downgrade(),
            content,
            selected_for_capture,
            proof_epoch,
            proven_epoch: Cell::new(0),
            in_flight_epoch: Cell::new(None),
            map_handler,
            unmap_handler,
        }
    }

    fn popover(&self) -> Option<gtk4::Popover> {
        self.popover.upgrade()
    }

    fn is_attached(&self) -> bool {
        self.popover().is_some()
    }

    fn matches(&self, popover: &gtk4::Popover) -> bool {
        self.popover()
            .is_some_and(|candidate| candidate == *popover)
    }

    fn set_selected(&self, selected: bool) {
        let was_selected = self.selected_for_capture.replace(selected);
        if selected && !was_selected {
            rearm_proof(&self.proof_epoch);
        } else if !selected {
            self.in_flight_epoch.set(None);
        }
    }

    fn set_capture_state(&self, suppressed: bool, input_enabled: bool) {
        let Some(popover) = self.popover() else {
            return;
        };
        if suppressed {
            popover.add_css_class(CAPTURE_TRANSPARENT_CLASS);
        } else {
            popover.remove_css_class(CAPTURE_TRANSPARENT_CLASS);
        }
        self.content.set_transparent(&popover, suppressed);
        set_native_widget_input_enabled(popover.upcast_ref(), input_enabled);
    }

    fn restore(self) {
        let Some(popover) = self.popover() else {
            return;
        };
        popover.disconnect(self.map_handler);
        popover.disconnect(self.unmap_handler);
        popover.remove_css_class(CAPTURE_TRANSPARENT_CLASS);
        self.content.restore(&popover);
        set_native_widget_input_enabled(popover.upcast_ref(), true);
    }

    fn capture_target(&self) -> Option<CaptureProofTarget> {
        if !self.selected_for_capture.get() {
            return None;
        }
        self.in_flight_epoch.set(Some(self.proof_epoch.get()));
        let selected_for_capture = Rc::clone(&self.selected_for_capture);
        self.popover().map(|popover| {
            CaptureProofTarget::new_withdrawable_with_callback(
                "gtk-owned-popover",
                &popover,
                self.content.surface(),
                move || selected_for_capture.set(false),
            )
        })
    }

    fn pending_capture_target(&self) -> Option<CaptureProofTarget> {
        let epoch = self.proof_epoch.get();
        if !self.selected_for_capture.get() || epoch == self.proven_epoch.get() {
            return None;
        }
        self.capture_target()
    }

    fn mark_in_flight_proven(&self) {
        let Some(epoch) = self.in_flight_epoch.replace(None) else {
            return;
        };
        if self.selected_for_capture.get() && self.proof_epoch.get() == epoch {
            self.proven_epoch.set(epoch);
        }
    }
}

fn rearm_proof(epoch: &Cell<u64>) {
    epoch.set(epoch.get().wrapping_add(1));
}

struct MenuScrollLayout {
    scrolled: gtk4::ScrolledWindow,
    propagate_width: bool,
    propagate_height: bool,
    policy: (gtk4::PolicyType, gtk4::PolicyType),
}

impl MenuScrollLayout {
    fn hold(scrolled: &gtk4::ScrolledWindow) -> Self {
        let layout = Self {
            scrolled: scrolled.clone(),
            propagate_width: scrolled.propagates_natural_width(),
            propagate_height: scrolled.propagates_natural_height(),
            policy: scrolled.policy(),
        };
        // The temporary model contains only the proof. Its natural size and
        // scrollbar minimum lengths must not inflate the saved allocation.
        scrolled.set_propagate_natural_width(false);
        scrolled.set_propagate_natural_height(false);
        scrolled.set_policy(gtk4::PolicyType::External, gtk4::PolicyType::External);

        layout
    }

    fn restore(self) {
        self.scrolled.set_policy(self.policy.0, self.policy.1);
        self.scrolled
            .set_propagate_natural_width(self.propagate_width);
        self.scrolled
            .set_propagate_natural_height(self.propagate_height);
    }
}

enum NativePopoverContent {
    Wrapped(CaptureSurfaceContent),
    Menu {
        surface: CaptureSurfaceContent,
        proof_model: gtk4::gio::Menu,
        original_model: RefCell<Option<gtk4::gio::MenuModel>>,
        original_child_size: RefCell<Option<(gtk4::Widget, i32, i32)>>,
        original_scroll_layout: RefCell<Option<MenuScrollLayout>>,
    },
}

impl NativePopoverContent {
    fn new(popover: &gtk4::Popover) -> Self {
        if let Ok(menu) = popover.clone().downcast::<gtk4::PopoverMenu>() {
            let surface = CaptureSurfaceContent::empty();
            surface
                .widget()
                .add_css_class("wayscriber-capture-menu-proof");
            return Self::Menu {
                surface,
                proof_model: capture_menu_proof_model(),
                original_model: RefCell::new(menu.menu_model()),
                original_child_size: RefCell::new(None),
                original_scroll_layout: RefCell::new(None),
            };
        }

        let child = popover.child();
        if child.is_some() {
            popover.set_child(None::<&gtk4::Widget>);
        }
        let surface = child
            .as_ref()
            .map_or_else(CaptureSurfaceContent::empty, CaptureSurfaceContent::new);
        popover.set_child(Some(surface.widget()));
        Self::Wrapped(surface)
    }

    fn surface(&self) -> &CaptureSurfaceContent {
        match self {
            Self::Wrapped(surface) | Self::Menu { surface, .. } => surface,
        }
    }

    fn set_transparent(&self, popover: &gtk4::Popover, transparent: bool) {
        if let Self::Wrapped(surface) = self {
            let wrapper = surface.widget().clone().upcast::<gtk4::Widget>();
            if popover.child().as_ref() != Some(&wrapper) {
                let replacement = popover.child();
                if replacement.is_some() {
                    popover.set_child(None::<&gtk4::Widget>);
                }
                let _ = surface.take_content();
                if let Some(replacement) = replacement.as_ref() {
                    surface.set_content(replacement);
                }
                popover.set_child(Some(surface.widget()));
            }
            surface.set_transparent(transparent);
            return;
        }

        let Self::Menu {
            surface,
            proof_model,
            original_model,
            original_child_size,
            original_scroll_layout,
        } = self
        else {
            return;
        };
        surface.set_transparent(transparent);
        if !transparent {
            return;
        }
        let Ok(menu) = popover.clone().downcast::<gtk4::PopoverMenu>() else {
            return;
        };
        let proof_model_object = proof_model.clone().upcast::<gtk4::gio::MenuModel>();
        if menu.menu_model().as_ref() != Some(&proof_model_object) {
            // Keep the class-owned child's allocation while replacing its
            // model. The proof slot must not resize the native popup, and the
            // child itself remains owned and parented by GtkPopoverMenu.
            if original_child_size.borrow().is_none()
                && let Some(child) = menu.child()
            {
                let (width, height) = child.size_request();
                // GTK's public outer-allocation getters include CSS margins.
                // compute_bounds returns the border box and would shrink a
                // CSS-margined child when used as its minimum request.
                #[allow(deprecated)]
                let allocation = (child.allocated_width(), child.allocated_height());
                // The temporary model's natural requisition must not grow
                // beyond that allocation. Restore normal model propagation
                // together with its original minimum request afterward.
                if let Some(scrolled) = child.downcast_ref::<gtk4::ScrolledWindow>() {
                    original_scroll_layout.replace(Some(MenuScrollLayout::hold(scrolled)));
                }
                child.set_size_request(allocation.0, allocation.1);
                original_child_size.replace(Some((child, width, height)));
            }
            original_model.replace(menu.menu_model());
            menu.set_menu_model(Some(proof_model));
        }
        if surface.widget().parent().is_none()
            && !menu.add_child(surface.widget(), CAPTURE_MENU_PROOF_ID)
        {
            log::error!("could not install GTK menu capture proof widget");
        }
        if let Some(slot) = surface.widget().parent() {
            slot.add_css_class("wayscriber-capture-menu-slot");
        }
    }

    fn restore(&self, popover: &gtk4::Popover) {
        self.surface().set_transparent(false);
        match self {
            Self::Wrapped(surface) => {
                let wrapper = surface.widget().clone().upcast::<gtk4::Widget>();
                if popover.child().is_some_and(|child| child == wrapper) {
                    let original = surface.take_content();
                    popover.set_child(None::<&gtk4::Widget>);
                    if let Some(original) = original.as_ref() {
                        popover.set_child(Some(original));
                    }
                }
            }
            Self::Menu {
                surface,
                proof_model,
                original_model,
                original_child_size,
                original_scroll_layout,
            } => {
                let Ok(menu) = popover.clone().downcast::<gtk4::PopoverMenu>() else {
                    return;
                };
                let proof_model_object = proof_model.clone().upcast::<gtk4::gio::MenuModel>();
                if menu.menu_model().as_ref() == Some(&proof_model_object) {
                    if surface.widget().parent().is_some() {
                        if let Some(slot) = surface.widget().parent() {
                            slot.remove_css_class("wayscriber-capture-menu-slot");
                        }
                        menu.remove_child(surface.widget());
                    }
                    menu.set_menu_model(original_model.borrow().as_ref());
                }
                if let Some((child, width, height)) = original_child_size.take() {
                    child.set_size_request(width, height);
                }
                if let Some(layout) = original_scroll_layout.take() {
                    layout.restore();
                }
            }
        }
    }
}

fn capture_menu_proof_model() -> gtk4::gio::Menu {
    let model = gtk4::gio::Menu::new();
    let item = gtk4::gio::MenuItem::new(None, None);
    item.set_attribute_value("custom", Some(&CAPTURE_MENU_PROOF_ID.to_variant()));
    model.append_item(&item);
    model
}
