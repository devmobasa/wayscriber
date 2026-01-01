use super::fixtures::create_placeholder_image;

#[test]
fn test_create_placeholder_image() {
    let image = create_placeholder_image();
    assert!(!image.is_empty());
    // PNG signature
    assert_eq!(&image[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
}
