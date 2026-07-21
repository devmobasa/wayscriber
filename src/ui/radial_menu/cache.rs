//! Prerendered static wedge base for the radial menu.
//!
//! The rest-state rings (color swatches, compass wedges with their glyphs,
//! labels, and keycap hints, and the size-ring track) are rendered once into
//! a thread-local `ImageSurface` and blitted per frame; hover overlays, the
//! sub-ring, the size value arc, and the center well stay dynamic on top.
//! The cache key covers everything the base bakes in — surface resolution,
//! palette + recents, binding hints, the slice table, and the active
//! tool/color state — so any change invalidates the surface. The theme is
//! process-fixed (`theme::init` is first-writer-wins), so it is not part of
//! the key.

use std::cell::RefCell;

use cairo::{Context, Format, ImageSurface};

use crate::input::state::{
    InputState, RADIAL_COMPASS_SLICES, RadialMenuLayout, RadialRingSwatch, RadialSliceKind,
    sub_ring_children,
};
use crate::ui::theme;

/// Extra logical padding around the outermost ring so borders and the round
/// track caps never clip at the surface edge.
const BASE_PAD: f64 = 2.0;

/// Cache key for the static wedge base. Everything rendered into the base
/// surface must be derivable from this key.
#[derive(Clone, PartialEq, Debug)]
pub(super) struct BaseKey {
    /// Physical pixel size of the (square) base surface.
    px: i32,
    /// Device scale in thousandths (stable f64 comparison).
    scale_thousandths: i64,
    /// Quick color palette fingerprint (colors, labels, radial length).
    palette: String,
    /// Session recent colors (order + rgba).
    recents: String,
    /// Primary keycap hint per compass action slice.
    bindings: String,
    /// Active tool/text-mode/color snapshot (selected wedge + swatch).
    actives: String,
    /// Compass slice table + parent children fingerprint.
    slices: String,
}

struct CachedBase {
    key: BaseKey,
    surface: ImageSurface,
}

thread_local! {
    static BASE_CACHE: RefCell<Option<CachedBase>> = const { RefCell::new(None) };
}

/// Blit the (possibly rebuilt) static base centered on the layout center.
/// Falls back to drawing the base directly when an offscreen surface cannot
/// be created.
pub(super) fn paint_base(
    ctx: &cairo::Context,
    input_state: &InputState,
    layout: &RadialMenuLayout,
    theme: &theme::Theme,
    swatches: &[RadialRingSwatch],
) {
    let extent = base_extent(layout);
    let scale = base_scale(ctx);
    let key = base_cache_key(input_state, swatches, extent, scale);

    let surface = BASE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(cached) = cache.as_ref().filter(|cached| cached.key == key) {
            return Some(cached.surface.clone());
        }
        let surface = render_base_surface(input_state, layout, theme, swatches, extent, scale)?;
        *cache = Some(CachedBase {
            key,
            surface: surface.clone(),
        });
        Some(surface)
    });

    match surface {
        Some(surface) => {
            let _ = ctx.save();
            let _ = ctx.set_source_surface(
                &surface,
                layout.center_x - extent,
                layout.center_y - extent,
            );
            let _ = ctx.paint();
            let _ = ctx.restore();
        }
        None => super::draw_static_base(
            ctx,
            input_state,
            theme,
            layout.center_x,
            layout.center_y,
            layout,
            swatches,
        ),
    }
}

/// Logical half-size of the base surface.
fn base_extent(layout: &RadialMenuLayout) -> f64 {
    layout.size_outer + BASE_PAD
}

