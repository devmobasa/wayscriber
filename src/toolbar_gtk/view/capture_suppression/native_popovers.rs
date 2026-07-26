//! Capture suppression for GTK-owned `GtkPopover` surfaces.

use gtk4::glib::variant::ToVariant;
use gtk4::prelude::*;

use super::set_native_widget_input_enabled;
use crate::toolbar_gtk::css::CAPTURE_TRANSPARENT_CLASS;
use crate::toolbar_gtk::view::{CaptureProofTarget, CaptureProofWithdrawal, CaptureSurfaceContent};

const CAPTURE_MENU_PROOF_ID: &str = "wayscriber-capture-proof";
pub(super) const CAPTURE_POPOVER_SELECTED_CLASS: &str = "wayscriber-capture-popover-selected";
const CAPTURE_POPOVER_PROOF_PENDING_CLASS: &str = "wayscriber-capture-popover-proof-pending";

#[derive(Default)]
pub(super) struct NativePopoverCapture {
    captures: Vec<NativePopoverSurface>,
}

impl NativePopoverCapture {
    pub(super) fn enroll(
        &mut self,
        popover: &gtk4::Popover,
        selected_for_capture: bool,
        input_enabled: bool,
    ) {
        self.captures.retain(NativePopoverSurface::is_attached);
        if let Some(capture) = self
            .captures
            .iter_mut()
            .find(|capture| capture.matches(popover))
        {
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

        let mut capture = NativePopoverSurface::new(popover);
        capture.set_selected(true);
        capture.set_capture_state(true, input_enabled);
        self.captures.push(capture);
    }

    pub(super) fn set_suppressed(&mut self, suppressed: bool) {
        if !suppressed {
            for capture in self.captures.drain(..) {
                capture.restore();
            }
            return;
        }

        self.captures.retain(NativePopoverSurface::is_attached);
        for capture in &mut self.captures {
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
        for capture in &self.captures {
            if let Some(popover) = capture.popover() {
                set_native_widget_input_enabled(popover.upcast_ref(), enabled);
            }
        }
    }

    pub(super) fn capture_targets(&mut self) -> Vec<CaptureProofTarget> {
        self.captures
            .iter_mut()
            .filter_map(NativePopoverSurface::capture_target)
            .collect()
    }

    pub(super) fn pending_capture_targets(&mut self) -> Vec<CaptureProofTarget> {
        self.captures
            .iter_mut()
            .filter_map(NativePopoverSurface::pending_capture_target)
            .collect()
    }

    pub(super) fn mark_proven(&mut self) {
        for capture in &mut self.captures {
            capture.mark_in_flight_proven();
        }
    }
}

struct NativePopoverSurface {
    popover: gtk4::glib::WeakRef<gtk4::Popover>,
    content: NativePopoverContent,
    in_flight_paintable: Option<gtk4::gdk::Paintable>,
    map_handler: gtk4::glib::SignalHandlerId,
    unmap_handler: gtk4::glib::SignalHandlerId,
}

impl NativePopoverSurface {
    fn new(popover: &gtk4::Popover) -> Self {
        let content = NativePopoverContent::new(popover);
        let map_content = content.surface().clone();
        let map_handler = popover.connect_map(move |popover| {
            popover.add_css_class(CAPTURE_POPOVER_SELECTED_CLASS);
            rearm_proof(popover, &map_content);
        });

        let unmap_handler = popover.connect_unmap(move |popover| {
            popover.remove_css_class(CAPTURE_POPOVER_SELECTED_CLASS);
        });

        Self {
            popover: popover.downgrade(),
            content,
            in_flight_paintable: None,
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

    fn set_selected(&mut self, selected: bool) {
        let Some(popover) = self.popover() else {
            return;
        };
        let was_selected = popover.has_css_class(CAPTURE_POPOVER_SELECTED_CLASS);
        if selected {
            popover.add_css_class(CAPTURE_POPOVER_SELECTED_CLASS);
            if !was_selected {
                rearm_proof(&popover, self.content.surface());
            }
        } else {
            popover.remove_css_class(CAPTURE_POPOVER_SELECTED_CLASS);
            self.in_flight_paintable = None;
        }
    }

    fn set_capture_state(&mut self, suppressed: bool, input_enabled: bool) {
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

    fn restore(mut self) {
        let Some(popover) = self.popover() else {
            return;
        };
        popover.disconnect(self.map_handler);
        popover.disconnect(self.unmap_handler);
        popover.remove_css_class(CAPTURE_TRANSPARENT_CLASS);
        popover.remove_css_class(CAPTURE_POPOVER_SELECTED_CLASS);
        popover.remove_css_class(CAPTURE_POPOVER_PROOF_PENDING_CLASS);
        self.content.restore(&popover);
        set_native_widget_input_enabled(popover.upcast_ref(), true);
    }

    fn capture_target(&mut self) -> Option<CaptureProofTarget> {
        let popover = self.popover()?;
        if !popover.has_css_class(CAPTURE_POPOVER_SELECTED_CLASS) {
            return None;
        }
        self.in_flight_paintable = self.content.surface().proof.paintable();
        Some(CaptureProofTarget::new_withdrawable_with_cleanup(
            "gtk-owned-popover",
            &popover,
            self.content.surface(),
            CaptureProofWithdrawal::RemoveWidgetClass(CAPTURE_POPOVER_SELECTED_CLASS),
        ))
    }

    fn pending_capture_target(&mut self) -> Option<CaptureProofTarget> {
        let popover = self.popover()?;
        if !popover.has_css_class(CAPTURE_POPOVER_SELECTED_CLASS)
            || !popover.has_css_class(CAPTURE_POPOVER_PROOF_PENDING_CLASS)
        {
            return None;
        }
        self.capture_target()
    }

    fn mark_in_flight_proven(&mut self) {
        let Some(in_flight) = self.in_flight_paintable.take() else {
            return;
        };
        let Some(popover) = self.popover() else {
            return;
        };
        if popover.has_css_class(CAPTURE_POPOVER_SELECTED_CLASS)
            && self.content.surface().proof.paintable().as_ref() == Some(&in_flight)
        {
            popover.remove_css_class(CAPTURE_POPOVER_PROOF_PENDING_CLASS);
        }
    }
}

fn rearm_proof(popover: &gtk4::Popover, content: &CaptureSurfaceContent) {
    content.refresh_transparent_proof();
    popover.add_css_class(CAPTURE_POPOVER_PROOF_PENDING_CLASS);
}

enum NativePopoverContent {
    Wrapped(CaptureSurfaceContent),
    Menu {
        surface: CaptureSurfaceContent,
        proof_model: gtk4::gio::Menu,
        original_model: Option<gtk4::gio::MenuModel>,
    },
}

impl NativePopoverContent {
    fn new(popover: &gtk4::Popover) -> Self {
        if let Ok(menu) = popover.clone().downcast::<gtk4::PopoverMenu>() {
            return Self::Menu {
                surface: CaptureSurfaceContent::empty(),
                proof_model: capture_menu_proof_model(),
                original_model: menu.menu_model(),
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

    fn set_transparent(&mut self, popover: &gtk4::Popover, transparent: bool) {
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
            *original_model = menu.menu_model();
            menu.set_menu_model(Some(proof_model));
        }
        if surface.widget().parent().is_none()
            && !menu.add_child(surface.widget(), CAPTURE_MENU_PROOF_ID)
        {
            log::error!("could not install GTK menu capture proof widget");
        }
    }

    fn restore(&mut self, popover: &gtk4::Popover) {
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
            } => {
                let Ok(menu) = popover.clone().downcast::<gtk4::PopoverMenu>() else {
                    return;
                };
                let proof_model_object = proof_model.clone().upcast::<gtk4::gio::MenuModel>();
                if menu.menu_model().as_ref() == Some(&proof_model_object) {
                    if surface.widget().parent().is_some() {
                        menu.remove_child(surface.widget());
                    }
                    menu.set_menu_model(original_model.as_ref());
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
