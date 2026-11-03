use wayscriber::domain::Language;
use wayscriber::i18n;

use crate::test_env;

use super::{
    begin_settling, finish_settling, is_latest_rebuild, pages_settled, validate_u32_range,
};

#[test]
fn a_whole_number_in_range_is_accepted_at_both_bounds() {
    for value in ["1", "30", "60", " 60 "] {
        assert_eq!(validate_u32_range(value, 1, 60), None, "{value:?}");
    }
}

#[test]
fn an_invalid_whole_number_names_the_actual_bounds() {
    for value in ["0", "61", "-1", "2.5", "", "many"] {
        assert_eq!(
            validate_u32_range(value, 1, 60).as_deref(),
            Some("Enter a whole number between 1 and 60."),
            "{value:?}"
        );
    }
}

#[test]
fn the_bounds_survive_translation() {
    test_env::in_language_child(
        concat!(module_path!(), "::the_bounds_survive_translation"),
        || {
            i18n::set_language(Language::SimplifiedChinese);
            assert_eq!(
                validate_u32_range("256", 0, 255).as_deref(),
                Some("请输入 0 到 255 之间的整数。")
            );
        },
    );
}

#[test]
fn rebuilt_pages_settle_only_with_the_latest_ticket() {
    assert!(pages_settled(), "a thread starts with settled pages");

    let first = begin_settling();
    assert!(!pages_settled(), "a rebuild leaves the pages settling");
    let second = begin_settling();
    assert!(
        !is_latest_rebuild(first),
        "a newer rebuild replaces the ticket"
    );

    assert!(
        !finish_settling(first),
        "a stale ticket must not settle the newer pages"
    );
    assert!(!pages_settled(), "the newer rebuild is still settling");
    assert!(
        finish_settling(second),
        "the latest ticket settles its pages"
    );
    assert!(pages_settled(), "nothing is left settling");
    assert!(
        !finish_settling(second),
        "a ticket settles its pages only once"
    );
}
