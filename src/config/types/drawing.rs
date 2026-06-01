use crate::config::enums::ColorSpec;
use crate::draw::shape::REGULAR_POLYGON_DEFAULT_SIDES;
use crate::input::{DragBindableTool, DragTool, EraserMode};
use serde::{Deserialize, Serialize};

/// Drawing-related settings.
///
/// Controls the default appearance of drawing tools when the overlay first opens.
/// Users can change these values at runtime using keybindings.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawingConfig {
    /// Default pen color - either a named color (red, green, blue, yellow, orange, pink, white, black)
    /// or an RGB array like `[255, 0, 0]` for red
    #[serde(default = "default_color")]
    pub default_color: ColorSpec,

    /// Default pen thickness in pixels (valid range: 1.0 - 50.0)
    #[serde(default = "default_thickness")]
    pub default_thickness: f64,

    /// Default eraser size in pixels (valid range: 1.0 - 50.0)
    #[serde(default = "default_eraser_size")]
    pub default_eraser_size: f64,

    /// Default eraser behavior (brush or stroke)
    #[serde(default = "default_eraser_mode")]
    pub default_eraser_mode: EraserMode,

    /// Default marker opacity multiplier (0.05 - 0.9), applied to the current color alpha
    #[serde(default = "default_marker_opacity")]
    pub marker_opacity: f64,

    /// Whether shapes start filled when applicable
    #[serde(default = "default_fill_enabled")]
    pub default_fill_enabled: bool,

    /// Default side count for the regular polygon tool (valid range: 3 - 12)
    #[serde(default = "default_polygon_sides")]
    pub polygon_sides: u8,

    /// Default font size for text mode in points (valid range: 8.0 - 72.0)
    #[serde(default = "default_font_size")]
    pub default_font_size: f64,

    /// Hit-test tolerance in pixels for selection (valid range: 1.0 - 20.0)
    #[serde(default = "default_hit_test_tolerance")]
    pub hit_test_tolerance: f64,

    /// Number of shapes processed linearly before enabling spatial index
    #[serde(default = "default_hit_test_threshold")]
    pub hit_test_linear_threshold: usize,

    /// Maximum undo actions retained (valid range: 10 - 1000)
    #[serde(default = "default_undo_stack_limit")]
    pub undo_stack_limit: usize,

    /// Tool used for drag with no modifier.
    #[serde(default = "default_drag_tool")]
    pub drag_tool: DragBindableTool,

    /// Tool used for Shift+drag.
    #[serde(default = "default_shift_drag_tool")]
    pub shift_drag_tool: DragBindableTool,

    /// Tool used for Ctrl+drag.
    #[serde(default = "default_ctrl_drag_tool")]
    pub ctrl_drag_tool: DragBindableTool,

    /// Tool used for Ctrl+Shift+drag.
    #[serde(default = "default_ctrl_shift_drag_tool")]
    pub ctrl_shift_drag_tool: DragBindableTool,

    /// Tool used for Tab+drag.
    #[serde(default = "default_tab_drag_tool")]
    pub tab_drag_tool: DragBindableTool,

    /// Optional per-mouse-button drag tool mapping.
    ///
    /// When omitted, the legacy drag-tool fields above apply to left-button
    /// drags and right/middle buttons keep their built-in behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_tools: Option<MouseDragToolsConfig>,

    /// Font family name for text rendering (e.g., "Sans", "Monospace", "JetBrains Mono")
    /// Falls back to "Sans" if the specified font is not available
    /// Note: Install fonts system-wide and reference by family name
    #[serde(default = "default_font_family")]
    pub font_family: String,

    /// Font weight (e.g., "normal", "bold", "light", 400, 700)
    /// Can be a named weight or a numeric value (100-900)
    #[serde(default = "default_font_weight")]
    pub font_weight: String,

    /// Font style (e.g., "normal", "italic", "oblique")
    #[serde(default = "default_font_style")]
    pub font_style: String,

    /// Enable semi-transparent background box behind text for better contrast
    #[serde(default = "default_text_background")]
    pub text_background_enabled: bool,
}

impl Default for DrawingConfig {
    fn default() -> Self {
        Self {
            default_color: default_color(),
            default_thickness: default_thickness(),
            default_eraser_size: default_eraser_size(),
            default_eraser_mode: default_eraser_mode(),
            marker_opacity: default_marker_opacity(),
            default_fill_enabled: default_fill_enabled(),
            polygon_sides: default_polygon_sides(),
            default_font_size: default_font_size(),
            hit_test_tolerance: default_hit_test_tolerance(),
            hit_test_linear_threshold: default_hit_test_threshold(),
            undo_stack_limit: default_undo_stack_limit(),
            drag_tool: default_drag_tool(),
            shift_drag_tool: default_shift_drag_tool(),
            ctrl_drag_tool: default_ctrl_drag_tool(),
            ctrl_shift_drag_tool: default_ctrl_shift_drag_tool(),
            tab_drag_tool: default_tab_drag_tool(),
            drag_tools: None,
            font_family: default_font_family(),
            font_weight: default_font_weight(),
            font_style: default_font_style(),
            text_background_enabled: default_text_background(),
        }
    }
}

