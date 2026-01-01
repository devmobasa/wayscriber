use super::core::Frame;
use super::frame_storage;
use super::types::{DrawnShape, ShapeId, UndoAction};
use crate::draw::shape::Shape;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl Serialize for DrawnShape {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        PersistedDrawnShape::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DrawnShape {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let helper = PersistedDrawnShape::deserialize(deserializer)?;
        Ok(helper.into())
    }
}

#[derive(Serialize, Deserialize)]
struct PersistedDrawnShape {
    id: ShapeId,
    shape: Shape,
    created_at: u64,
    locked: bool,
}

impl From<&DrawnShape> for PersistedDrawnShape {
    fn from(value: &DrawnShape) -> Self {
        Self {
            id: value.id,
            shape: value.shape.clone(),
            created_at: value.created_at,
            locked: value.locked,
        }
    }
}

impl From<PersistedDrawnShape> for DrawnShape {
    fn from(value: PersistedDrawnShape) -> Self {
        DrawnShape::with_metadata(value.id, value.shape, value.created_at, value.locked)
    }
}

impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct FrameHelper {
            #[serde(with = "frame_storage")]
            shapes: Vec<DrawnShape>,
            #[serde(default)]
            undo_stack: Vec<UndoAction>,
            #[serde(default)]
            redo_stack: Vec<UndoAction>,
        }

        let helper = FrameHelper::deserialize(deserializer)?;
        let mut frame = Frame {
            shapes: helper.shapes,
            undo_stack: helper.undo_stack,
            redo_stack: helper.redo_stack,
            next_shape_id: 1,
        };
        frame.rebuild_next_id();
        Ok(frame)
    }
}
