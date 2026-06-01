mod config;
mod identifiers;
mod types;
mod validation;

#[cfg(test)]
mod tests;

pub use config::{options_from_config, options_from_config_for_named_file};
pub(crate) use types::append_path_suffix;
pub use types::{
    CompressionMode, DEFAULT_AUTO_COMPRESS_THRESHOLD_BYTES, SessionOptions, SessionTarget,
};
pub use validation::{
    normalize_named_session_file_arg, validate_named_session_file_for_clear,
    validate_named_session_file_for_foreground, validate_named_session_file_for_info,
};