/// Device scale of the destination context (the UI layer applies an integer
/// HiDPI scale transform before rendering overlays).
fn base_scale(ctx: &cairo::Context) -> f64 {
    let scale = ctx.matrix().xx().abs();
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

/// Build the cache key for the current input state.
pub(super) fn base_cache_key(
    input_state: &InputState,
    swatches: &[RadialRingSwatch],
    extent: f64,
    scale: f64,
) -> BaseKey {
    let active_tool = input_state.active_tool();
    let active_color = input_state.color_for_tool(active_tool);
    let text_input = matches!(
        input_state.state,
        crate::input::DrawingState::TextInput { .. }
    );

    let bindings = RADIAL_COMPASS_SLICES
        .iter()
        .map(|slice| match slice.kind {
            RadialSliceKind::Action(action) => input_state
                .action_binding_primary_label(action)
                .unwrap_or_default(),
            RadialSliceKind::Parent(_) => String::new(),
        })
        .collect::<Vec<_>>()
        .join("|");

    let recents = swatches
        .iter()
        .filter(|swatch| swatch.recent)
        .map(|swatch| color_key(&swatch.color))
        .collect::<Vec<_>>()
        .join("|");

    let slices = format!(
        "{:?};shapes={:?};notes={:?}",
        RADIAL_COMPASS_SLICES,
        sub_ring_children(2),
        sub_ring_children(5),
    );

    BaseKey {
        px: physical_size(extent, scale),
        scale_thousandths: (scale * 1000.0).round() as i64,
        palette: input_state.quick_colors.cache_key(),
        recents,
        bindings,
        actives: format!(
            "{active_tool:?};text={text_input};mode={:?};color={}",
            input_state.text_input_mode,
            color_key(&active_color),
        ),
        slices,
    }
}

/// Render the static base into a fresh offscreen surface, centered.
fn render_base_surface(
    input_state: &InputState,
    layout: &RadialMenuLayout,
    theme: &theme::Theme,
    swatches: &[RadialRingSwatch],
    extent: f64,
    scale: f64,
) -> Option<ImageSurface> {
    let px = physical_size(extent, scale);
    let surface = ImageSurface::create(Format::ARgb32, px, px).ok()?;
    surface.set_device_scale(scale, scale);
    {
        let ctx = Context::new(&surface).ok()?;
        super::draw_static_base(&ctx, input_state, theme, extent, extent, layout, swatches);
    }
    Some(surface)
}

fn physical_size(extent: f64, scale: f64) -> i32 {
    ((extent * 2.0) * scale).ceil() as i32
}

fn color_key(color: &crate::draw::Color) -> String {
    format!(
        "{:.4},{:.4},{:.4},{:.4}",
        color.r, color.g, color.b, color.a
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Color;
    use crate::input::Tool;
    use crate::input::state::test_support::make_test_input_state;

    const EXTENT: f64 = 192.0;

    fn key_for(state: &InputState) -> BaseKey {
        base_cache_key(state, &state.radial_ring_swatches(), EXTENT, 1.0)
    }

    /// The same state must produce the same key (a stable key is what makes
    /// the cache hit at all).
    #[test]
    fn base_cache_key_is_stable_for_unchanged_state() {
        let state = make_test_input_state();
        assert_eq!(key_for(&state), key_for(&state));
    }

    /// Every baked-in input invalidates the key: scale, palette, recents,
    /// and the active tool/color snapshot.
    #[test]
    fn base_cache_key_changes_with_each_baked_input() {
        let mut state = make_test_input_state();
        let base = key_for(&state);

        // Resolution / device scale
        let scaled = base_cache_key(&state, &state.radial_ring_swatches(), EXTENT, 2.0);
        assert_ne!(base, scaled, "device scale must be part of the key");

        // Recents arc
        state.apply_color_from_ui(Color {
            r: 0.123,
            g: 0.456,
            b: 0.789,
            a: 1.0,
        });
        let with_recent = key_for(&state);
        assert_ne!(base, with_recent, "a new recent color must invalidate");

        // Active tool (selected wedge is baked into the base)
        assert!(state.set_tool_override(Some(Tool::Eraser)));
        let with_tool = key_for(&state);
        assert_ne!(with_recent, with_tool, "active tool must invalidate");

        // Active color (selected swatch border is baked into the base)
        assert!(state.set_color(Color {
            r: 0.9,
            g: 0.1,
            b: 0.2,
            a: 1.0,
        }));
        assert_ne!(with_tool, key_for(&state), "active color must invalidate");
    }

    /// Changing the quick palette (colors the base bakes as swatches)
    /// invalidates the key.
    #[test]
    fn base_cache_key_changes_with_quick_palette() {
        let mut state = make_test_input_state();
        let base = key_for(&state);
        state.set_quick_colors(crate::config::QuickColorPalette::from_entries(vec![
            crate::config::QuickColorPaletteEntry {
                label: "Only".to_string(),
                color: Color {
                    r: 0.2,
                    g: 0.4,
                    b: 0.6,
                    a: 1.0,
                },
            },
        ]));
        assert_ne!(base, key_for(&state));
    }

    /// Changing a compass action's primary binding label (the baked keycap
    /// hints) invalidates the key.
    #[test]
    fn base_cache_key_changes_with_binding_hints() {
        use crate::config::{Action, KeyBinding};
        use crate::input::state::test_support::make_test_input_state_with_action_bindings;

        let default_state = make_test_input_state();
        let mut bindings = crate::config::KeybindingsConfig::default()
            .build_action_bindings()
            .expect("default bindings");
        bindings.insert(
            Action::SelectPenTool,
            vec![KeyBinding::parse("F9").expect("parse F9")],
        );
        let rebound_state = make_test_input_state_with_action_bindings(bindings);

        assert_ne!(key_for(&default_state), key_for(&rebound_state));
    }
}
