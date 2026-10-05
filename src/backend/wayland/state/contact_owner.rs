//! The owner a held contact was bound to when its press ran.
//!
//! Whoever owns a press owns its motion and release, whatever is under the
//! device later. Pointer buttons and the stylus tip record an owner for the
//! canvas, the pans and the inline strip. A contact with no record keeps its
//! handler's state-based path. Touch binds its sequence with `TouchTarget`.

/// Who owns a held contact after its press ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ContactOwner {
    /// The `InputState` canvas router: tools, selection, and the popups it routes.
    Canvas,
    /// A zoom-view pan started with the middle button.
    ZoomPan,
    /// A board pan started with the board-pan key held.
    BoardPan,
    /// The built-in strip painted on the canvas surface.
    InlineToolbar,
}

/// Where motion goes while a contact is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ContactMotion {
    /// Nothing recorded: the device's hover path decides.
    Hover,
    /// Inline-strip motion only. The canvas never sees it.
    Toolbar,
    ZoomPan,
    BoardPan,
    /// Canvas motion, also where the inline strip is painted.
    Canvas,
}

impl ContactOwner {
    pub(in crate::backend::wayland) fn motion(self) -> ContactMotion {
        match self {
            Self::Canvas => ContactMotion::Canvas,
            Self::ZoomPan => ContactMotion::ZoomPan,
            Self::BoardPan => ContactMotion::BoardPan,
            Self::InlineToolbar => ContactMotion::Toolbar,
        }
    }
}

impl ContactMotion {
    /// A canvas-surface gesture keeps its motion where the inline strip is
    /// painted, and the strip shows no hover for it.
    pub(in crate::backend::wayland) fn skips_inline_strip(self) -> bool {
        matches!(self, Self::Canvas | Self::ZoomPan | Self::BoardPan)
    }
}

pub(in crate::backend::wayland) fn held_motion(owner: Option<ContactOwner>) -> ContactMotion {
    owner.map_or(ContactMotion::Hover, ContactOwner::motion)
}

/// Where a release goes once the modal, chrome and radial gates have passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum ReleaseRoute {
    /// End the inline-strip interaction. The canvas never sees the release.
    InlineToolbar,
    /// Skip the toolbar gates: the pan or canvas handlers finish the gesture.
    CanvasSurface,
    /// The state-based toolbar gates decide, as before.
    Unowned,
}

/// Layer-shell and GTK sessions pass `inline_toolbars = false`, so their
/// canvas releases keep the state-based gates.
pub(in crate::backend::wayland) fn release_route(
    owner: Option<ContactOwner>,
    inline_toolbars: bool,
) -> ReleaseRoute {
    match owner {
        Some(ContactOwner::InlineToolbar) => ReleaseRoute::InlineToolbar,
        Some(owner) if inline_toolbars && owner.motion().skips_inline_strip() => {
            ReleaseRoute::CanvasSurface
        }
        _ => ReleaseRoute::Unowned,
    }
}
/// What a primary press (touch, pen tip) on the canvas surface hits on the
/// inline strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) enum InlinePress {
    Control,
    /// Between controls. The strip owns it, so it never draws under the strip.
    Strip,
}

/// `control` implies `over_strip`. The onboarding card paints above the strip
/// body and keeps presses there. A strip control still wins, as before.
pub(in crate::backend::wayland) fn inline_primary_press(
    control: bool,
    over_strip: bool,
    onboarding_card: bool,
) -> Option<InlinePress> {
    if control {
        return Some(InlinePress::Control);
    }

    (over_strip && !onboarding_card).then_some(InlinePress::Strip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_release_follows_the_press_owner() {
        for (owner, expected) in [
            (ContactOwner::Canvas, ReleaseRoute::CanvasSurface),
            (ContactOwner::ZoomPan, ReleaseRoute::CanvasSurface),
            (ContactOwner::BoardPan, ReleaseRoute::CanvasSurface),
            (ContactOwner::InlineToolbar, ReleaseRoute::InlineToolbar),
        ] {
            assert_eq!(release_route(Some(owner), true), expected);
        }
        assert_eq!(release_route(None, true), ReleaseRoute::Unowned);
        assert_eq!(
            release_route(Some(ContactOwner::Canvas), false),
            ReleaseRoute::Unowned
        );
        assert_eq!(
            release_route(Some(ContactOwner::InlineToolbar), false),
            ReleaseRoute::InlineToolbar
        );
    }

    #[test]
    fn primary_press_claims_strip_gaps_and_respects_the_onboarding_card() {
        for (control, strip, card, expected) in [
            (true, true, false, Some(InlinePress::Control)),
            (true, true, true, Some(InlinePress::Control)),
            (false, true, false, Some(InlinePress::Strip)),
            (false, true, true, None),
            (false, false, false, None),
        ] {
            assert_eq!(inline_primary_press(control, strip, card), expected);
        }
    }
}
