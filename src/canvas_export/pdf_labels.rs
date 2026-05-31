use crate::config::{
    PdfLabelConfig, PdfLabelContentMode, PdfLabelPosition, validate_pdf_label_template,
};

use super::pdf::PdfPageMetadata;

const ELLIPSIS: &str = "…";

pub(crate) fn render_pdf_label(
    ctx: &cairo::Context,
    config: &PdfLabelConfig,
    metadata: &PdfPageMetadata,
    page_width: f64,
    page_height: f64,
) {
    if !config.enabled {
        return;
    }
    let available_width = page_width - config.margin * 2.0 - config.padding_x * 2.0;
    if available_width <= config.font_size {
        return;
    }

    let Ok(text) = pdf_label_text(config, metadata) else {
        return;
    };
    let text = text.replace(['\n', '\r'], " ");
    if text.trim().is_empty() {
        return;
    }

    ctx.select_font_face(
        &config.font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(config.font_size);

    let Some(text) = ellipsize_to_width(ctx, &text, available_width) else {
        return;
    };
    let Ok(extents) = ctx.text_extents(&text) else {
        return;
    };
    if extents.width() <= 0.0 || extents.height() <= 0.0 {
        return;
    }

    let box_width = extents.width() + config.padding_x * 2.0;
    let box_height = extents.height() + config.padding_y * 2.0;
    if box_width > page_width || box_height > page_height {
        return;
    }

    let Some((box_x, box_y)) = pdf_label_box_origin(
        config.position,
        page_width,
        page_height,
        box_width,
        box_height,
        config.margin,
    ) else {
        return;
    };

    let _ = ctx.save();
    if config.background_enabled {
        let color = config.background_color;
        ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
        ctx.rectangle(box_x, box_y, box_width, box_height);
        let _ = ctx.fill();
    }

    let text_x = box_x + config.padding_x - extents.x_bearing();
    let text_y = box_y + config.padding_y - extents.y_bearing();
    let color = config.text_color;
    ctx.set_source_rgba(color[0], color[1], color[2], color[3]);
    ctx.move_to(text_x, text_y);
    let _ = ctx.show_text(&text);
    let _ = ctx.restore();
}

fn pdf_label_text(config: &PdfLabelConfig, metadata: &PdfPageMetadata) -> Result<String, String> {
    match config.content {
        PdfLabelContentMode::CustomTemplate => {
            format_pdf_label_template(&config.template, metadata)
        }
        PdfLabelContentMode::BoardAndPage => Ok(format!(
            "{} - {} ({}/{})",
            metadata.board_name,
            metadata.page_name_label,
            metadata.document_page_label,
            metadata.document_page_count_label
        )),
        PdfLabelContentMode::DocumentPage => Ok(format!(
            "{}/{}",
            metadata.document_page_label, metadata.document_page_count_label
        )),
        PdfLabelContentMode::BoardName => Ok(metadata.board_name.clone()),
        PdfLabelContentMode::PageName => Ok(metadata.page_name_label.clone()),
    }
}

fn pdf_label_box_origin(
    position: PdfLabelPosition,
    page_width: f64,
    page_height: f64,
    box_width: f64,
    box_height: f64,
    margin: f64,
) -> Option<(f64, f64)> {
    let (x, y) = match position {
        PdfLabelPosition::TopLeft => (margin, margin),
        PdfLabelPosition::TopRight => (page_width - margin - box_width, margin),
        PdfLabelPosition::BottomLeft => (margin, page_height - margin - box_height),
        PdfLabelPosition::BottomRight => (
            page_width - margin - box_width,
            page_height - margin - box_height,
        ),
        PdfLabelPosition::BottomCenter => (
            (page_width - box_width) / 2.0,
            page_height - margin - box_height,
        ),
    };

    (x >= 0.0 && y >= 0.0 && x + box_width <= page_width && y + box_height <= page_height)
        .then_some((x, y))
}

pub(crate) fn format_pdf_label_template(
    template: &str,
    metadata: &PdfPageMetadata,
) -> Result<String, String> {
    validate_pdf_label_template(template)?;
    let mut out = String::with_capacity(template.len() + 16);
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                out.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                out.push('}');
            }
            '{' => {
                let mut name = String::new();
                for next in chars.by_ref() {
                    if next == '}' {
                        break;
                    }
                    name.push(next);
                }
                out.push_str(label_value(&name, metadata).unwrap_or_default());
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

fn label_value<'a>(name: &str, metadata: &'a PdfPageMetadata) -> Option<&'a str> {
    match name {
        "app_board" => Some(metadata.app_board_label.as_str()),
        "app_boards" => Some(metadata.app_board_count_label.as_str()),
        "export_board" => Some(metadata.export_board_label.as_str()),
        "export_boards" => Some(metadata.export_board_count_label.as_str()),
        "page" => Some(metadata.board_page_label.as_str()),
        "pages" => Some(metadata.board_page_count_label.as_str()),
        "document_page" => Some(metadata.document_page_label.as_str()),
        "document_pages" => Some(metadata.document_page_count_label.as_str()),
        "board_name" => Some(metadata.board_name.as_str()),
        "page_name" => Some(metadata.page_name_label.as_str()),
        _ => None,
    }
}

fn ellipsize_to_width(ctx: &cairo::Context, text: &str, max_width: f64) -> Option<String> {
    if ctx.text_extents(text).ok()?.width() <= max_width {
        return Some(text.to_string());
    }
    let ellipsis_width = ctx.text_extents(ELLIPSIS).ok()?.width();
    if ellipsis_width > max_width {
        return None;
    }

    let chars: Vec<char> = text.chars().collect();
    let mut lo = 0usize;
    let mut hi = chars.len();
    let mut best = None;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let candidate = chars
            .iter()
            .take(mid)
            .copied()
            .chain(ELLIPSIS.chars())
            .collect::<String>();
        let width = ctx.text_extents(&candidate).ok()?.width();
        if width <= max_width {
            best = Some(candidate);
            lo = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            hi = mid - 1;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_export::pdf::PdfPageMetadata;

    fn metadata() -> PdfPageMetadata {
        PdfPageMetadata::new(
            0,
            2,
            1,
            1,
            0,
            3,
            4,
            12,
            "Board".to_string(),
            Some("Intro".to_string()),
        )
    }

    #[test]
    fn label_template_formats_metadata_and_escaped_braces() {
        let text = format_pdf_label_template(
            "{{{board_name}}} {page_name} {document_page}/{document_pages}",
            &metadata(),
        )
        .expect("format");

        assert_eq!(text, "{Board} Intro 5/12");
    }

    #[test]
    fn label_template_rejects_unknown_placeholders() {
        let err = format_pdf_label_template("{missing}", &metadata()).expect_err("invalid");
        assert!(err.contains("Unknown"));
    }

    #[test]
    fn label_content_modes_format_without_custom_template() {
        let config = PdfLabelConfig {
            content: PdfLabelContentMode::DocumentPage,
            template: "{missing}".to_string(),
            ..PdfLabelConfig::default()
        };

        let text = pdf_label_text(&config, &metadata()).expect("label text");

        assert_eq!(text, "5/12");
    }

    #[test]
    fn label_box_origin_supports_corner_positions() {
        assert_eq!(
            pdf_label_box_origin(PdfLabelPosition::TopLeft, 200.0, 100.0, 50.0, 20.0, 10.0),
            Some((10.0, 10.0))
        );
        assert_eq!(
            pdf_label_box_origin(
                PdfLabelPosition::BottomRight,
                200.0,
                100.0,
                50.0,
                20.0,
                10.0
            ),
            Some((140.0, 70.0))
        );
    }
}
