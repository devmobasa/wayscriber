//! Small layout cursors used by tree builders.

// Consumed starting with the top-strip port; the allow is removed then.
#![allow(dead_code)]
/// Left-to-right placement cursor with a uniform gap.
#[derive(Debug, Clone, Copy)]
pub struct RowCursor {
    x: f64,
    gap: f64,
}

impl RowCursor {
    pub fn new(start_x: f64, gap: f64) -> Self {
        Self { x: start_x, gap }
    }

    /// Reserve `width`, returning the x position of the reserved slot and
    /// advancing past it plus the gap.
    pub fn place(&mut self, width: f64) -> f64 {
        let x = self.x;
        self.x += width + self.gap;
        x
    }

    /// Extra spacing beyond the uniform gap (group separation).
    pub fn skip(&mut self, extra: f64) {
        self.x += extra;
    }

    /// Current x (start of the next slot).
    pub fn x(&self) -> f64 {
        self.x
    }

    /// Width consumed so far, ignoring the trailing gap after the last slot.
    pub fn consumed_from(&self, start_x: f64) -> f64 {
        (self.x - start_x - self.gap).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_cursor_places_with_uniform_gaps() {
        let mut row = RowCursor::new(10.0, 5.0);
        assert_eq!(row.place(40.0), 10.0);
        assert_eq!(row.place(40.0), 55.0);
        row.skip(8.0);
        assert_eq!(row.place(20.0), 108.0);
        assert_eq!(row.x(), 133.0);
        assert_eq!(row.consumed_from(10.0), 118.0);
    }
}
