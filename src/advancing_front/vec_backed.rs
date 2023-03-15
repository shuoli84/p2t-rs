use super::*;
use crate::shape::InnerTriangle;
use crate::{points::Points, shape::Point, triangles::TriangleId, PointId};

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
pub struct AdvancingFrontVec {
    nodes: Vec<(PointKey, NodeInner)>,
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
    ) -> Node<'b> {
        Node {
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

        Self { nodes }
    }

    /// insert a new node for point and triangle
    /// or update the node pointing to new triangle
    #[inline(never)]
    pub fn insert(&mut self, point_id: PointId, point: Point, triangle_id: TriangleId) {
        debug_assert!(!triangle_id.invalid());
        match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => {
                self.nodes[idx].1 = NodeInner {
                    point_id,
                    triangle: Some(triangle_id),
                };
            }
            Err(idx) => {
                self.nodes.insert(
                    idx,
                    (
                        point.into(),
                        NodeInner {
                            point_id,
                            triangle: Some(triangle_id),
                        },
                    ),
                );
            }
        }
    }

    /// delete the node identified by `point`
    #[inline(never)]
    pub fn delete(&mut self, point: Point) {
        match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => {
                self.nodes.remove(idx);
            }
            Err(_) => {}
        }
    }

    /// delete the node identified by `point`
    #[inline(never)]
    pub fn delete_node(&mut self, node: Node) {
        self.nodes.remove(node.index);
    }

    /// Get `n`th node
    #[inline(never)]
    pub fn nth(&self, n: usize) -> Option<(Point, Node)> {
        self.nodes
            .get(n)
            .map(|(k, v)| (k.point(), v.to_node(n, k.point(), self)))
    }

    #[inline(never)]
    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (Point, Node)> + 'a> {
        Box::new(
            self.nodes
                .iter()
                .enumerate()
                .map(|(idx, (p, n))| (p.point(), n.to_node(idx, p.point(), self))),
        )
    }

    /// locate the node containing point
    /// locate the node for `x`
    #[inline(never)]
    pub fn locate_node(&self, x: f64) -> Option<Node> {
        let key = PointKey(Point::new(x, f64::MAX));
        let idx = match self.nodes.binary_search_by_key(&key, |e| e.0) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let point = self.nodes[idx].0.point();
        Some(self.nodes[idx].1.to_node(idx, point, self))
    }

    /// Get the node identified by `point`
    #[inline(never)]
    pub fn get_node(&self, point: Point) -> Option<Node> {
        match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => Some(self.nodes[idx].1.to_node(idx, point, self)),
            Err(_) => None,
        }
    }

    /// update node's triangle
    #[inline(never)]
    pub fn update_triangle(&mut self, point: Point, triangle_id: TriangleId) {
        let idx = self
            .nodes
            .binary_search_by_key(&PointKey(point), |e| e.0)
            .unwrap();
        self.nodes[idx].1.triangle = Some(triangle_id);
    }

    /// Get next node of the node identified by `point`
    /// Note: even if the node is deleted, this also returns next node as if it is not deleted
    #[inline(never)]
    pub fn locate_next_node(&self, point: Point) -> Option<Node> {
        let idx = match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
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
    #[inline(never)]
    pub(super) fn next_node(&self, node: &Node) -> Option<Node> {
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
    #[inline(never)]
    pub fn locate_prev_node(&self, point: Point) -> Option<Node> {
        let idx = match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
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
    #[inline(never)]
    pub(super) fn prev_node(&self, node: &Node) -> Option<Node> {
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
}
