mod apply;
mod apply_selection;
mod entries;
pub(crate) mod metrics;
mod panel;
mod panel_layout;
mod slider;
mod state;
mod summary;
mod types;
mod utils;

pub(crate) use apply_selection::RecolorOpacity;
pub use state::PropertiesPanelState;
pub use types::{
    LevelRange, PanelAction, PanelRect, PanelScroll, PropertiesPanelHit, PropertiesPanelLayout,
    PropertiesPanelLock, PropertiesPanelSwatch, PropertiesRowControl, PropertiesRowGeometry,
    SelectionPropertyEntry, SelectionPropertyKind, SelectionPropertyValue, ShapePropertiesPanel,
};
