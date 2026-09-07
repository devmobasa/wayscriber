use super::DrawnShape;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_CONTENT_REVISION: AtomicU64 = AtomicU64::new(1);

fn fresh_revision() -> u64 {
    NEXT_CONTENT_REVISION.fetch_add(1, Ordering::Relaxed)
}

/// Shape storage whose mutable borrows invalidate visible-content caches.
///
/// Clones initially share a content token; every subsequent mutable borrow gets
/// a globally distinct token, including undo, previews, rollback, and direct
/// index/iterator edits. Read-only access does not advance it. The token is never
/// serialized, and replacing this collection carries the replacement's token.
#[derive(Debug, Clone)]
pub struct FrameShapes {
    values: Vec<DrawnShape>,
    revision: u64,
}

impl Default for FrameShapes {
    fn default() -> Self {
        Vec::new().into()
    }
}

impl FrameShapes {
    pub(super) fn revision(&self) -> u64 {
        self.revision
    }
}

impl From<Vec<DrawnShape>> for FrameShapes {
    fn from(values: Vec<DrawnShape>) -> Self {
        Self {
            values,
            revision: fresh_revision(),
        }
    }
}

impl FromIterator<DrawnShape> for FrameShapes {
    fn from_iter<T: IntoIterator<Item = DrawnShape>>(iter: T) -> Self {
        iter.into_iter().collect::<Vec<_>>().into()
    }
}

impl Deref for FrameShapes {
    type Target = Vec<DrawnShape>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl DerefMut for FrameShapes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.revision = fresh_revision();
        &mut self.values
    }
}

impl<'a> IntoIterator for &'a FrameShapes {
    type Item = &'a DrawnShape;
    type IntoIter = std::slice::Iter<'a, DrawnShape>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

impl<'a> IntoIterator for &'a mut FrameShapes {
    type Item = &'a mut DrawnShape;
    type IntoIter = std::slice::IterMut<'a, DrawnShape>;
    fn into_iter(self) -> Self::IntoIter {
        self.deref_mut().iter_mut()
    }
}

impl IntoIterator for FrameShapes {
    type Item = DrawnShape;
    type IntoIter = std::vec::IntoIter<DrawnShape>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use crate::draw::{Color, Frame, Shape};
    #[test]
    fn content_tokens_cover_mutable_access_and_diverging_clones_without_serializing() {
        let mut frame = Frame::new();
        let id = frame.add_shape(Shape::Line {
            x1: 0,
            y1: 0,
            x2: 20,
            y2: 30,
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            thick: 2.0,
        });
        let original = frame.content_revision();
        assert_eq!(frame.shapes.len(), 1);
        assert!(frame.shape(id).is_some());
        assert_eq!(frame.content_revision(), original);
        let mut copy = frame.clone();
        assert_eq!(copy.content_revision(), original);
        frame.shape_mut(id).unwrap().shape.translate(5, 0);
        copy.shapes[0].shape.translate(10, 0);
        assert_ne!(frame.content_revision(), original);
        assert_ne!(copy.content_revision(), frame.content_revision());
        let before = copy.content_revision();
        for shape in &mut copy.shapes {
            shape.shape.translate(5, 0);
        }
        assert_ne!(copy.content_revision(), before);
        let json = serde_json::to_string(&copy).unwrap();
        assert!(!json.contains("revision"));
        let restored: Frame = serde_json::from_str(&json).unwrap();
        assert_ne!(restored.content_revision(), copy.content_revision());
        assert_eq!(serde_json::to_string(&restored).unwrap(), json);
    }
}
