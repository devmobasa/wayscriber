//! The widget tree: one flat, z-ordered list of nodes per bar.

#[cfg(test)]
use crate::backend::wayland::toolbar::events::HitKind;
use crate::backend::wayland::toolbar::hit::HitRegion;
#[cfg(test)]
use crate::backend::wayland::toolbar::hit::rect_contains_with_min_target;

use super::node::{WidgetId, WidgetNode};

/// A built view: nodes in paint order (background first) plus the logical
/// size of the surface that contains them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WidgetTree {
    nodes: Vec<WidgetNode>,
    size: (f64, f64),
}

impl WidgetTree {
    pub fn new(size: (f64, f64)) -> Self {
        Self {
            nodes: Vec::new(),
            size,
        }
    }

    pub fn push(&mut self, node: WidgetNode) {
        self.nodes.push(node);
    }

    pub fn nodes(&self) -> &[WidgetNode] {
        &self.nodes
    }

    #[cfg(test)]
    pub fn size(&self) -> (f64, f64) {
        self.size
    }

    pub fn node_by_id(&self, id: &WidgetId) -> Option<&WidgetNode> {
        self.nodes.iter().find(|node| &node.id == id)
    }

    /// Topmost interactive node at a logical point. Nodes are stored in
    /// paint order, so the scan runs back-to-front; compact targets are
    /// inflated with the same predicate the legacy hit path uses.
    /// Production consumes the tree through [`Self::to_hit_regions`]; the
    /// invariant tests below pin this native path to the same semantics.
    #[cfg(test)]
    pub fn hit(&self, x: f64, y: f64) -> Option<&WidgetNode> {
        self.nodes
            .iter()
            .rev()
            .filter(|node| node.interact.is_some())
            .find(|node| rect_contains_with_min_target(node.rect, x, y))
    }

    /// Ids of keyboard-focusable nodes (click interactions), in paint order.
    #[cfg(test)]
    pub fn focusable_ids(&self) -> impl Iterator<Item = &WidgetId> {
        self.nodes
            .iter()
            .filter(|node| {
                node.interact
                    .as_ref()
                    .is_some_and(|interact| matches!(interact.kind, HitKind::Click))
            })
            .map(|node| &node.id)
    }

    /// Next focusable id after `current` (wrapping), or the first/last
    /// focusable when nothing is focused yet.
    #[cfg(test)]
    pub fn next_focus(&self, current: Option<&WidgetId>, reverse: bool) -> Option<WidgetId> {
        let ids: Vec<&WidgetId> = self.focusable_ids().collect();
        if ids.is_empty() {
            return None;
        }
        let pos = current.and_then(|id| ids.iter().position(|entry| *entry == id));
        let next = match (pos, reverse) {
            (Some(pos), false) => (pos + 1) % ids.len(),
            (Some(pos), true) => (pos + ids.len() - 1) % ids.len(),
            (None, false) => 0,
            (None, true) => ids.len() - 1,
        };
        ids.get(next).map(|id| (*id).clone())
    }

