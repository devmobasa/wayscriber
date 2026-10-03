use super::{Config, float::finite_or_default};

impl Config {
    pub(super) fn validate_board(&mut self) {
        // Validate board mode default
        if !matches!(
            self.board.default_mode.to_lowercase().as_str(),
            "transparent" | "whiteboard" | "blackboard"
        ) {
            log::warn!(
                "Invalid board default_mode '{}', falling back to 'transparent'",
                self.board.default_mode
            );
            self.board.default_mode = "transparent".to_string();
        }

        let defaults = crate::config::BoardConfig::default();
        for (field, color, fallback) in [
            (
                "board.whiteboard_color",
                &mut self.board.whiteboard_color,
                defaults.whiteboard_color,
            ),
            (
                "board.blackboard_color",
                &mut self.board.blackboard_color,
                defaults.blackboard_color,
            ),
            (
                "board.whiteboard_pen_color",
                &mut self.board.whiteboard_pen_color,
                defaults.whiteboard_pen_color,
            ),
            (
                "board.blackboard_pen_color",
                &mut self.board.blackboard_pen_color,
                defaults.blackboard_pen_color,
            ),
        ] {
            for (index, (component, fallback)) in color.iter_mut().zip(fallback).enumerate() {
                *component = finite_or_default(*component, fallback, &format!("{field}[{index}]"));
            }
        }

        // Validate board color RGB values (0.0-1.0)
        for i in 0..3 {
            if !(0.0..=1.0).contains(&self.board.whiteboard_color[i]) {
                log::warn!(
                    "Invalid whiteboard_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.whiteboard_color[i]
                );
                self.board.whiteboard_color[i] = self.board.whiteboard_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.blackboard_color[i]) {
                log::warn!(
                    "Invalid blackboard_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.blackboard_color[i]
                );
                self.board.blackboard_color[i] = self.board.blackboard_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.whiteboard_pen_color[i]) {
                log::warn!(
                    "Invalid whiteboard_pen_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.whiteboard_pen_color[i]
                );
                self.board.whiteboard_pen_color[i] =
                    self.board.whiteboard_pen_color[i].clamp(0.0, 1.0);
            }
            if !(0.0..=1.0).contains(&self.board.blackboard_pen_color[i]) {
                log::warn!(
                    "Invalid blackboard_pen_color[{}] = {:.3}, clamping to 0.0-1.0",
                    i,
                    self.board.blackboard_pen_color[i]
                );
                self.board.blackboard_pen_color[i] =
                    self.board.blackboard_pen_color[i].clamp(0.0, 1.0);
            }
        }
    }
}
