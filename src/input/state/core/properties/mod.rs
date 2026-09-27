mod apply;
mod apply_selection;
mod entries;
pub(crate) mod metrics;
mod panel;
mod panel_layout;
mod state;
mod summary;
mod types;
mod utils;

pub use state::PropertiesPanelState;
pub use types::{
    PanelRect, PropertiesPanelHit, PropertiesPanelLayout, PropertiesPanelLock,
    PropertiesPanelSwatch, PropertiesRowControl, PropertiesRowGeometry, SelectionPropertyEntry,
    SelectionPropertyKind, SelectionPropertyValue, ShapePropertiesPanel,
};