    /// Tight bounding box over all nodes: (x, y, w, h). Falls back to the
    /// declared size when the tree is empty.
    #[cfg(test)]
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        if self.nodes.is_empty() {
            return (0.0, 0.0, self.size.0, self.size.1);
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for node in &self.nodes {
            let (x, y, w, h) = node.rect;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
        }
        (min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// Transitional adapter: interactive nodes as legacy [`HitRegion`]s.
    ///
    /// Emitted topmost-first because the legacy consumers resolve overlaps
    /// with first-match (`find_map`), while the tree resolves them with
    /// topmost-wins — this ordering makes both agree.
    pub fn to_hit_regions(&self) -> Vec<HitRegion> {
        self.nodes
            .iter()
            .rev()
            .filter_map(|node| {
                node.interact.as_ref().map(|interact| HitRegion {
                    focus_id: Some(node.id.as_str().to_string()),
                    rect: node.rect,
                    event: interact.event.clone(),
                    kind: interact.kind.clone(),
                    tooltip: interact.tooltip.clone(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::node::{ButtonStyle, Interaction, LabelSpec, WidgetKind, WidgetNode};
    use super::*;
    use crate::backend::wayland::toolbar::hit::next_focus_index;
    use crate::ui::toolbar::ToolbarEvent;

    fn button(id: &'static str, rect: (f64, f64, f64, f64), event: ToolbarEvent) -> WidgetNode {
        WidgetNode::new(
            id,
            rect,
            WidgetKind::TextButton {
                label: LabelSpec::new(id, 13.0, false),
                style: ButtonStyle::plain(),
            },
            Some(Interaction::click(event, None)),
        )
    }

    fn sample_tree() -> WidgetTree {
        let mut tree = WidgetTree::new((200.0, 100.0));
        tree.push(WidgetNode::decor(
            "panel",
            (0.0, 0.0, 200.0, 100.0),
            WidgetKind::Panel,
        ));
        tree.push(button("undo", (10.0, 10.0, 40.0, 40.0), ToolbarEvent::Undo));
        tree.push(button("redo", (60.0, 10.0, 40.0, 40.0), ToolbarEvent::Redo));
        tree
    }

    #[test]
    fn hit_ignores_decoration_and_finds_topmost() {
        let mut tree = sample_tree();
        // A node drawn later overlapping "redo" must win the hit.
        tree.push(button("top", (60.0, 10.0, 40.0, 40.0), ToolbarEvent::Undo));

        assert_eq!(tree.hit(30.0, 30.0).unwrap().id.as_str(), "undo");
        assert_eq!(tree.hit(80.0, 30.0).unwrap().id.as_str(), "top");
        assert!(tree.hit(150.0, 80.0).is_none(), "panel is decoration");
    }

    #[test]
    fn hit_inflates_compact_targets_like_the_legacy_path() {
        let mut tree = WidgetTree::new((100.0, 100.0));
        tree.push(button("tiny", (50.0, 50.0, 14.0, 14.0), ToolbarEvent::Undo));

        // 14x14 inflates by 5px on each side to reach 24x24.
        assert!(tree.hit(46.0, 46.0).is_some());
        assert!(tree.hit(68.0, 68.0).is_some());
        assert!(tree.hit(44.0, 57.0).is_none());
    }

    #[test]
    fn focus_traversal_wraps_in_both_directions() {
        let tree = sample_tree();
        let undo = WidgetId::from("undo");
        let redo = WidgetId::from("redo");

        assert_eq!(tree.next_focus(None, false), Some(undo.clone()));
        assert_eq!(tree.next_focus(Some(&undo), false), Some(redo.clone()));
        assert_eq!(tree.next_focus(Some(&redo), false), Some(undo.clone()));
        assert_eq!(tree.next_focus(None, true), Some(redo.clone()));
        assert_eq!(tree.next_focus(Some(&undo), true), Some(redo));
    }

    #[test]
    fn focus_survives_rebuild_by_id_not_index() {
        let tree = sample_tree();
        let focused = WidgetId::from("redo");

        // Rebuild with an extra button inserted before the focused one.
        let mut rebuilt = WidgetTree::new((200.0, 100.0));
        rebuilt.push(button(
            "clear",
            (0.0, 60.0, 40.0, 30.0),
            ToolbarEvent::ClearCanvas { instant: false },
        ));
        for node in tree.nodes() {
            rebuilt.push(node.clone());
        }

        assert!(rebuilt.node_by_id(&focused).is_some());
        assert_eq!(
            rebuilt.next_focus(Some(&focused), false),
            Some(WidgetId::from("clear"))
        );
    }

    #[test]
    fn hit_region_adapter_emits_topmost_first_and_skips_decoration() {
        let tree = sample_tree();
        let regions = tree.to_hit_regions();

        assert_eq!(regions.len(), 2);
        assert!(matches!(regions[0].event, ToolbarEvent::Redo));
        assert!(matches!(regions[1].event, ToolbarEvent::Undo));

        // First-match over the adapter output equals topmost-wins on the tree.
        let hit = regions
            .iter()
            .find(|region| region.contains(80.0, 30.0))
            .unwrap();
        assert!(matches!(hit.event, ToolbarEvent::Redo));
    }

    #[test]
    fn production_adapter_tabs_in_visual_order() {
        let tree = sample_tree();
        let regions = tree.to_hit_regions();

        let first = next_focus_index(&regions, None, false).expect("first focus");
        assert!(matches!(regions[first].event, ToolbarEvent::Undo));
        assert_eq!(regions[first].focus_id.as_deref(), Some("undo"));
        let second = next_focus_index(&regions, Some(first), false).expect("second focus");
        assert!(matches!(regions[second].event, ToolbarEvent::Redo));
    }

    #[test]
    fn bounds_covers_all_nodes() {
        let tree = sample_tree();
        assert_eq!(tree.bounds(), (0.0, 0.0, 200.0, 100.0));

        let empty = WidgetTree::new((30.0, 20.0));
        assert_eq!(empty.bounds(), (0.0, 0.0, 30.0, 20.0));
    }
}
