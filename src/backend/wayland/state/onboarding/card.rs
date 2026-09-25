//! Pointer ownership of the first-run onboarding card.
//!
//! The card is painted on the overlay surface, above the canvas. Without an
//! owner every press on it fell through to the tool underneath: a click on
//! the card body drew a dot and ticked "Draw a stroke" off the checklist.

use crate::backend::wayland::state::WaylandState;
use crate::ui::{OnboardingCardLayout, OnboardingCardPress};

/// The rectangle painted this frame, plus the stylus press the card owns.
///
/// Pointer and touch presses use the shared chrome-press slot on
/// `PointerRuntime`, like toasts; the stylus has no such slot, so its press
/// is held here until the tip lifts.
#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct OnboardingCardChrome {
    layout: Option<OnboardingCardLayout>,
    #[cfg(feature = "tablet-input")]
    stylus_press: Option<OnboardingCardPress>,
}

impl OnboardingCardChrome {
    /// Records what the renderer painted; `None` when the card was not drawn.
    pub(in crate::backend::wayland) fn set_layout(&mut self, layout: Option<OnboardingCardLayout>) {
        self.layout = layout;
    }

    /// The press a screen point would start, while the card is shown.
    fn press_at(&self, card_visible: bool, x: f64, y: f64) -> Option<OnboardingCardPress> {
        if !card_visible {
            return None;
        }
        self.layout?.press_at(x, y)
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn set_stylus_press(&mut self, press: OnboardingCardPress) {
        self.stylus_press = Some(press);
    }

    #[cfg(feature = "tablet-input")]
    pub(in crate::backend::wayland) fn take_stylus_press(&mut self) -> Option<OnboardingCardPress> {
        self.stylus_press.take()
    }
}

impl WaylandState {
    /// What a press at a screen point targets on the onboarding card, if the
    /// card is on screen and the point lands on it.
    pub(in crate::backend::wayland) fn onboarding_card_press_at(
        &self,
        x: f64,
        y: f64,
    ) -> Option<OnboardingCardPress> {
        self.onboarding_card
            .press_at(self.first_run_onboarding_card_visible(), x, y)
    }

    /// Finish a press the card owned. The press only ever lands on the card
    /// body, which does nothing beyond keeping the click off the canvas.
    pub(in crate::backend::wayland) fn release_onboarding_card_press(
        &mut self,
        press: OnboardingCardPress,
        _x: f64,
        _y: f64,
    ) {
        match press {
            OnboardingCardPress::Body => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chrome_with_card() -> OnboardingCardChrome {
        let mut chrome = OnboardingCardChrome::default();
        chrome.set_layout(Some(OnboardingCardLayout {
            x: 100.0,
            y: 50.0,
            width: 200.0,
            height: 120.0,
        }));
        chrome
    }

    #[test]
    fn a_press_on_the_visible_card_belongs_to_the_card() {
        let chrome = chrome_with_card();

        assert_eq!(
            chrome.press_at(true, 150.0, 100.0),
            Some(OnboardingCardPress::Body)
        );
        assert_eq!(
            chrome.press_at(true, 100.0, 50.0),
            Some(OnboardingCardPress::Body)
        );
        assert_eq!(
            chrome.press_at(true, 300.0, 170.0),
            Some(OnboardingCardPress::Body)
        );
    }

    #[test]
    fn presses_beside_the_card_still_reach_the_canvas() {
        let chrome = chrome_with_card();

        assert_eq!(chrome.press_at(true, 99.0, 100.0), None);
        assert_eq!(chrome.press_at(true, 150.0, 171.0), None);
        assert_eq!(chrome.press_at(true, 20.0, 400.0), None);
    }

    #[test]
    fn a_hidden_or_unpainted_card_owns_nothing() {
        let chrome = chrome_with_card();
        assert_eq!(chrome.press_at(false, 150.0, 100.0), None);

        let mut chrome = chrome;
        chrome.set_layout(None);
        assert_eq!(chrome.press_at(true, 150.0, 100.0), None);
    }

    #[cfg(feature = "tablet-input")]
    #[test]
    fn the_stylus_press_is_taken_once() {
        let mut chrome = chrome_with_card();
        chrome.set_stylus_press(OnboardingCardPress::Body);

        assert_eq!(chrome.take_stylus_press(), Some(OnboardingCardPress::Body));
        assert_eq!(chrome.take_stylus_press(), None);
    }
}
