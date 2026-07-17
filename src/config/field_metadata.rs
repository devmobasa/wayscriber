//! Shared metadata for the first declarative configurator field slice.
//!
//! Performance is deliberately the only section represented here. The configurator keeps its
//! typed draft and messages while sharing user-facing field identity and constraints with core
//! config validation.

pub const PERFORMANCE_BUFFER_COUNT_MIN: u32 = 2;
pub const PERFORMANCE_BUFFER_COUNT_MAX: u32 = 4;
pub const PERFORMANCE_BUFFER_COUNTS: &[u32] = &[2, 3, 4];
pub const PERFORMANCE_UI_ANIMATION_FPS_MAX: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PerformanceFieldId {
    BufferCount,
    EnableVsync,
    MaxFpsNoVsync,
    UiAnimationFps,
}

impl PerformanceFieldId {
    pub const ALL: [Self; 4] = [
        Self::BufferCount,
        Self::EnableVsync,
        Self::MaxFpsNoVsync,
        Self::UiAnimationFps,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceFieldGroup {
    Rendering,
    Animations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarConstraint {
    Boolean,
    Unsigned { min: u32, max: u32 },
    UnsignedChoice(&'static [u32]),
}

impl ScalarConstraint {
    pub const fn accepts_u32(self, value: u32) -> bool {
        match self {
            Self::Boolean => false,
            Self::Unsigned { min, max } => value >= min && value <= max,
            Self::UnsignedChoice(values) => {
                let mut index = 0;
                while index < values.len() {
                    if values[index] == value {
                        return true;
                    }
                    index += 1;
                }
                false
            }
        }
    }

    pub const fn unsigned_range(self) -> Option<(u32, u32)> {
        match self {
            Self::Unsigned { min, max } => Some((min, max)),
            Self::Boolean | Self::UnsignedChoice(_) => None,
        }
    }

    pub const fn unsigned_choices(self) -> Option<&'static [u32]> {
        match self {
            Self::UnsignedChoice(values) => Some(values),
            Self::Boolean | Self::Unsigned { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceFieldMetadata {
    pub id: PerformanceFieldId,
    pub path: &'static str,
    pub group: PerformanceFieldGroup,
    pub label: &'static str,
    pub help: &'static str,
    pub search_terms: &'static [&'static str],
    pub constraint: ScalarConstraint,
}

pub const PERFORMANCE_FIELD_METADATA: &[PerformanceFieldMetadata] = &[
    PerformanceFieldMetadata {
        id: PerformanceFieldId::BufferCount,
        path: "performance.buffer_count",
        group: PerformanceFieldGroup::Rendering,
        label: "Buffer count (2-4)",
        help: "2 uses less memory; 3 is recommended; 4 adds another queued buffer.",
        search_terms: &["rendering", "buffer", "double triple quad buffering"],
        constraint: ScalarConstraint::UnsignedChoice(PERFORMANCE_BUFFER_COUNTS),
    },
    PerformanceFieldMetadata {
        id: PerformanceFieldId::EnableVsync,
        path: "performance.enable_vsync",
        group: PerformanceFieldGroup::Rendering,
        label: "Enable VSync",
        help: "Synchronizes rendering with display refresh to prevent tearing, with some input latency.",
        search_terms: &["rendering", "vsync", "tearing", "display refresh"],
        constraint: ScalarConstraint::Boolean,
    },
    PerformanceFieldMetadata {
        id: PerformanceFieldId::MaxFpsNoVsync,
        path: "performance.max_fps_no_vsync",
        group: PerformanceFieldGroup::Rendering,
        label: "Max FPS (VSync off)",
        help: "Caps frame rate when VSync is off. Default 120; try 144 or 240 on high-refresh displays; 0 means unlimited.",
        search_terms: &["rendering", "fps", "frame rate", "vsync off", "unlimited"],
        constraint: ScalarConstraint::Unsigned {
            min: 0,
            max: u32::MAX,
        },
    },
    PerformanceFieldMetadata {
        id: PerformanceFieldId::UiAnimationFps,
        path: "performance.ui_animation_fps",
        group: PerformanceFieldGroup::Animations,
        label: "UI Animation FPS",
        help: "Controls UI effect ticks without changing input responsiveness; 30-60 is recommended, 0 means unlimited.",
        search_terms: &[
            "animation",
            "ui",
            "fps",
            "effects",
            "toasts",
            "click highlights",
        ],
        constraint: ScalarConstraint::Unsigned {
            min: 0,
            max: PERFORMANCE_UI_ANIMATION_FPS_MAX,
        },
    },
];

pub fn performance_field_metadata(id: PerformanceFieldId) -> &'static PerformanceFieldMetadata {
    PERFORMANCE_FIELD_METADATA
        .iter()
        .find(|metadata| metadata.id == id)
        .expect("every PerformanceFieldId must have metadata")
}
