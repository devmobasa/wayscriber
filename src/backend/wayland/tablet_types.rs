/// Physical tool types as defined by the zwp_tablet_tool_v2 protocol.
///
/// See: https://wayland.app/protocols/tablet-v2#zwp_tablet_tool_v2:enum:type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabletToolType {
    /// Pen tool (0x140)
    Pen,
    /// Eraser tool (0x141) - typically the back end of a stylus
    Eraser,
    /// Brush tool (0x142)
    Brush,
    /// Pencil tool (0x143)
    Pencil,
    /// Airbrush tool (0x144)
    Airbrush,
    /// Finger tool (0x145)
    Finger,
    /// Mouse tool (0x146) - tablet puck
    Mouse,
    /// Lens tool (0x147) - mouse-shaped with lens for precision
    Lens,
    /// Unknown tool type with raw protocol value
    Unknown(u32),
}

impl TabletToolType {
    /// Protocol value for pen tool
    const PEN_VALUE: u32 = 0x140;
    /// Protocol value for eraser tool
    const ERASER_VALUE: u32 = 0x141;
    /// Protocol value for brush tool
    const BRUSH_VALUE: u32 = 0x142;
    /// Protocol value for pencil tool
    const PENCIL_VALUE: u32 = 0x143;
    /// Protocol value for airbrush tool
    const AIRBRUSH_VALUE: u32 = 0x144;
    /// Protocol value for finger tool
    const FINGER_VALUE: u32 = 0x145;
    /// Protocol value for mouse tool
    const MOUSE_VALUE: u32 = 0x146;
    /// Protocol value for lens tool
    const LENS_VALUE: u32 = 0x147;

    /// Converts from the raw u32 protocol value to TabletToolType.
    pub fn from_raw(value: u32) -> Self {
        match value {
            Self::PEN_VALUE => TabletToolType::Pen,
            Self::ERASER_VALUE => TabletToolType::Eraser,
            Self::BRUSH_VALUE => TabletToolType::Brush,
            Self::PENCIL_VALUE => TabletToolType::Pencil,
            Self::AIRBRUSH_VALUE => TabletToolType::Airbrush,
            Self::FINGER_VALUE => TabletToolType::Finger,
            Self::MOUSE_VALUE => TabletToolType::Mouse,
            Self::LENS_VALUE => TabletToolType::Lens,
            other => TabletToolType::Unknown(other),
        }
    }

    /// Returns true if this is an eraser tool type.
    pub fn is_eraser(&self) -> bool {
        matches!(self, TabletToolType::Eraser)
    }
}

impl
    From<
        wayland_client::WEnum<wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Type>,
    > for TabletToolType
{
    fn from(
        tool_type: wayland_client::WEnum<
            wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Type,
        >,
    ) -> Self {
        use wayland_protocols::wp::tablet::zv2::client::zwp_tablet_tool_v2::Type;
        let raw_value: u32 = match tool_type {
            wayland_client::WEnum::Value(v) => match v {
                Type::Pen => Self::PEN_VALUE,
                Type::Eraser => Self::ERASER_VALUE,
                Type::Brush => Self::BRUSH_VALUE,
                Type::Pencil => Self::PENCIL_VALUE,
                Type::Airbrush => Self::AIRBRUSH_VALUE,
                Type::Finger => Self::FINGER_VALUE,
                Type::Mouse => Self::MOUSE_VALUE,
                Type::Lens => Self::LENS_VALUE,
                _ => 0,
            },
            wayland_client::WEnum::Unknown(v) => v,
        };
        Self::from_raw(raw_value)
    }
}
