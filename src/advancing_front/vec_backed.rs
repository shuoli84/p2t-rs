use super::*;
use crate::shape::InnerTriangle;
use crate::{points::Points, shape::Point, triangles::TriangleId, PointId};

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
pub struct AdvancingFrontVec {
    nodes: Vec<(PointKey, NodeInner)>,
    /// In my local test, hit rate is about 40%
    access_cache: Option<(PointKey, usize)>,
}

/// New type to wrap `Point` as Node's key
#[derive(Debug, Clone, Copy)]
struct PointKey(Point);

impl PartialEq for PointKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.x.eq(&other.0.x) && self.0.y.eq(&other.0.y)
    }
}

impl Eq for PointKey {}

impl PartialOrd for PointKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.0.x.partial_cmp(&other.0.x) {
            None | Some(Ordering::Equal) => self.0.y.partial_cmp(&other.0.y),
            x_order => {
                return x_order;
            }
        }
    }
}

impl Ord for PointKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl From<Point> for PointKey {
    fn from(value: Point) -> Self {
        Self(value)
    }
}

impl PointKey {
    /// clone the point
    fn point(&self) -> Point {
        self.0
    }
}

struct NodeInner {
    point_id: PointId,
    /// last node's triangle is None
    pub triangle: Option<TriangleId>,
}

impl NodeInner {
    fn to_node<'a, 'b>(
        &'a self,
        index: usize,
        point: Point,
        advancing_front: &'b AdvancingFront,
    ) -> NodeRef<'b> {
        NodeRef {
            point_id: self.point_id,
            point,
            triangle: self.triangle,
            index,
            advancing_front,
        }
    }
}

impl AdvancingFrontVec {
    /// Create a new advancing front with the initial triangle
    /// Triangle's point order: P0, P-1, P-2
    pub fn new(triangle: &InnerTriangle, triangle_id: TriangleId, points: &Points) -> Self {
        let mut nodes = Vec::<(PointKey, NodeInner)>::with_capacity(32);

        let first_point = points
            .get_point(triangle.points[1])
            .expect("should not fail");
        let middle_point = points
            .get_point(triangle.points[0])
            .expect("should not fail");
        let tail_node = points
            .get_point(triangle.points[2])
            .expect("should not fail");

        nodes.push((
            first_point.into(),
            NodeInner {
                point_id: triangle.points[1],
                triangle: Some(triangle_id),
            },
        ));
        nodes.push((
            middle_point.into(),
            NodeInner {
                point_id: triangle.points[0],
                triangle: Some(triangle_id),
            },
        ));
        nodes.push((
            tail_node.into(),
            NodeInner {
                point_id: triangle.points[2],
                triangle: None,
            },
        ));

        nodes.sort_unstable_by_key(|e| e.0);

        Self {
            nodes,
            access_cache: None,
        }
    }

    /// insert a new node for point and triangle
    /// or update the node pointing to new triangle
    pub fn insert(&mut self, point_id: PointId, point: Point, triangle_id: TriangleId) {
        debug_assert!(!triangle_id.invalid());
        let new_node = NodeInner {
            point_id,
            triangle: Some(triangle_id),
        };
        let node_index = match self.search_by_key(&PointKey(point)) {
            Ok(idx) => {
                self.nodes[idx].1 = new_node;
                idx
            }
            Err(idx) => {
                self.nodes.insert(idx, (point.into(), new_node));
                idx
            }
        };
        self.access_cache = Some((PointKey(point), node_index));
    }

    /// delete the node identified by `point`
    pub fn delete(&mut self, point: Point) {
        match self.search_by_key(&PointKey(point)) {
            Ok(idx) => {
                self.nodes.remove(idx);
            }
            Err(_) => {}
        }

        // clear cache
        self.access_cache = None;
    }

    /// delete the node identified by `point`
    pub fn delete_node(&mut self, node: NodeRef) {
        self.nodes.remove(node.index);

        // clear cache
        self.access_cache = None;
    }

    /// Get `n`th node
    pub fn nth(&self, n: usize) -> Option<NodeRef> {
        self.nodes
            .get(n)
            .map(|(k, v)| v.to_node(n, k.point(), self))
    }

    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = NodeRef> + 'a> {
        Box::new(
            self.nodes
                .iter()
                .enumerate()
                .map(|(idx, (p, n))| n.to_node(idx, p.point(), self)),
        )
    }

    /// locate the node containing point
    /// locate the node for `x`
    pub fn locate_node(&self, x: f64) -> Option<NodeRef> {
        let key = PointKey(Point::new(x, f64::MAX));
        let idx = match self.search_by_key(&key) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let point = self.nodes[idx].0.point();
        Some(self.nodes[idx].1.to_node(idx, point, self))
    }

    /// Get the node identified by `point`
    pub fn get_node(&self, point: Point) -> Option<NodeRef> {
        match self.search_by_key(&PointKey(point)) {
            Ok(idx) => Some(self.nodes[idx].1.to_node(idx, point, self)),
            Err(_) => None,
        }
    }

    /// update node's triangle
    pub fn update_triangle(&mut self, point: Point, triangle_id: TriangleId) {
        if let Some((p, i)) = self.access_cache {
            if p.0.eq(&point) {
                self.nodes[i].1.triangle = Some(triangle_id);
                return;
            }
        }
        let idx = self.search_by_key(&PointKey(point)).unwrap();
        self.nodes[idx].1.triangle = Some(triangle_id);

        self.access_cache = Some((PointKey(point), idx));
    }

    /// Get next node of the node identified by `point`
    /// Note: even if the node is deleted, this also returns next node as if it is not deleted
    pub fn locate_next_node(&self, point: Point) -> Option<NodeRef> {
        let idx = match self.search_by_key(&PointKey(point)) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        };
        if idx < self.nodes.len() {
            Some(
                self.nodes[idx]
                    .1
                    .to_node(idx, self.nodes[idx].0.point(), self),
            )
        } else {
            None
        }
    }

    /// Get next node of the node identified by `point`
    /// Note: even if the node is deleted, this also returns next node as if it is not deleted
    pub(super) fn next_node(&self, node: &NodeRef) -> Option<NodeRef> {
        let idx = node.index + 1;
        if idx < self.nodes.len() {
            Some(
                self.nodes[idx]
                    .1
                    .to_node(idx, self.nodes[idx].0.point(), self),
            )
        } else {
            None
        }
    }

    /// Get prev node of the node identified by `point`
    /// Note: even if the node is deleted, then this returns prev node as if it is not deleted
    pub fn locate_prev_node(&self, point: Point) -> Option<NodeRef> {
        let idx = match self.search_by_key(&PointKey(point)) {
            Ok(idx) | Err(idx) if idx > 0 => idx - 1,
            _ => return None,
        };
        Some(
            self.nodes[idx]
                .1
                .to_node(idx, self.nodes[idx].0.point(), self),
        )
    }

    /// Get prev node of the node identified by `point`
    /// Note: even if the node is deleted, then this returns prev node as if it is not deleted
    pub(super) fn prev_node(&self, node: &NodeRef) -> Option<NodeRef> {
        if node.index == 0 {
            return None;
        }

        let index = node.index - 1;
        Some(
            self.nodes[index]
                .1
                .to_node(index, self.nodes[index].0.point(), self),
        )
    }

    fn search_by_key(&self, key: &PointKey) -> Result<usize, usize> {
        self.nodes.binary_search_by_key(key, |e| e.0)
    }
}
