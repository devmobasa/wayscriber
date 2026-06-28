use crate::draw::Shape;
use crate::draw::shape::{PolygonTemplate, generated_points, has_minimum_distinct_points};
use crate::input::tool::{Tool, ToolDrawingBehavior};

use super::{
    FinishedToolStroke, PolygonProvisionalSnapshot, PolygonStrokeSnapshot, ProvisionalToolStroke,
    ToolUsage,
};

impl Tool {
    pub(crate) fn polygon_template(self) -> Option<PolygonTemplate> {
        match self.drawing_behavior() {
            ToolDrawingBehavior::Polygon(template) => Some(template),
            _ => None,
        }
    }

    pub(crate) fn finish_polygon_stroke(
        self,
        snapshot: PolygonStrokeSnapshot,
    ) -> FinishedToolStroke {
        debug_assert_eq!(self, snapshot.tool);
        let Some(template) = self.polygon_template() else {
            debug_assert!(false, "non-polygon tool cannot finish a polygon stroke");
            return FinishedToolStroke::Noop;
        };
        finish_polygon(snapshot, ToolUsage::default(), template)
    }

    pub(crate) fn provisional_polygon_stroke(
        self,
        snapshot: PolygonProvisionalSnapshot,
    ) -> ProvisionalToolStroke<'static> {
        debug_assert_eq!(self, snapshot.tool);
        let Some(template) = self.polygon_template() else {
            debug_assert!(false, "non-polygon tool cannot preview a polygon stroke");
            return ProvisionalToolStroke::None;
        };
        provisional_polygon(snapshot, template)
    }
}

fn finish_polygon(
    snapshot: PolygonStrokeSnapshot,
    usage: ToolUsage,
    template: PolygonTemplate,
) -> FinishedToolStroke {
    let points = generated_points(
        template,
        snapshot.start,
        snapshot.end,
        snapshot.regular_sides,
    );
    if !has_minimum_distinct_points(&points) {
        return FinishedToolStroke::Noop;
    }

    FinishedToolStroke::Shape {
        shape: Shape::Polygon {
            kind: template.kind(snapshot.regular_sides),
            points,
            fill: snapshot.fill_enabled,
            color: snapshot.color,
            thick: snapshot.size,
        },
        usage,
    }
}

fn provisional_polygon(
    snapshot: PolygonProvisionalSnapshot,
    template: PolygonTemplate,
) -> ProvisionalToolStroke<'static> {
    let points = generated_points(
        template,
        snapshot.start,
        snapshot.current,
        snapshot.regular_sides,
    );
    ProvisionalToolStroke::Shape(Shape::Polygon {
        kind: template.kind(snapshot.regular_sides),
        points,
        fill: snapshot.fill_enabled,
        color: snapshot.color,
        thick: snapshot.size,
    })
}
