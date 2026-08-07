#[derive(Debug, Clone, Copy)]
pub(in crate::backend::wayland::toolbar) struct GridItem {
    pub(in crate::backend::wayland::toolbar) x: f64,
    pub(in crate::backend::wayland::toolbar) y: f64,
    pub(in crate::backend::wayland::toolbar) w: f64,
    pub(in crate::backend::wayland::toolbar) h: f64,
}

#[derive(Debug, Clone)]
pub(in crate::backend::wayland::toolbar) struct GridLayout {
    pub(in crate::backend::wayland::toolbar) items: Vec<GridItem>,
    pub(in crate::backend::wayland::toolbar) height: f64,
}

pub(in crate::backend::wayland::toolbar) fn row_item_width(
    content_width: f64,
    columns: usize,
    gap: f64,
) -> f64 {
    if columns == 0 {
        return 0.0;
    }
    (content_width - gap * (columns as f64 - 1.0)) / columns as f64
}

#[allow(clippy::too_many_arguments)]
pub(in crate::backend::wayland::toolbar) fn grid_layout(
    start_x: f64,
    start_y: f64,
    item_w: f64,
    item_h: f64,
    col_gap: f64,
    row_gap: f64,
    columns: usize,
    total_items: usize,
) -> GridLayout {
    if columns == 0 || total_items == 0 {
        return GridLayout {
            items: Vec::new(),
            height: 0.0,
        };
    }

    let rows = total_items.div_ceil(columns);
    let mut items = Vec::with_capacity(total_items);
    for index in 0..total_items {
        let row = index / columns;
        let col = index % columns;
        items.push(GridItem {
            x: start_x + (item_w + col_gap) * col as f64,
            y: start_y + (item_h + row_gap) * row as f64,
            w: item_w,
            h: item_h,
        });
    }

    let height = if rows > 0 {
        item_h * rows as f64 + row_gap * (rows as f64 - 1.0)
    } else {
        0.0
    };

    GridLayout { items, height }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_layout_reports_rows_and_height() {
        let layout = grid_layout(10.0, 5.0, 20.0, 10.0, 2.0, 3.0, 3, 7);

        assert_eq!(layout.items.len(), 7);
        assert!((layout.height - 36.0).abs() < 1e-6);
    }
}
