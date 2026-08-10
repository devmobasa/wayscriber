//! Which parts of the dialog are interactive, in what order, and what they do.
//!
//! Focus order, hit testing, and painting all read this one list, so a
//! keyboard-selected element and a clicked element can never disagree.

use super::content::{AboutAction, AboutContent, UpdateState};
use super::layout::{Plan, Rect};

/// An interactive element, identified by role rather than index into a
/// paint-time vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Element {
    UpdateCard,
    Link(usize),
    Button(usize),
    Close,
}

/// Tab order: content first, close last.
pub(super) fn focus_order(content: &AboutContent, update: &UpdateState) -> Vec<Element> {
    let mut elements = Vec::new();
    if update.action().is_some() {
        elements.push(Element::UpdateCard);
    }
    elements.extend((0..content.links.len()).map(Element::Link));
    elements.extend((0..content.buttons().len()).map(Element::Button));
    elements.push(Element::Close);
    elements
}

/// Where an element sits on screen, or `None` if the plan has no slot for it
/// (a link that was trimmed, say).
pub(super) fn rect_for(element: Element, plan: &Plan) -> Option<Rect> {
    match element {
        Element::UpdateCard => Some(plan.update_card),
        Element::Link(index) => plan.link_rows.get(index).copied(),
        Element::Button(index) => plan.buttons.get(index).copied(),
        Element::Close => Some(plan.close),
    }
}

/// What activating an element does.
pub(super) fn action_for(
    element: Element,
    content: &AboutContent,
    update: &UpdateState,
) -> Option<AboutAction> {
    match element {
        Element::UpdateCard => update.action(),
        Element::Link(index) => content.links.get(index).map(|link| link.action.clone()),
        Element::Button(index) => content
            .buttons()
            .get(index)
            .map(|button| button.action.clone()),
        Element::Close => Some(AboutAction::Close),
    }
}

/// Index into `elements` of whatever is under the pointer.
pub(super) fn element_at(elements: &[Element], plan: &Plan, position: (f64, f64)) -> Option<usize> {
    elements.iter().position(|element| {
        rect_for(*element, plan).is_some_and(|rect| rect_contains(rect, position))
    })
}

/// Index of `element` within the focus order, for painting hover/focus state.
pub(super) fn index_of(elements: &[Element], element: Element) -> Option<usize> {
    elements.iter().position(|candidate| *candidate == element)
}

/// Move focus by `delta`, wrapping at both ends. Starting from nothing, a
/// forward step lands on the first element and a backward step on the last.
pub(super) fn step_focus(current: Option<usize>, len: usize, delta: i32) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let len_i = len as i32;
    let next = match current {
        Some(index) => (index as i32 + delta).rem_euclid(len_i),
        None if delta >= 0 => 0,
        None => len_i - 1,
    };
    Some(next as usize)
}

fn rect_contains(rect: Rect, position: (f64, f64)) -> bool {
    let (x, y) = position;
    x >= rect.0 && x <= rect.0 + rect.2 && y >= rect.1 && y <= rect.1 + rect.3
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_check::{AvailableUpdate, DEFAULT_NOTES_URL, DEFAULT_UPDATE_URL};

    fn available() -> UpdateState {
        UpdateState::Available {
            update: Box::new(AvailableUpdate {
                version: "0.9.23".to_string(),
                released: None,
                update_url: DEFAULT_UPDATE_URL.to_string(),
                notes_url: DEFAULT_NOTES_URL.to_string(),
            }),
            freshness: crate::update_check::Freshness {
                checked_seconds_ago: Some(0),
                last_attempt_failed: false,
            },
        }
    }

    #[test]
    fn focus_order_runs_content_first_and_close_last() {
        let content = AboutContent::build();
        let elements = focus_order(
            &content,
            &UpdateState::Unknown(crate::update_check::Freshness::default()),
        );

        assert_eq!(elements.first(), Some(&Element::UpdateCard));
        assert_eq!(elements.last(), Some(&Element::Close));
        assert_eq!(
            elements
                .iter()
                .filter(|e| matches!(e, Element::Link(_)))
                .count(),
            content.links.len()
        );
    }

    #[test]
    fn a_running_check_drops_the_card_from_the_focus_order() {
        let content = AboutContent::build();

        let checking = focus_order(&content, &UpdateState::Checking);
        assert!(!checking.contains(&Element::UpdateCard));

        let idle = focus_order(
            &content,
            &UpdateState::Unknown(crate::update_check::Freshness::default()),
        );
        assert_eq!(idle.len(), checking.len() + 1);
    }

    #[test]
    fn hit_testing_matches_the_planned_rectangles() {
        let content = AboutContent::build();
        let plan = super::super::layout::plan(&content);
        let elements = focus_order(
            &content,
            &UpdateState::Unknown(crate::update_check::Freshness::default()),
        );

        let card = plan.update_card;
        let inside = (card.0 + card.2 / 2.0, card.1 + card.3 / 2.0);
        assert_eq!(
            element_at(&elements, &plan, inside),
            index_of(&elements, Element::UpdateCard)
        );

        let first_row = plan.link_rows[0];
        let on_row = (first_row.0 + 4.0, first_row.1 + 4.0);
        assert_eq!(
            element_at(&elements, &plan, on_row),
            index_of(&elements, Element::Link(0))
        );

        assert_eq!(element_at(&elements, &plan, (-10.0, -10.0)), None);
    }

    #[test]
    fn actions_follow_the_element_role() {
        let content = AboutContent::build();
        let update = available();

        assert_eq!(
            action_for(Element::UpdateCard, &content, &update),
            Some(AboutAction::OpenUrl(DEFAULT_UPDATE_URL.to_string()))
        );
        assert_eq!(
            action_for(Element::Link(0), &content, &update),
            Some(content.links[0].action.clone())
        );
        assert_eq!(
            action_for(Element::Close, &content, &update),
            Some(AboutAction::Close)
        );
        assert_eq!(action_for(Element::Link(99), &content, &update), None);
    }

    /// A row's action comes from the row itself, so a row that does more than
    /// open a URL keeps working through the same one element list.
    #[test]
    fn a_row_can_carry_an_action_that_is_not_a_plain_link() {
        let content = AboutContent::build();
        let index = content
            .links
            .iter()
            .position(|link| matches!(link.action, AboutAction::ReportBug { .. }))
            .expect("one row reports a problem");

        assert_eq!(
            action_for(Element::Link(index), &content, &available()),
            Some(content.links[index].action.clone())
        );
        assert!(focus_order(&content, &available()).contains(&Element::Link(index)));
    }

    #[test]
    fn focus_wraps_in_both_directions() {
        assert_eq!(step_focus(None, 3, 1), Some(0));
        assert_eq!(step_focus(None, 3, -1), Some(2));
        assert_eq!(step_focus(Some(0), 3, 1), Some(1));
        assert_eq!(step_focus(Some(2), 3, 1), Some(0));
        assert_eq!(step_focus(Some(0), 3, -1), Some(2));
        assert_eq!(step_focus(Some(1), 0, 1), None);
    }
}
