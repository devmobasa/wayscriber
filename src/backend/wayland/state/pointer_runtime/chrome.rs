use super::*;

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PendingChromePress {
    toast: Option<ToastPress>,
    status_hud: bool,
    zoom_chip: ZoomChipPress,
    onboarding_card: Option<OnboardingCardPress>,
}

impl PendingChromePress {
    pub(super) fn occupied(&self) -> bool {
        self.toast.is_some()
            || self.status_hud
            || self.zoom_chip.is_pending()
            || self.onboarding_card.is_some()
    }

    pub(super) fn clear(&mut self) {
        self.toast = None;
        self.status_hud = false;
        self.zoom_chip = ZoomChipPress::None;
        self.onboarding_card = None;
    }

    pub(super) fn arm_toast(&mut self, press: ToastPress) -> bool {
        if self.occupied() {
            return false;
        }
        self.toast = Some(press);
        true
    }

    pub(super) fn take_toast(&mut self) -> Option<ToastPress> {
        self.toast.take()
    }

    pub(super) fn arm_status_hud(&mut self) -> bool {
        if self.occupied() {
            return false;
        }
        self.status_hud = true;
        true
    }

    pub(super) fn take_status_hud(&mut self) -> bool {
        std::mem::take(&mut self.status_hud)
    }

    pub(super) fn arm_zoom_chip(&mut self, press: ZoomChipPress) -> bool {
        if self.occupied() || !press.is_pending() {
            return false;
        }
        self.zoom_chip = press;
        true
    }

    pub(super) fn take_zoom_chip(&mut self) -> ZoomChipPress {
        std::mem::replace(&mut self.zoom_chip, ZoomChipPress::None)
    }

    pub(super) fn arm_onboarding_card(&mut self, press: OnboardingCardPress) -> bool {
        if self.occupied() {
            return false;
        }
        self.onboarding_card = Some(press);
        true
    }

    pub(super) fn take_onboarding_card(&mut self) -> Option<OnboardingCardPress> {
        self.onboarding_card.take()
    }
}
