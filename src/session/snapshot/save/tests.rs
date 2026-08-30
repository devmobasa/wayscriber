use super::replace_session_file;
use std::fs;

#[test]
fn failed_session_replace_leaves_the_previous_primary_intact() {
    let temp = crate::test_temp::tempdir().expect("temporary directory");
    let primary = temp.path().join("session.json");
    let missing_replacement = temp.path().join("missing.tmp");
    fs::write(&primary, b"previous session").expect("previous primary");

    replace_session_file(&missing_replacement, &primary).expect_err("replacement should fail");

    assert_eq!(
        fs::read(&primary).expect("previous primary remains readable"),
        b"previous session"
    );
}
