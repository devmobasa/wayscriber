//! Board/canvas mode selection.

use crate::config::BoardConfig;
use crate::draw::Color;

/// Board rendering mode
///
/// Determines the background and visual style of the drawing canvas.
/// Each mode maintains its own isolated frame of shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum BoardMode {
    /// Transparent overlay showing underlying screen (default)
    #[default]
    Transparent,
    /// White/light background for drawing (whiteboard)
    Whiteboard,
    /// Dark/black background for drawing (blackboard)
    Blackboard,
}


impl BoardMode {
    /// Returns the background color for this mode, if any.
    ///
    /// Transparent mode returns None (no background fill).
    /// Whiteboard and Blackboard return their respective colors from config.
    pub fn background_color(&self, config: &BoardConfig) -> Option<Color> {
        match self {
            Self::Transparent => None,
            Self::Whiteboard => {
                let rgb = config.whiteboard_color;
                Some(Color {
                    r: rgb[0],
                    g: rgb[1],
                    b: rgb[2],
                    a: 1.0,
                })
            }
            Self::Blackboard => {
                let rgb = config.blackboard_color;
                Some(Color {
                    r: rgb[0],
                    g: rgb[1],
                    b: rgb[2],
                    a: 1.0,
                })
            }
        }
    }

    /// Returns the default pen color for this mode.
    ///
    /// Used for auto-adjusting pen color when entering board modes
    /// to ensure good contrast.
    pub fn default_pen_color(&self, config: &BoardConfig) -> Option<Color> {
        match self {
            Self::Transparent => None, // No default change for transparent
            Self::Whiteboard => {
                let rgb = config.whiteboard_pen_color;
                Some(Color {
                    r: rgb[0],
                    g: rgb[1],
                    b: rgb[2],
                    a: 1.0,
                })
            }
            Self::Blackboard => {
                let rgb = config.blackboard_pen_color;
                Some(Color {
                    r: rgb[0],
                    g: rgb[1],
                    b: rgb[2],
                    a: 1.0,
                })
            }
        }
    }
}

impl std::str::FromStr for BoardMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "transparent" => Ok(Self::Transparent),
            "whiteboard" => Ok(Self::Whiteboard),
            "blackboard" => Ok(Self::Blackboard),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_default_mode_is_transparent() {
        assert_eq!(BoardMode::default(), BoardMode::Transparent);
    }

    #[test]
    fn test_background_color() {
        let config = BoardConfig::default();

        assert_eq!(BoardMode::Transparent.background_color(&config), None);
        assert!(BoardMode::Whiteboard.background_color(&config).is_some());
        assert!(BoardMode::Blackboard.background_color(&config).is_some());

        // Verify specific colors from default config
        let white_bg = BoardMode::Whiteboard.background_color(&config).unwrap();
        assert!((white_bg.r - 0.992).abs() < 0.001);
        assert_eq!(white_bg.a, 1.0);

        let black_bg = BoardMode::Blackboard.background_color(&config).unwrap();
        assert!((black_bg.r - 0.067).abs() < 0.001);
        assert_eq!(black_bg.a, 1.0);
    }

    #[test]
    fn test_default_pen_color() {
        let config = BoardConfig::default();

        assert_eq!(BoardMode::Transparent.default_pen_color(&config), None);
        assert!(BoardMode::Whiteboard.default_pen_color(&config).is_some());
        assert!(BoardMode::Blackboard.default_pen_color(&config).is_some());

        // Whiteboard should default to black pen
        let white_pen = BoardMode::Whiteboard.default_pen_color(&config).unwrap();
        assert_eq!(white_pen.r, 0.0);
        assert_eq!(white_pen.g, 0.0);
        assert_eq!(white_pen.b, 0.0);

        // Blackboard should default to white pen
        let black_pen = BoardMode::Blackboard.default_pen_color(&config).unwrap();
        assert_eq!(black_pen.r, 1.0);
        assert_eq!(black_pen.g, 1.0);
        assert_eq!(black_pen.b, 1.0);
    }

    #[test]
    fn test_from_str() {
        assert_eq!(
            BoardMode::from_str("transparent").unwrap(),
            BoardMode::Transparent
        );
        assert_eq!(
            BoardMode::from_str("Whiteboard").unwrap(),
            BoardMode::Whiteboard
        );
        assert_eq!(
            BoardMode::from_str("BLACKBOARD").unwrap(),
            BoardMode::Blackboard
        );
        assert!(BoardMode::from_str("invalid").is_err());
    }
}