/// Drag bindings for all supported mouse buttons.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MouseDragToolsConfig {
    /// Left mouse button drag bindings.
    #[serde(default = "default_left_drag_button")]
    pub left: DragButtonConfig,

    /// Right mouse button drag bindings.
    #[serde(default = "default_button_behavior_drag_button")]
    pub right: DragButtonConfig,

    /// Middle mouse button drag bindings.
    #[serde(default = "default_button_behavior_drag_button")]
    pub middle: DragButtonConfig,

    /// Whether the left table was present in the user's config.
    #[serde(skip)]
    #[cfg_attr(feature = "config-schema", schemars(skip))]
    left_explicit: bool,
}

impl MouseDragToolsConfig {
    pub fn from_legacy(
        drag_tool: DragBindableTool,
        shift_drag_tool: DragBindableTool,
        ctrl_drag_tool: DragBindableTool,
        ctrl_shift_drag_tool: DragBindableTool,
        tab_drag_tool: DragBindableTool,
    ) -> Self {
        Self {
            left: DragButtonConfig::from_legacy(
                drag_tool,
                shift_drag_tool,
                ctrl_drag_tool,
                ctrl_shift_drag_tool,
                tab_drag_tool,
            ),
            right: DragButtonConfig::button_behavior(),
            middle: DragButtonConfig::button_behavior(),
            left_explicit: false,
        }
    }

    pub fn from_buttons(
        left: DragButtonConfig,
        right: DragButtonConfig,
        middle: DragButtonConfig,
    ) -> Self {
        Self {
            left,
            right,
            middle,
            left_explicit: true,
        }
    }

    pub fn left_explicit(&self) -> bool {
        self.left_explicit
    }

    pub fn resolve_with_left_defaults(mut self, left_defaults: &DragButtonConfig) -> Self {
        if self.left_explicit() {
            self.left.apply_defaults_from(left_defaults);
        } else {
            self.left = left_defaults.clone();
        }
        self.left_explicit = true;
        self
    }
}

impl<'de> Deserialize<'de> for MouseDragToolsConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct MouseDragToolsConfigFields {
            left: Option<DragButtonConfig>,
            right: Option<DragButtonConfig>,
            middle: Option<DragButtonConfig>,
        }

        let fields = MouseDragToolsConfigFields::deserialize(deserializer)?;
        let left_explicit = fields.left.is_some();
        Ok(Self {
            left: fields.left.unwrap_or_else(default_left_drag_button),
            right: fields
                .right
                .unwrap_or_else(default_button_behavior_drag_button),
            middle: fields
                .middle
                .unwrap_or_else(default_button_behavior_drag_button),
            left_explicit,
        })
    }
}

impl Default for MouseDragToolsConfig {
    fn default() -> Self {
        Self::from_legacy(
            default_drag_tool(),
            default_shift_drag_tool(),
            default_ctrl_drag_tool(),
            default_ctrl_shift_drag_tool(),
            default_tab_drag_tool(),
        )
    }
}

/// Drag bindings for one mouse button.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DragButtonConfig {
    /// Tool/action used for drag with no modifier.
    #[serde(default = "default_button_behavior_drag_tool")]
    pub drag_tool: DragTool,
    /// Optional color used for drag with no modifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drag_color: Option<ColorSpec>,

    /// Tool/action used for Shift+drag.
    #[serde(default = "default_button_behavior_drag_tool")]
    pub shift_drag_tool: DragTool,
    /// Optional color used for Shift+drag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shift_drag_color: Option<ColorSpec>,

    /// Tool/action used for Ctrl+drag.
    #[serde(default = "default_button_behavior_drag_tool")]
    pub ctrl_drag_tool: DragTool,
    /// Optional color used for Ctrl+drag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctrl_drag_color: Option<ColorSpec>,

    /// Tool/action used for Ctrl+Shift+drag.
    #[serde(default = "default_button_behavior_drag_tool")]
    pub ctrl_shift_drag_tool: DragTool,
    /// Optional color used for Ctrl+Shift+drag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ctrl_shift_drag_color: Option<ColorSpec>,

    /// Tool/action used for Tab+drag.
    #[serde(default = "default_button_behavior_drag_tool")]
    pub tab_drag_tool: DragTool,
    /// Optional color used for Tab+drag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_drag_color: Option<ColorSpec>,
}

impl DragButtonConfig {
    pub fn from_legacy(
        drag_tool: DragBindableTool,
        shift_drag_tool: DragBindableTool,
        ctrl_drag_tool: DragBindableTool,
        ctrl_shift_drag_tool: DragBindableTool,
        tab_drag_tool: DragBindableTool,
    ) -> Self {
        Self {
            drag_tool: drag_tool.to_drag_tool(),
            drag_color: None,
            shift_drag_tool: shift_drag_tool.to_drag_tool(),
            shift_drag_color: None,
            ctrl_drag_tool: ctrl_drag_tool.to_drag_tool(),
            ctrl_drag_color: None,
            ctrl_shift_drag_tool: ctrl_shift_drag_tool.to_drag_tool(),
            ctrl_shift_drag_color: None,
            tab_drag_tool: tab_drag_tool.to_drag_tool(),
            tab_drag_color: None,
        }
    }

