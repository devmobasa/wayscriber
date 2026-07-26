mod delete_restore;
mod lifecycle;
mod pages;
mod steps;
mod switch;

pub use steps::{STEP_CAPTURE_BOARD_ID, StepCaptureFrame, StepPageReceipt};

const BOARD_RECENT_LIMIT: usize = 5;
