use super::types::{DrawnShape, ShapeId, current_timestamp_ms};
use crate::draw::shape::Shape;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S>(shapes: &[DrawnShape], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let helper: Vec<SerializedDrawnShape<'_>> = shapes
        .iter()
        .map(|shape| SerializedDrawnShape {
            id: shape.id,
            shape: &shape.shape,
            created_at: shape.created_at,
            locked: shape.locked,
        })
        .collect();
    helper.serialize(serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<DrawnShape>, D::Error>
where
    D: Deserializer<'de>,
{
    let helpers: Vec<ShapeElement> = Vec::deserialize(deserializer)?;
    let mut shapes = Vec::with_capacity(helpers.len());
    let mut next_id: ShapeId = 1;

    for entry in helpers {
        match entry {
            ShapeElement::WithMeta(helper) => {
                let mut id = helper.id.unwrap_or(0);
                if id == 0 {
                    id = next_id;
                }
                let created_at = helper.created_at.unwrap_or_else(current_timestamp_ms);
                let locked = helper.locked.unwrap_or(false);
                shapes.push(DrawnShape::with_metadata(
                    id,
                    helper.shape,
                    created_at,
                    locked,
                ));
                next_id = next_id.max(id.saturating_add(1));
            }
            ShapeElement::Legacy(shape) => {
                let id = next_id;
                shapes.push(DrawnShape::with_metadata(
                    id,
                    shape,
                    current_timestamp_ms(),
                    false,
                ));
                next_id = next_id.saturating_add(1);
            }
        }
    }

    Ok(shapes)
}

#[derive(Serialize)]
struct SerializedDrawnShape<'a> {
    id: ShapeId,
    shape: &'a Shape,
    created_at: u64,
    locked: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ShapeElement {
    WithMeta(WithMeta),
    Legacy(Shape),
}

#[derive(Deserialize)]
struct WithMeta {
    #[serde(default)]
    id: Option<ShapeId>,
    shape: Shape,
    #[serde(default)]
    created_at: Option<u64>,
    #[serde(default)]
    locked: Option<bool>,
}