    pub fn button_behavior() -> Self {
        Self {
            drag_tool: DragTool::Default,
            drag_color: None,
            shift_drag_tool: DragTool::Default,
            shift_drag_color: None,
            ctrl_drag_tool: DragTool::Default,
            ctrl_drag_color: None,
            ctrl_shift_drag_tool: DragTool::Default,
            ctrl_shift_drag_color: None,
            tab_drag_tool: DragTool::Default,
            tab_drag_color: None,
        }
    }

    fn apply_defaults_from(&mut self, defaults: &Self) {
        if self.drag_tool == DragTool::Default {
            self.drag_tool = defaults.drag_tool;
            if self.drag_color.is_none() {
                self.drag_color = defaults.drag_color.clone();
            }
        }
        if self.shift_drag_tool == DragTool::Default {
            self.shift_drag_tool = defaults.shift_drag_tool;
            if self.shift_drag_color.is_none() {
                self.shift_drag_color = defaults.shift_drag_color.clone();
            }
        }
        if self.ctrl_drag_tool == DragTool::Default {
            self.ctrl_drag_tool = defaults.ctrl_drag_tool;
            if self.ctrl_drag_color.is_none() {
                self.ctrl_drag_color = defaults.ctrl_drag_color.clone();
            }
        }
        if self.ctrl_shift_drag_tool == DragTool::Default {
            self.ctrl_shift_drag_tool = defaults.ctrl_shift_drag_tool;
            if self.ctrl_shift_drag_color.is_none() {
                self.ctrl_shift_drag_color = defaults.ctrl_shift_drag_color.clone();
            }
        }
        if self.tab_drag_tool == DragTool::Default {
            self.tab_drag_tool = defaults.tab_drag_tool;
            if self.tab_drag_color.is_none() {
                self.tab_drag_color = defaults.tab_drag_color.clone();
            }
        }
    }
}

impl Default for DragButtonConfig {
    fn default() -> Self {
        Self::button_behavior()
    }
}

impl DrawingConfig {
    pub fn effective_drag_tools(&self) -> MouseDragToolsConfig {
        match self.drag_tools.clone() {
            Some(drag_tools) => {
                let legacy_left = DragButtonConfig::from_legacy(
                    self.drag_tool,
                    self.shift_drag_tool,
                    self.ctrl_drag_tool,
                    self.ctrl_shift_drag_tool,
                    self.tab_drag_tool,
                );
                drag_tools.resolve_with_left_defaults(&legacy_left)
            }
            None => MouseDragToolsConfig::from_buttons(
                DragButtonConfig::from_legacy(
                    self.drag_tool,
                    self.shift_drag_tool,
                    self.ctrl_drag_tool,
                    self.ctrl_shift_drag_tool,
                    self.tab_drag_tool,
                ),
                DragButtonConfig::button_behavior(),
                DragButtonConfig::button_behavior(),
            ),
        }
    }
}

fn default_color() -> ColorSpec {
    ColorSpec::Name("red".to_string())
}

fn default_thickness() -> f64 {
    3.0
}

fn default_eraser_size() -> f64 {
    12.0
}

fn default_eraser_mode() -> EraserMode {
    EraserMode::Brush
}

fn default_marker_opacity() -> f64 {
    0.32
}

fn default_fill_enabled() -> bool {
    false
}

fn default_polygon_sides() -> u8 {
    REGULAR_POLYGON_DEFAULT_SIDES
}

fn default_font_size() -> f64 {
    32.0
}

fn default_font_family() -> String {
    "Sans".to_string()
}

fn default_font_weight() -> String {
    "bold".to_string()
}

fn default_font_style() -> String {
    "normal".to_string()
}

fn default_text_background() -> bool {
    false
}

fn default_hit_test_tolerance() -> f64 {
    6.0
}

fn default_hit_test_threshold() -> usize {
    400
}

fn default_undo_stack_limit() -> usize {
    100
}

fn default_drag_tool() -> DragBindableTool {
    DragBindableTool::Pen
}

fn default_shift_drag_tool() -> DragBindableTool {
    DragBindableTool::Line
}

fn default_ctrl_drag_tool() -> DragBindableTool {
    DragBindableTool::Rect
}

fn default_ctrl_shift_drag_tool() -> DragBindableTool {
    DragBindableTool::Arrow
}

fn default_tab_drag_tool() -> DragBindableTool {
    DragBindableTool::Ellipse
}

fn default_button_behavior_drag_tool() -> DragTool {
    DragTool::Default
}

fn default_left_drag_button() -> DragButtonConfig {
    DragButtonConfig::from_legacy(
        default_drag_tool(),
        default_shift_drag_tool(),
        default_ctrl_drag_tool(),
        default_ctrl_shift_drag_tool(),
        default_tab_drag_tool(),
    )
}

fn default_button_behavior_drag_button() -> DragButtonConfig {
    DragButtonConfig::button_behavior()
}
