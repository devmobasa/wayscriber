//! Bounded raster reuse keyed by content and the exact card pixels beneath it.
//! Retaining the backdrop preserves transparent paint and eraser semantics.
use super::content::render_page_content;
use super::types::PageContentArgs;
use crate::draw::{RenderCtx, TextMeasurer};
use crate::input::BoardBackground;
use crate::ui_text::UiTextEngine;
use pango::prelude::*;
use std::collections::{HashSet, VecDeque};

const MAX_ENTRIES: usize = 24;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(PartialEq)]
struct ThumbnailKey {
    content: u64,
    view: (i32, i32),
    background: Option<[u64; 3]>,
    backdrop: Vec<u8>,
    geometry: [u64; 4],
    matrix: [u64; 6],
    device_scale: [u64; 2],
    screen: (u32, u32),
    halo: bool,
    font_map: usize,
    font_serial: u32,
    font_options: cairo::FontOptions,
    antialias: cairo::Antialias,
    line_cap: cairo::LineCap,
    line_join: cairo::LineJoin,
    fill_rule: cairo::FillRule,
    line_values: [u64; 4],
    dashes: Vec<u64>,
}

struct Entry {
    key: ThumbnailKey,
    surface: cairo::ImageSurface,
    bytes: usize,
}

#[derive(Default)]
pub(crate) struct ThumbnailCache {
    entries: VecDeque<Entry>,
    bytes: usize,
    #[cfg(test)]
    hits: usize,
}

impl ThumbnailCache {
    pub(crate) fn retain_revisions(&mut self, revisions: impl Iterator<Item = u64>) {
        let live: HashSet<_> = revisions.collect();
        self.entries
            .retain(|entry| live.contains(&entry.key.content));
        self.bytes = self.entries.iter().map(|entry| entry.bytes).sum();
    }

    fn get(&mut self, key: &ThumbnailKey) -> Option<cairo::ImageSurface> {
        let index = self.entries.iter().position(|entry| entry.key == *key)?;
        let entry = self.entries.remove(index)?;
        let surface = entry.surface.clone();
        self.entries.push_back(entry);
        #[cfg(test)]
        {
            self.hits += 1;
        }
        Some(surface)
    }

    fn insert(&mut self, key: ThumbnailKey, surface: cairo::ImageSurface) {
        let bytes = surface.stride() as usize * surface.height() as usize + key.backdrop.len();
        if bytes > MAX_BYTES {
            return;
        }
        while self.entries.len() >= MAX_ENTRIES || self.bytes + bytes > MAX_BYTES {
            if let Some(entry) = self.entries.pop_front() {
                self.bytes -= entry.bytes;
            }
        }
        self.bytes += bytes;
        self.entries.push_back(Entry {
            key,
            surface,
            bytes,
        });
    }
}

pub(super) fn render_cached_page_content(
    engine: &UiTextEngine,
    measurer: &TextMeasurer,
    cache: &mut ThumbnailCache,
    mut args: PageContentArgs<'_, '_, '_>,
) {
    if try_render_cached(engine, measurer, cache, &mut args).is_none() {
        render_page_content(engine, measurer, args);
    }
}

