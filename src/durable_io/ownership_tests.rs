use std::io::Write;

use super::*;

#[test]
fn independent_writes_need_no_shared_temporary_sequence() {
    let temp = crate::test_temp::tempdir().expect("test owns its temporary directory fixture");
    let file_name = std::ffi::OsStr::new("state.toml");
    let candidates = || (0..64).map(|attempt| temp_path(temp.path(), file_name, 7, attempt));

    let (first_path, mut first_file) = create_temp_file_from_candidates(candidates())
        .expect("first test writer has an available temporary candidate");
    let (second_path, mut second_file) = create_temp_file_from_candidates(candidates())
        .expect("second test writer skips the first writer's owned candidate");
    first_file
        .write_all(b"first")
        .expect("first fixture owns its temporary file");
    second_file
        .write_all(b"second")
        .expect("second fixture owns its temporary file");

    assert_ne!(first_path, second_path);
    assert!(first_path.exists());
    assert!(second_path.exists());
}
