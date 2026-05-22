use super::*;
use std::path::PathBuf;

#[test]
fn compressed_estimate_blocks_when_raw_payload_exceeds_expanded_safety_limit() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "expanded-preflight");
    options.compression = CompressionMode::On;
    options.max_file_size_bytes = u64::MAX;

    let estimate = CloneStorageEstimate {
        raw_bytes: 513,
        image_encoded_raw_bytes: 512,
        image_original_bytes: 384,
        image_count: 1,
        non_image_shape_count: 0,
    };

    assert!(exceeds_expanded_safety_limit(&estimate, &options, 512));
}

#[test]
fn compressed_estimate_counts_non_image_payload_at_raw_size() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "compressed-preflight");
    options.compression = CompressionMode::On;

    let estimate = CloneStorageEstimate {
        raw_bytes: 1100,
        image_encoded_raw_bytes: 400,
        image_original_bytes: 300,
        image_count: 1,
        non_image_shape_count: 1,
    };

    assert_eq!(estimate.estimated_written_bytes(&options), 1143);
}

#[test]
fn uncompressed_estimate_does_not_apply_expanded_safety_limit() {
    let mut options = SessionOptions::new(PathBuf::from("/tmp"), "expanded-preflight");
    options.compression = CompressionMode::Off;
    options.max_file_size_bytes = u64::MAX;

    let estimate = CloneStorageEstimate {
        raw_bytes: 513,
        image_encoded_raw_bytes: 512,
        image_original_bytes: 384,
        image_count: 1,
        non_image_shape_count: 0,
    };

    assert!(!exceeds_expanded_safety_limit(&estimate, &options, 512));
}
