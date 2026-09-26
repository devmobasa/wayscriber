//! Pointer ownership of the first-run onboarding card.
//!
//! The card is painted on the overlay surface, above the canvas. Without an
//! owner every press on it fell through to the tool underneath: a click on
//! the card body drew a dot and ticked "Draw a stroke" off the checklist.
//! Presses on the card body are swallowed; presses on its buttons run the
//! button when the release lands on the same button.

use crate::backend::wayland::state::WaylandState;
use crate::ui::{OnboardingCardAction, OnboardingCardLayout, OnboardingCardPress};

/// The rectangles painted this frame, the hovered button, and the stylus
/// press the card owns.
///
/// Pointer and touch presses use the shared chrome-press slot on
/// `PointerRuntime`, like toasts; the stylus has no such slot, so its press
/// is held here until the tip lifts.
#[derive(Debug, Default)]
pub(in crate::backend::wayland) struct OnboardingCardChrome {
    layout: Option<OnboardingCardLayout>,
    hovered: Option<OnboardingCardAction>,
    #[cfg(feature = "tablet-input")]
    stylus_press: Option<OnboardingCardPress>,
}

impl OnboardingCardChrome {
    /// Records what the renderer painted; `None` when the card was not drawn.
    pub(in crate::backend::wayland) fn set_layout(&mut self, layout: Option<OnboardingCardLayout>) {
        if layout.is_none() {
            self.hovered = None;
        }
        self.layout = layout;
    }

    pub(in crate::backend::wayland) fn hovered(&self) -> Option<OnboardingCardAction> {
        self.hovered
    }

    /// The press a screen point would start, while the card is shown.
    fn press_at(&self, card_visible: bool, x: f64, y: f64) -> Option<OnboardingCardPress> {
        if !card_visible {
            return None;
        }
        self.layout.as_ref()?.press_at(x, y)
    }

    /// Updates the hovered button; returns the card rectangle to repaint when
    /// the highlight changed.
    fn set_hovered(
        &mut self,
        hovered: Option<OnboardingCardAction>,
    ) -> Option<&OnboardingCardLayout> {
        if self.hovered == hovered {
            return None;
        }
        self.hovered = hovered;
        self.layout.as_ref()
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

/// A button runs only when press and release land on the same button.
fn released_action(
    pressed: OnboardingCardPress,
    released: Option<OnboardingCardPress>,
) -> Option<OnboardingCardAction> {
    let action = pressed.action()?;
    (released == Some(pressed)).then_some(action)
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

    /// Finish a press the card owned: run the button under both the press and
    /// the release, or do nothing for the card body.
    pub(in crate::backend::wayland) fn release_onboarding_card_press(
        &mut self,
        press: OnboardingCardPress,
        x: f64,
        y: f64,
    ) {
        if let Some(action) = released_action(press, self.onboarding_card_press_at(x, y)) {
            self.run_onboarding_card_action(action);
        }
    }

    /// Tracks the button under an idle pointer so it can highlight; `None`
    /// (a gesture in progress) highlights nothing.
    pub(in crate::backend::wayland) fn update_onboarding_card_hover(
        &mut self,
        pointer: Option<(f64, f64)>,
    ) {
        let hovered = pointer
            .and_then(|(x, y)| self.onboarding_card_press_at(x, y))
            .and_then(OnboardingCardPress::action);
        if self.onboarding_card.set_hovered(hovered).is_none() {
            return;
        }

        // Hover only recolors a button, and the card repaints as a whole.
        self.input_state
            .dirty_tracker
            .mark_full_for(crate::draw::DirtyFullReason::FirstRunOnboarding);
        self.input_state.needs_redraw = true;
    }

    /// Runs a card button or its keyboard equivalent.
    pub(in crate::backend::wayland) fn run_onboarding_card_action(
        &mut self,
        action: OnboardingCardAction,
    ) {
        match action {
            OnboardingCardAction::Continue => self.acknowledge_first_run_toolbar_exit(),
            OnboardingCardAction::SetUpBackgroundMode => {
                self.answer_first_run_background_mode(true);
            }
            OnboardingCardAction::SkipBackgroundMode => {
                self.answer_first_run_background_mode(false);
            }
            OnboardingCardAction::SkipTour => {
                self.try_skip_first_run_onboarding();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::OnboardingCardButtonHit;

    const SKIP: OnboardingCardAction = OnboardingCardAction::SkipTour;

    fn chrome_with_card() -> OnboardingCardChrome {
        let mut chrome = OnboardingCardChrome::default();
        chrome.set_layout(Some(OnboardingCardLayout {
            x: 100.0,
            y: 50.0,
            width: 200.0,
            height: 120.0,
            buttons: vec![OnboardingCardButtonHit {
                x: 120.0,
                y: 130.0,
                width: 80.0,
                height: 30.0,
                action: SKIP,
            }],
        }));
        chrome
    }

    #[test]
    fn a_press_on_the_visible_card_belongs_to_the_card() {
        let chrome = chrome_with_card();

        assert_eq!(
            chrome.press_at(true, 150.0, 60.0),
            Some(OnboardingCardPress::Body)
        );
        assert_eq!(
            chrome.press_at(true, 100.0, 50.0),
            Some(OnboardingCardPress::Body)
        );
        assert_eq!(
            chrome.press_at(true, 150.0, 140.0),
            Some(OnboardingCardPress::Button(SKIP))
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

    #[test]
    fn a_button_runs_only_when_released_on_itself() {
        let pressed = OnboardingCardPress::Button(SKIP);

        assert_eq!(released_action(pressed, Some(pressed)), Some(SKIP));
        assert_eq!(
            released_action(pressed, Some(OnboardingCardPress::Body)),
            None,
            "dragging off onto the card body cancels"
        );
        assert_eq!(released_action(pressed, None), None);
        assert_eq!(
            released_action(OnboardingCardPress::Body, Some(OnboardingCardPress::Body)),
            None,
            "the body has no action"
        );
    }

    #[test]
    fn hover_reports_a_repaint_only_when_it_changes() {
        let mut chrome = chrome_with_card();

        assert!(chrome.set_hovered(Some(SKIP)).is_some());
        assert!(chrome.set_hovered(Some(SKIP)).is_none());
        assert_eq!(chrome.hovered(), Some(SKIP));

        chrome.set_layout(None);
        assert_eq!(chrome.hovered(), None, "an unpainted card hovers nothing");
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
