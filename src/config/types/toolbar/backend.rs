use serde::{Deserialize, Serialize};

/// Which frontend renders the toolbars.
///
/// `Gtk` requires the `toolbar-gtk` build feature and a compositor where the
/// built-in toolbars would use their own layer surfaces (layer-shell present,
/// no inline fallback). Whenever those preconditions fail the built-in Cairo
/// toolbars are used regardless of this setting.
#[cfg_attr(feature = "config-schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolbarBackendKind {
    /// Use the GTK4 toolbars where supported, built-in toolbars elsewhere.
    #[default]
    Auto,
    /// Request the GTK4 toolbars; falls back to built-in with a warning when
    /// unsupported.
    #[serde(alias = "gtk4")]
    Gtk,
    /// Always use the built-in Cairo toolbars.
    #[serde(alias = "cairo")]
    Builtin,
}
