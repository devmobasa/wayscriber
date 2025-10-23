//! RGBA color type and predefined color constants.

/// Represents an RGBA color with floating-point components.
///
/// All components are in the range 0.0 (minimum) to 1.0 (maximum).
///
/// # Examples
///
/// ```
/// use wayscriber::draw::Color;
/// let red = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
/// let semi_transparent_blue = Color { r: 0.0, g: 0.0, b: 1.0, a: 0.5 };
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    /// Red component (0.0 = no red, 1.0 = full red)
    pub r: f64,
    /// Green component (0.0 = no green, 1.0 = full green)
    pub g: f64,
    /// Blue component (0.0 = no blue, 1.0 = full blue)
    pub b: f64,
    /// Alpha/transparency (0.0 = fully transparent, 1.0 = fully opaque)
    pub a: f64,
}

impl Color {
    /// Creates a new color from RGBA components.
    ///
    /// All values should be in the range 0.0 to 1.0.
    /// This method is kept for future extensibility (custom colors in config file).
    #[allow(dead_code)]
    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }
}

// ============================================================================
// Predefined Color Constants (ZoomIt-inspired palette)
// ============================================================================

/// Predefined red color (R=1.0, G=0.0, B=0.0)
pub const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// Predefined green color (R=0.0, G=1.0, B=0.0)
pub const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

/// Predefined blue color (R=0.0, G=0.0, B=1.0)
pub const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// Predefined yellow color (R=1.0, G=1.0, B=0.0)
pub const YELLOW: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

/// Predefined orange color (R=1.0, G=0.5, B=0.0)
pub const ORANGE: Color = Color {
    r: 1.0,
    g: 0.5,
    b: 0.0,
    a: 1.0,
};

/// Predefined pink/magenta color (R=1.0, G=0.0, B=1.0)
pub const PINK: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// Predefined white color (R=1.0, G=1.0, B=1.0)
pub const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

/// Predefined black color (R=0.0, G=0.0, B=0.0)
pub const BLACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// Fully transparent color - kept for future use (e.g., effects, config file)
#[allow(dead_code)]
pub const TRANSPARENT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};
