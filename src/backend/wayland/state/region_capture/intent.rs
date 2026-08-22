use crate::{
    backend::ExitAfterCaptureMode,
    capture::{CaptureDestination, file::FileSaveConfig},
    config::Action,
    input::state::RegionPurposeTag,
};

/// Runtime picker settings captured when a region action is accepted.
///
/// Keeping these values outside the live config prevents a reload from
/// changing a picker that is already open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::backend::wayland) struct RegionPickerOptions {
    show_size_readout: bool,
    show_loupe: bool,
    show_legend: bool,
}

impl RegionPickerOptions {
    pub(in crate::backend::wayland) const fn new(
        show_size_readout: bool,
        show_loupe: bool,
        show_legend: bool,
    ) -> Self {
        Self {
            show_size_readout,
            show_loupe,
            show_legend,
        }
    }

    pub(in crate::backend::wayland) const fn show_size_readout(self) -> bool {
        self.show_size_readout
    }

    #[allow(dead_code)] // Persisted in Phase 1; the loupe renderer arrives in Phase 2.
    pub(in crate::backend::wayland) const fn show_loupe(self) -> bool {
        self.show_loupe
    }

    pub(in crate::backend::wayland) const fn show_legend(self) -> bool {
        self.show_legend
    }
}

/// Immutable snapshot of a region capture action.
///
/// The action entry point resolves config-derived values once and stores them
/// here. Acquisition, fallback, and submission must use this snapshot instead
/// of consulting mutable runtime config again.
#[derive(Clone, Debug)]
pub(in crate::backend::wayland) struct RegionCaptureIntent {
    action: Action,
    purpose: RegionPurposeTag,
    destination: CaptureDestination,
    save_config: Option<FileSaveConfig>,
    exit_mode: ExitAfterCaptureMode,
    options: RegionPickerOptions,
}

impl RegionCaptureIntent {
    pub(in crate::backend::wayland) fn new(
        action: Action,
        purpose: RegionPurposeTag,
        destination: CaptureDestination,
        save_config: Option<FileSaveConfig>,
        exit_mode: ExitAfterCaptureMode,
        options: RegionPickerOptions,
    ) -> Self {
        Self {
            action,
            purpose,
            destination,
            save_config,
            exit_mode,
            options,
        }
    }

    pub(in crate::backend::wayland) const fn action(&self) -> Action {
        self.action
    }

    pub(in crate::backend::wayland) const fn purpose(&self) -> RegionPurposeTag {
        self.purpose
    }

    pub(in crate::backend::wayland) const fn destination(&self) -> CaptureDestination {
        self.destination
    }

    pub(in crate::backend::wayland) const fn save_config(&self) -> Option<&FileSaveConfig> {
        self.save_config.as_ref()
    }

    pub(in crate::backend::wayland) const fn exit_mode(&self) -> ExitAfterCaptureMode {
        self.exit_mode
    }

    pub(in crate::backend::wayland) const fn options(&self) -> RegionPickerOptions {
        self.options
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn intent_is_a_complete_immutable_action_snapshot() {
        let save_config = FileSaveConfig {
            save_directory: PathBuf::from("/tmp/region-captures"),
            filename_template: "region-%Y%m%d".to_string(),
            format: "png".to_string(),
        };
        let options = RegionPickerOptions::new(true, false, true);
        let intent = RegionCaptureIntent::new(
            Action::CaptureClipboardRegion,
            RegionPurposeTag::CaptureDeliver,
            CaptureDestination::ClipboardOnly,
            Some(save_config),
            ExitAfterCaptureMode::Auto,
            options,
        );

        assert_eq!(intent.action(), Action::CaptureClipboardRegion);
        assert_eq!(intent.purpose(), RegionPurposeTag::CaptureDeliver);
        assert_eq!(intent.destination(), CaptureDestination::ClipboardOnly);
        assert_eq!(
            intent.save_config().unwrap().save_directory,
            PathBuf::from("/tmp/region-captures")
        );
        assert_eq!(
            intent.save_config().unwrap().filename_template,
            "region-%Y%m%d"
        );
        assert_eq!(intent.save_config().unwrap().format, "png");
        assert_eq!(intent.exit_mode(), ExitAfterCaptureMode::Auto);
        assert_eq!(intent.options(), options);
        assert!(intent.options().show_size_readout());
        assert!(!intent.options().show_loupe());
        assert!(intent.options().show_legend());
    }

    #[test]
    fn interactive_purpose_and_all_option_values_are_representable() {
        let options = RegionPickerOptions::new(false, true, false);
        let intent = RegionCaptureIntent::new(
            Action::CaptureSelection,
            RegionPurposeTag::CaptureInteractive,
            CaptureDestination::ClipboardAndFile,
            None,
            ExitAfterCaptureMode::Never,
            options,
        );

        assert_eq!(intent.purpose(), RegionPurposeTag::CaptureInteractive);
        assert!(intent.save_config().is_none());
        assert!(!intent.options().show_size_readout());
        assert!(intent.options().show_loupe());
        assert!(!intent.options().show_legend());
    }
}
