//! Declarative view engine for the toolbars.
//!
//! One pure build pass per bar produces a [`WidgetTree`] of typed
//! [`WidgetNode`]s in logical coordinates. A single painter draws the tree
//! and the hit-tester, focus traversal, tooltips, and drag routing all query
//! the same nodes — so every rect exists in exactly one place. This replaces
//! the historical split where render code pushed hit regions while drawing
//! and a parallel test-only builder re-derived the same geometry.
//!
//! Coordinates: trees are always built in logical units. Input events are
//! mapped from surface coordinates into logical space through a single
//! [`ViewTransform`] before hit-testing; the painter applies the same
//! transform when drawing. No per-node coordinate fixups exist by design.

pub mod flow;
pub mod measure;
pub mod node;
pub mod transform;
pub mod tree;

#[allow(unused_imports)]
pub use flow::RowCursor;
#[allow(unused_imports)]
pub use measure::{FixedMeasure, TextMeasure};
#[allow(unused_imports)]
pub use node::{ButtonStyle, IconFn, Interaction, LabelSpec, WidgetId, WidgetKind, WidgetNode};
#[allow(unused_imports)]
pub use transform::ViewTransform;
#[allow(unused_imports)]
pub use tree::WidgetTree;
