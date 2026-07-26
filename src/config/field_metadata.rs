//! Shared metadata for the first declarative configurator field slice.
//!
//! Performance is deliberately the only section represented here. The configurator keeps its
//! typed draft and messages while sharing user-facing field identity and constraints with core
//! config validation.

pub const PERFORMANCE_BUFFER_COUNT_MIN: u32 = 2;
pub const PERFORMANCE_BUFFER_COUNT_MAX: u32 = 4;
pub const PERFORMANCE_BUFFER_COUNTS: &[u32] = &[
    PERFORMANCE_BUFFER_COUNT_MIN,
    3,
    PERFORMANCE_BUFFER_COUNT_MAX,
];
pub const PERFORMANCE_UI_ANIMATION_FPS_MAX: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceFieldPresentation {
    path: &'static str,
    group: PerformanceFieldGroup,
    label: &'static str,
    help: &'static str,
    search_terms: &'static [&'static str],
}

impl PerformanceFieldPresentation {
    pub const fn path(self) -> &'static str {
        self.path
    }

    pub const fn group(self) -> PerformanceFieldGroup {
        self.group
    }

    pub const fn label(self) -> &'static str {
        self.label
    }

    pub const fn help(self) -> &'static str {
        self.help
    }

    pub const fn search_terms(self) -> &'static [&'static str] {
        self.search_terms
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceBooleanFieldMetadata {
    presentation: PerformanceFieldPresentation,
}

impl PerformanceBooleanFieldMetadata {
    pub const fn presentation(self) -> PerformanceFieldPresentation {
        self.presentation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceUnsignedChoiceFieldMetadata {
    presentation: PerformanceFieldPresentation,
    choices: &'static [u32],
}

impl PerformanceUnsignedChoiceFieldMetadata {
    pub const fn presentation(self) -> PerformanceFieldPresentation {
        self.presentation
    }

    pub const fn choices(self) -> &'static [u32] {
        self.choices
    }

    pub const fn accepts(self, value: u32) -> bool {
        let mut index = 0;
        while index < self.choices.len() {
            if self.choices[index] == value {
                return true;
            }
            index += 1;
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceUnsignedRangeFieldMetadata {
    presentation: PerformanceFieldPresentation,
    min: u32,
    max: u32,
}

impl PerformanceUnsignedRangeFieldMetadata {
    pub const fn presentation(self) -> PerformanceFieldPresentation {
        self.presentation
    }

    pub const fn min(self) -> u32 {
        self.min
    }

    pub const fn max(self) -> u32 {
        self.max
    }

    pub const fn bounds(self) -> (u32, u32) {
        (self.min, self.max)
    }

    pub const fn accepts(self, value: u32) -> bool {
        value >= self.min && value <= self.max
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceFields {
    buffer_count: PerformanceUnsignedChoiceFieldMetadata,
    enable_vsync: PerformanceBooleanFieldMetadata,
    max_fps_no_vsync: PerformanceUnsignedRangeFieldMetadata,
    ui_animation_fps: PerformanceUnsignedRangeFieldMetadata,
}

impl PerformanceFields {
    pub const fn buffer_count(self) -> PerformanceUnsignedChoiceFieldMetadata {
        self.buffer_count
    }

    pub const fn enable_vsync(self) -> PerformanceBooleanFieldMetadata {
        self.enable_vsync
    }

    pub const fn max_fps_no_vsync(self) -> PerformanceUnsignedRangeFieldMetadata {
        self.max_fps_no_vsync
    }

    pub const fn ui_animation_fps(self) -> PerformanceUnsignedRangeFieldMetadata {
        self.ui_animation_fps
    }

    pub const fn presentations(self) -> [PerformanceFieldPresentation; 4] {
        [
            self.buffer_count.presentation,
            self.enable_vsync.presentation,
            self.max_fps_no_vsync.presentation,
            self.ui_animation_fps.presentation,
        ]
    }
}

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

pub const PERFORMANCE_FIELDS: PerformanceFields = PerformanceFields {
    buffer_count: PerformanceUnsignedChoiceFieldMetadata {
        presentation: PerformanceFieldPresentation {
            path: "performance.buffer_count",
            group: PerformanceFieldGroup::Rendering,
            label: "Buffer count (2-4)",
            help: "2 uses less memory; 3 is recommended; 4 adds another queued buffer.",
            search_terms: &["rendering", "buffer", "double triple quad buffering"],
        },
        choices: PERFORMANCE_BUFFER_COUNTS,
    },
    enable_vsync: PerformanceBooleanFieldMetadata {
        presentation: PerformanceFieldPresentation {
            path: "performance.enable_vsync",
            group: PerformanceFieldGroup::Rendering,
            label: "Enable VSync",
            help: "Synchronizes rendering with display refresh to prevent tearing, with some input latency.",
            search_terms: &["rendering", "vsync", "tearing", "display refresh"],
        },
    },
    max_fps_no_vsync: PerformanceUnsignedRangeFieldMetadata {
        presentation: PerformanceFieldPresentation {
            path: "performance.max_fps_no_vsync",
            group: PerformanceFieldGroup::Rendering,
            label: "Max FPS (VSync off)",
            help: "Caps frame rate when VSync is off. Default 120; try 144 or 240 on high-refresh displays; 0 means unlimited.",
            search_terms: &["rendering", "fps", "frame rate", "vsync off", "unlimited"],
        },
        min: 0,
        max: u32::MAX,
    },
    ui_animation_fps: PerformanceUnsignedRangeFieldMetadata {
        presentation: PerformanceFieldPresentation {
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
        },
        min: 0,
        max: PERFORMANCE_UI_ANIMATION_FPS_MAX,
    },
};

const fn legacy_metadata(
    id: PerformanceFieldId,
    presentation: PerformanceFieldPresentation,
    constraint: ScalarConstraint,
) -> PerformanceFieldMetadata {
    PerformanceFieldMetadata {
        id,
        path: presentation.path,
        group: presentation.group,
        label: presentation.label,
        help: presentation.help,
        search_terms: presentation.search_terms,
        constraint,
    }
}

static PERFORMANCE_FIELD_METADATA_ARRAY: [PerformanceFieldMetadata; 4] = [
    legacy_metadata(
        PerformanceFieldId::BufferCount,
        PERFORMANCE_FIELDS.buffer_count.presentation,
        ScalarConstraint::UnsignedChoice(PERFORMANCE_FIELDS.buffer_count.choices),
    ),
    legacy_metadata(
        PerformanceFieldId::EnableVsync,
        PERFORMANCE_FIELDS.enable_vsync.presentation,
        ScalarConstraint::Boolean,
    ),
    legacy_metadata(
        PerformanceFieldId::MaxFpsNoVsync,
        PERFORMANCE_FIELDS.max_fps_no_vsync.presentation,
        ScalarConstraint::Unsigned {
            min: PERFORMANCE_FIELDS.max_fps_no_vsync.min,
            max: PERFORMANCE_FIELDS.max_fps_no_vsync.max,
        },
    ),
    legacy_metadata(
        PerformanceFieldId::UiAnimationFps,
        PERFORMANCE_FIELDS.ui_animation_fps.presentation,
        ScalarConstraint::Unsigned {
            min: PERFORMANCE_FIELDS.ui_animation_fps.min,
            max: PERFORMANCE_FIELDS.ui_animation_fps.max,
        },
    ),
];

pub const PERFORMANCE_FIELD_METADATA: &[PerformanceFieldMetadata] =
    &PERFORMANCE_FIELD_METADATA_ARRAY;

pub fn performance_field_metadata(id: PerformanceFieldId) -> &'static PerformanceFieldMetadata {
    let [
        buffer_count,
        enable_vsync,
        max_fps_no_vsync,
        ui_animation_fps,
    ] = &PERFORMANCE_FIELD_METADATA_ARRAY;
    match id {
        PerformanceFieldId::BufferCount => buffer_count,
        PerformanceFieldId::EnableVsync => enable_vsync,
        PerformanceFieldId::MaxFpsNoVsync => max_fps_no_vsync,
        PerformanceFieldId::UiAnimationFps => ui_animation_fps,
    }
}
