use super::super::draft::ConfigDraft;
use wayscriber::config::Config;

impl ConfigDraft {
    pub(super) fn apply_presenter_mode(&self, config: &mut Config) {
        config.presenter_mode.hide_status_bar = self.presenter_hide_status_bar;
        config.presenter_mode.hide_toolbars = self.presenter_hide_toolbars;
        config.presenter_mode.hide_tool_preview = self.presenter_hide_tool_preview;
        config.presenter_mode.close_help_overlay = self.presenter_close_help_overlay;
        config.presenter_mode.enable_click_highlight = self.presenter_enable_click_highlight;
        config.presenter_mode.tool_behavior = self.presenter_tool_behavior.to_behavior();
        config.presenter_mode.show_toast = self.presenter_show_toast;
    }
}
