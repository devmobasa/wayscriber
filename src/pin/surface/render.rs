//! Static image raster caching and self-contained control painting.

use anyhow::{Context, Result};

use super::{Control, CopyVisual, VisualState, control_strip};
use crate::pin::{PinFrame, PinImage};

#[derive(Debug, Clone)]
pub(crate) struct RasterCache {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Damage {
    Full,
    Controls {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
}

pub(crate) fn build_static_raster(
    image: &PinImage,
    physical_size: (u32, u32),
    scale: i32,
) -> Result<RasterCache> {
    let (width, height) = physical_size;
    let stride = checked_stride(width)?;
    let len = checked_len(height, stride)?;
    let mut bytes = vec![0; len];

    // SAFETY: source points into immutable Arc storage that outlives the Cairo
    // surface and context below. Cairo is only asked to read it. Target owns
    // `bytes`, whose validated stride/length cover the complete image.
    let source = unsafe {
        cairo::ImageSurface::create_for_data_unsafe(
            image.argb32.as_ptr().cast_mut(),
            cairo::Format::ARgb32,
            i32::try_from(image.width)?,
            i32::try_from(image.height)?,
            image.stride,
        )
    }
    .context("wrap decoded pin image")?;
    let target = unsafe {
        cairo::ImageSurface::create_for_data_unsafe(
            bytes.as_mut_ptr(),
            cairo::Format::ARgb32,
            i32::try_from(width)?,
            i32::try_from(height)?,
            stride,
        )
    }
    .context("wrap pin raster cache")?;
    let ctx = cairo::Context::new(&target).context("create pin raster context")?;
    ctx.set_operator(cairo::Operator::Clear);
    ctx.paint().context("clear pin raster")?;
    ctx.set_operator(cairo::Operator::Over);
    let scale = f64::from(scale.max(1));
    let padding = f64::from(super::CHROME_PADDING) * scale;
    let content_width = f64::from(width) - padding * 2.0;
    let content_height = f64::from(height) - padding * 2.0;
    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.3);
    rounded_rect(
        &ctx,
        padding - 3.0 * scale,
        padding - 2.0 * scale,
        content_width + 6.0 * scale,
        content_height + 6.0 * scale,
        7.0 * scale,
    );
    ctx.fill().context("paint pin shadow")?;
    ctx.save().context("save pin image transform")?;
    ctx.translate(padding, padding);
    ctx.scale(
        content_width / f64::from(image.width),
        content_height / f64::from(image.height),
    );
    ctx.set_source_surface(&source, 0.0, 0.0)
        .context("set pin image source")?;
    ctx.source().set_filter(
        if content_width == f64::from(image.width) && content_height == f64::from(image.height) {
            cairo::Filter::Nearest
        } else {
            cairo::Filter::Bilinear
        },
    );
    ctx.paint().context("paint pin image")?;
    ctx.restore().context("restore pin image transform")?;
    ctx.set_source_rgba(0.92, 0.93, 0.95, 0.72);
    ctx.set_line_width(scale);
    ctx.rectangle(
        padding + scale / 2.0,
        padding + scale / 2.0,
        (content_width - scale).max(0.0),
        (content_height - scale).max(0.0),
    );
    ctx.stroke().context("paint pin border")?;
    target.flush();
    drop(ctx);
    drop(target);
    drop(source);
    Ok(RasterCache {
        width,
        height,
        bytes,
    })
}

pub(crate) fn render_frame(
    cache: &RasterCache,
    canvas_ptr: usize,
    canvas_len: usize,
    logical_frame: PinFrame,
    scale: i32,
    visual: &VisualState,
    full_damage: bool,
) -> Result<Damage> {
    if cache.bytes.len() > canvas_len {
        anyhow::bail!("pin SHM canvas is smaller than raster cache");
    }
    // SAFETY: `PinBuffers::acquire` provides a writable slot pointer of at
    // least `canvas_len`, and the caller does not attach it until this returns.
    unsafe {
        std::ptr::copy_nonoverlapping(
            cache.bytes.as_ptr(),
            canvas_ptr as *mut u8,
            cache.bytes.len(),
        );
    }
    let stride = checked_stride(cache.width)?;
    let target = unsafe {
        cairo::ImageSurface::create_for_data_unsafe(
            canvas_ptr as *mut u8,
            cairo::Format::ARgb32,
            i32::try_from(cache.width)?,
            i32::try_from(cache.height)?,
            stride,
        )
    }
    .context("wrap pin frame buffer")?;
    let ctx = cairo::Context::new(&target).context("create pin frame context")?;
    paint_controls(&ctx, logical_frame, scale.max(1), visual)?;
    target.flush();

    if full_damage {
        Ok(Damage::Full)
    } else {
        let (x, y, width, height) = control_strip(logical_frame);
        let scale = scale.max(1);
        let padding = i32::try_from(super::CHROME_PADDING).unwrap_or(i32::MAX);
        Ok(Damage::Controls {
            x: x.saturating_add(padding).saturating_mul(scale),
            y: y.saturating_add(padding).saturating_mul(scale),
            width: width.saturating_mul(scale as u32),
            height: height.saturating_mul(scale as u32),
        })
    }
}

fn paint_controls(
    ctx: &cairo::Context,
    frame: PinFrame,
    scale: i32,
    visual: &VisualState,
) -> Result<()> {
    ctx.save().context("save pin control context")?;
    ctx.scale(f64::from(scale), f64::from(scale));
    ctx.translate(
        f64::from(super::CHROME_PADDING),
        f64::from(super::CHROME_PADDING),
    );
    let (_, _, strip_width, strip_height) = control_strip(frame);
    let x = f64::from(frame.width.saturating_sub(strip_width));
    let opacity = if visual.pointer_position.is_some() {
        0.88
    } else {
        0.28
    };
    ctx.set_source_rgba(0.06, 0.07, 0.09, opacity);
    rounded_rect(
        ctx,
        x,
        0.0,
        f64::from(strip_width),
        f64::from(strip_height),
        8.0,
    );
    ctx.fill().context("paint pin control strip")?;

    paint_control(ctx, frame, Control::Copy, visual)?;
    paint_control(ctx, frame, Control::Close, visual)?;
    ctx.restore().context("restore pin control context")?;
    Ok(())
}

fn paint_control(
    ctx: &cairo::Context,
    frame: PinFrame,
    control: Control,
    visual: &VisualState,
) -> Result<()> {
    use super::hit::{CONTROL_GAP, CONTROL_INSET, CONTROL_SIZE};
    let right = f64::from(frame.width) - CONTROL_INSET;
    let x = match control {
        Control::Close => right - CONTROL_SIZE,
        Control::Copy => right - CONTROL_SIZE * 2.0 - CONTROL_GAP,
    };
    let hovered = visual.hover == Some(control);
    let pressed = visual.pressed == Some(control);
    if hovered || pressed {
        let (r, g, b) = if control == Control::Close {
            (0.83, 0.20, 0.24)
        } else {
            (0.28, 0.48, 0.88)
        };
        ctx.set_source_rgba(r, g, b, if pressed { 1.0 } else { 0.82 });
        rounded_rect(ctx, x, CONTROL_INSET, CONTROL_SIZE, CONTROL_SIZE, 6.0);
        ctx.fill().context("paint pin control hover")?;
    }

    ctx.set_source_rgba(1.0, 1.0, 1.0, 0.95);
    ctx.set_line_width(2.0);
    let cx = x + CONTROL_SIZE / 2.0;
    let cy = CONTROL_INSET + CONTROL_SIZE / 2.0;
    match control {
        Control::Close => {
            ctx.move_to(cx - 5.0, cy - 5.0);
            ctx.line_to(cx + 5.0, cy + 5.0);
            ctx.move_to(cx + 5.0, cy - 5.0);
            ctx.line_to(cx - 5.0, cy + 5.0);
            ctx.stroke().context("paint close glyph")?;
        }
        Control::Copy => match visual.copy {
            CopyVisual::Idle => {
                ctx.rectangle(cx - 6.0, cy - 7.0, 9.0, 11.0);
                ctx.rectangle(cx - 2.0, cy - 3.0, 9.0, 11.0);
                ctx.stroke().context("paint copy glyph")?;
            }
            CopyVisual::Copying => {
                ctx.arc(
                    cx,
                    cy,
                    6.0,
                    -std::f64::consts::FRAC_PI_2,
                    std::f64::consts::PI,
                );
                ctx.stroke().context("paint copying spinner")?;
            }
            CopyVisual::Succeeded { .. } => {
                ctx.move_to(cx - 6.0, cy);
                ctx.line_to(cx - 1.5, cy + 4.5);
                ctx.line_to(cx + 7.0, cy - 5.0);
                ctx.stroke().context("paint copy success check")?;
            }
            CopyVisual::Failed { .. } => {
                ctx.move_to(cx, cy - 7.0);
                ctx.line_to(cx, cy + 2.0);
                ctx.stroke().context("paint copy failure mark")?;
                ctx.arc(cx, cy + 6.0, 1.1, 0.0, std::f64::consts::TAU);
                ctx.fill().context("paint copy failure dot")?;
            }
        },
    }
    Ok(())
}

fn rounded_rect(ctx: &cairo::Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    let radius = radius.min(width / 2.0).min(height / 2.0);
    ctx.new_sub_path();
    ctx.arc(
        x + width - radius,
        y + radius,
        radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    ctx.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    ctx.arc(
        x + radius,
        y + height - radius,
        radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    ctx.arc(
        x + radius,
        y + radius,
        radius,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );
    ctx.close_path();
}

fn checked_stride(width: u32) -> Result<i32> {
    i32::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .context("pin raster stride overflow")
}

fn checked_len(height: u32, stride: i32) -> Result<usize> {
    usize::try_from(height)
        .ok()
        .and_then(|height| height.checked_mul(usize::try_from(stride).ok()?))
        .context("pin raster byte length overflow")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn static_raster_keeps_transparent_chrome_outside_content() {
        let image = PinImage {
            png: Arc::new(vec![1]),
            argb32: Arc::new(vec![0xff, 0xff, 0xff, 0xff]),
            width: 1,
            height: 1,
            stride: 4,
        };
        let raster = build_static_raster(&image, (32, 32), 1).unwrap();
        assert_eq!((raster.width, raster.height), (32, 32));
        assert_eq!(
            raster.bytes[3], 0,
            "outer corner must remain click-through chrome"
        );
        let center_alpha = raster.bytes[(16 * 32 + 16) * 4 + 3];
        assert_ne!(center_alpha, 0, "content must remain opaque");
    }
}