fn try_render_cached(
    engine: &UiTextEngine,
    measurer: &TextMeasurer,
    cache: &mut ThumbnailCache,
    args: &mut PageContentArgs<'_, '_, '_>,
) -> Option<()> {
    let ctx = args.render.cairo;
    let matrix = ctx.matrix();
    let (dx, dy) = ctx.group_target().device_scale();
    // Native thumbnails use positive, axis-aligned transforms. Other callers
    // retain the direct renderer without changing its transform semantics.
    if matrix.xy() != 0.0
        || matrix.yx() != 0.0
        || matrix.xx() <= 0.0
        || matrix.yy() <= 0.0
        || ctx.operator() != cairo::Operator::Over
    {
        return None;
    }
    let clip = ctx.copy_clip_rectangle_list().ok()?;
    if !clip.iter().any(|rect| {
        rect.x() <= args.x
            && rect.y() <= args.y
            && rect.x() + rect.width() >= args.x + args.width
            && rect.y() + rect.height() >= args.y + args.height
    }) {
        return None;
    }
    let left = ((args.x * matrix.xx() + matrix.x0()) * dx).floor();
    let top = ((args.y * matrix.yy() + matrix.y0()) * dy).floor();
    let width = (((args.x + args.width) * matrix.xx() + matrix.x0()) * dx).ceil() - left;
    let height = (((args.y + args.height) * matrix.yy() + matrix.y0()) * dy).ceil() - top;
    if ![left, top, width, height, dx, dy]
        .iter()
        .all(|value| value.is_finite())
        || width <= 0.0
        || height <= 0.0
        || dx <= 0.0
        || dy <= 0.0
        || width * height * 8.0 > MAX_BYTES as f64
    {
        return None;
    }
    let destination = cairo::ImageSurface::try_from(ctx.group_target()).ok()?;
    if destination.device_offset() != (0.0, 0.0)
        || !matches!(
            destination.format(),
            cairo::Format::ARgb32 | cairo::Format::Rgb24
        )
        || left < 0.0
        || top < 0.0
        || left + width > f64::from(destination.width())
        || top + height > f64::from(destination.height())
    {
        return None;
    }
    // Exact bytes avoid a hash-collision or ambient-background assumption for
    // transparent boards and eraser strokes. This read is bounded by card size.
    let mut backdrop = Vec::with_capacity(width as usize * height as usize * 4);
    destination
        .with_data(|pixels| {
            for row in top as usize..(top + height) as usize {
                let start = row * destination.stride() as usize + left as usize * 4;
                backdrop.extend_from_slice(&pixels[start..start + width as usize * 4]);
            }
        })
        .ok()?;
    let font_map = pangocairo::FontMap::default();
    let key = ThumbnailKey {
        content: args.frame.content_revision(),
        view: args.frame.view_offset(),
        background: match args.background {
            BoardBackground::Solid(color) => {
                Some([color.r.to_bits(), color.g.to_bits(), color.b.to_bits()])
            }
            BoardBackground::Transparent => None,
        },
        backdrop,
        geometry: [
            args.x.to_bits(),
            args.y.to_bits(),
            args.width.to_bits(),
            args.height.to_bits(),
        ],
        matrix: [
            matrix.xx().to_bits(),
            matrix.yx().to_bits(),
            matrix.xy().to_bits(),
            matrix.yy().to_bits(),
            matrix.x0().to_bits(),
            matrix.y0().to_bits(),
        ],
        device_scale: [dx.to_bits(), dy.to_bits()],
        screen: (args.screen_width, args.screen_height),
        halo: args.text_halo_enabled,
        font_map: font_map.as_ptr() as usize,
        font_serial: font_map.serial(),
        font_options: ctx.font_options().ok()?,
        antialias: ctx.antialias(),
        line_cap: ctx.line_cap(),
        line_join: ctx.line_join(),
        fill_rule: ctx.fill_rule(),
        line_values: [
            ctx.line_width().to_bits(),
            ctx.miter_limit().to_bits(),
            ctx.tolerance().to_bits(),
            ctx.dash_offset().to_bits(),
        ],
        dashes: ctx.dash_dashes().into_iter().map(f64::to_bits).collect(),
    };
    let surface = if let Some(surface) = cache.get(&key) {
        surface
    } else {
        let surface =
            cairo::ImageSurface::create(cairo::Format::ARgb32, width as i32, height as i32).ok()?;
        surface.set_device_scale(dx, dy);
        let target = cairo::Context::new(&surface).ok()?;
        target.set_operator(cairo::Operator::Source);
        target
            .set_source_surface(&destination, -left / dx, -top / dy)
            .ok()?;
        target.paint().ok()?;
        target.set_operator(cairo::Operator::Over);
        target.set_matrix(cairo::Matrix::new(
            matrix.xx(),
            matrix.yx(),
            matrix.xy(),
            matrix.yy(),
            matrix.x0() - left / dx,
            matrix.y0() - top / dy,
        ));
        target.set_font_options(&key.font_options);
        target.set_antialias(key.antialias);
        target.set_line_cap(key.line_cap);
        target.set_line_join(key.line_join);
        target.set_fill_rule(key.fill_rule);
        target.set_line_width(ctx.line_width());
        target.set_miter_limit(ctx.miter_limit());
        target.set_tolerance(ctx.tolerance());
        target.set_dash(&ctx.dash_dashes(), ctx.dash_offset());
        render_page_content(
            engine,
            measurer,
            PageContentArgs {
                render: &mut RenderCtx::new(&target, args.render.caches),
                frame: args.frame,
                background: args.background,
                x: args.x,
                y: args.y,
                width: args.width,
                height: args.height,
                screen_width: args.screen_width,
                screen_height: args.screen_height,
                text_halo_enabled: args.text_halo_enabled,
            },
        );
        target.status().ok()?;
        drop(target);
        cache.insert(key, surface.clone());
        surface
    };
    ctx.save().ok()?;
    ctx.identity_matrix();
    ctx.rectangle(left / dx, top / dy, width / dx, height / dy);
    ctx.clip();
    ctx.set_operator(cairo::Operator::Source);
    let painted = ctx
        .set_source_surface(&surface, left / dx, top / dy)
        .and_then(|()| {
            ctx.source().set_filter(cairo::Filter::Nearest);
            ctx.paint()
        });
    let restored = ctx.restore();
    painted.ok()?;
    restored.ok()?;
    Some(())
}

#[cfg(test)]
mod tests;
