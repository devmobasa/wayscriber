use super::review_action_suppresses_next_release;
use crate::ui::RegionAction;

#[test]
fn retained_review_toggle_does_not_arm_the_post_modal_release_latch() {
    assert!(!review_action_suppresses_next_release(
        RegionAction::ToggleIncludeDrawings
    ));
    for terminal in [
        RegionAction::Copy,
        RegionAction::Save,
        RegionAction::Both,
        RegionAction::Board,
    ] {
        assert!(review_action_suppresses_next_release(terminal));
    }
}
