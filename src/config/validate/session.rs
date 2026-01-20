use super::super::types::SessionStorageMode;
use super::Config;

impl Config {
    pub(super) fn validate_session(&mut self) {
        const MIN_AUTOSAVE_IDLE_MS: u64 = 1_000;
        const MIN_AUTOSAVE_INTERVAL_MS: u64 = 1_000;
        const MIN_AUTOSAVE_FAILURE_BACKOFF_MS: u64 = 1_000;

        if self.session.max_shapes_per_frame == 0 {
            log::warn!("session.max_shapes_per_frame must be positive; using 1 instead");
            self.session.max_shapes_per_frame = 1;
        }

        if self.session.max_file_size_mb == 0 {
            log::warn!("session.max_file_size_mb must be positive; using 1 MB instead");
            self.session.max_file_size_mb = 1;
        } else if self.session.max_file_size_mb > 1024 {
            log::warn!(
                "session.max_file_size_mb {} too large, clamping to 1024",
                self.session.max_file_size_mb
            );
            self.session.max_file_size_mb = 1024;
        }

        if self.session.auto_compress_threshold_kb == 0 {
            log::warn!("session.auto_compress_threshold_kb must be positive; using 1 KiB");
            self.session.auto_compress_threshold_kb = 1;
        }

        if self.session.autosave_idle_ms < MIN_AUTOSAVE_IDLE_MS {
            log::warn!(
                "session.autosave_idle_ms must be at least {} ms; using {} instead",
                MIN_AUTOSAVE_IDLE_MS,
                MIN_AUTOSAVE_IDLE_MS
            );
            self.session.autosave_idle_ms = MIN_AUTOSAVE_IDLE_MS;
        }

        if self.session.autosave_interval_ms < MIN_AUTOSAVE_INTERVAL_MS {
            log::warn!(
                "session.autosave_interval_ms must be at least {} ms; using {} instead",
                MIN_AUTOSAVE_INTERVAL_MS,
                MIN_AUTOSAVE_INTERVAL_MS
            );
            self.session.autosave_interval_ms = MIN_AUTOSAVE_INTERVAL_MS;
        }

        if self.session.autosave_failure_backoff_ms < MIN_AUTOSAVE_FAILURE_BACKOFF_MS {
            log::warn!(
                "session.autosave_failure_backoff_ms must be at least {} ms; using {} instead",
                MIN_AUTOSAVE_FAILURE_BACKOFF_MS,
                MIN_AUTOSAVE_FAILURE_BACKOFF_MS
            );
            self.session.autosave_failure_backoff_ms = MIN_AUTOSAVE_FAILURE_BACKOFF_MS;
        }

        if matches!(self.session.storage, SessionStorageMode::Custom) {
            let custom = self
                .session
                .custom_directory
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty());
            if custom.is_none() {
                log::warn!(
                    "session.storage set to 'custom' but session.custom_directory missing or empty; falling back to 'auto'"
                );
                self.session.storage = SessionStorageMode::Auto;
                self.session.custom_directory = None;
            }
        }
    }
}
