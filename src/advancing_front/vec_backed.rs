use super::*;
use crate::shape::InnerTriangle;
use crate::{points::Points, shape::Point, triangles::TriangleId, PointId};

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
pub struct AdvancingFrontVec {
    // nodes: BTreeMap<PointKey, Node>,
    nodes: Vec<(PointKey, Node)>,
}

impl AdvancingFrontVec {
    /// Create a new advancing front with the initial triangle
    /// Triangle's point order: P0, P-1, P-2
    pub fn new(triangle: &InnerTriangle, triangle_id: TriangleId, points: &Points) -> Self {
        let mut nodes = Vec::<(PointKey, Node)>::with_capacity(32);

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
            Node {
                point_id: triangle.points[1],
                triangle: Some(triangle_id),
            },
        ));
        nodes.push((
            middle_point.into(),
            Node {
                point_id: triangle.points[0],
                triangle: Some(triangle_id),
            },
        ));
        nodes.push((
            tail_node.into(),
            Node {
                point_id: triangle.points[2],
                triangle: None,
            },
        ));

        nodes.sort_unstable_by_key(|e| e.0);

        Self { nodes }
    }

    /// insert a new node for point and triangle
    /// or update the node pointing to new triangle
    pub fn insert(&mut self, point_id: PointId, point: Point, triangle_id: TriangleId) {
        debug_assert!(!triangle_id.invalid());
        match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => {
                self.nodes[idx].1 = Node {
                    point_id,
                    triangle: Some(triangle_id),
                };
            }
            Err(idx) => {
                self.nodes.insert(
                    idx,
                    (
                        point.into(),
                        Node {
                            point_id,
                            triangle: Some(triangle_id),
                        },
                    ),
                );
            }
        }
    }

    /// delete the node identified by `point`
    pub fn delete(&mut self, point: Point) {
        match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => {
                self.nodes.remove(idx);
            }
            Err(_) => {}
        }
    }

    /// Get `n`th node
    pub fn nth(&self, n: usize) -> Option<(Point, &Node)> {
        self.nodes.get(n).map(|(k, v)| (k.point(), v))
    }

    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (Point, &Node)> + 'a> {
        Box::new(self.nodes.iter().map(|(p, n)| (p.point(), n)))
    }

    /// locate the node containing point
    /// locate the node for `x`
    pub fn locate_node(&self, x: f64) -> Option<(Point, &Node)> {
        let key = PointKey(Point::new(x, f64::MAX));
        let idx = match self.nodes.binary_search_by_key(&key, |e| e.0) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        Some((self.nodes[idx].0.point(), &self.nodes[idx].1))
    }

    /// locate the node containing point
    /// locate the node for `x`
    pub fn locate_node_and_next(&self, x: f64) -> (Option<(Point, &Node)>, Option<(Point, &Node)>) {
        let key = PointKey(Point::new(x, f64::MAX));
        let idx = match self.nodes.binary_search_by_key(&key, |e| e.0) {
            Ok(idx) => idx,
            Err(idx) => idx - 1,
        };
        let node = Some((self.nodes[idx].0.point(), &self.nodes[idx].1));
        let next = if idx + 1 < self.nodes.len() {
            Some((self.nodes[idx + 1].0.point(), &self.nodes[idx + 1].1))
        } else {
            None
        };

        (node, next)
    }

    /// Get the node identified by `point`
    pub fn get_node(&self, point: Point) -> Option<&Node> {
        match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => Some(&self.nodes[idx].1),
            Err(_) => None,
        }
    }

    /// Get a mut reference to the node identified by `point`
    pub fn get_node_mut(&mut self, point: Point) -> Option<&mut Node> {
        match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => Some(&mut self.nodes[idx].1),
            Err(_) => None,
        }
    }

    /// Get next node of the node identified by `point`
    /// Note: even if the node is deleted, this also returns next node as if it is not deleted
    pub fn next_node(&self, point: Point) -> Option<(Point, &Node)> {
        let idx = match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        };
        if idx < self.nodes.len() {
            Some((self.nodes[idx].0.point(), &self.nodes[idx].1))
        } else {
            None
        }
    }

    /// Get prev node of the node identified by `point`
    /// Note: even if the node is deleted, then this returns prev node as if it is not deleted
    pub fn prev_node(&self, point: Point) -> Option<(Point, &Node)> {
        let idx = match self.nodes.binary_search_by_key(&PointKey(point), |e| e.0) {
            Ok(idx) | Err(idx) if idx > 0 => idx - 1,
            _ => return None,
        };
        if idx < self.nodes.len() {
            Some((self.nodes[idx].0.point(), &self.nodes[idx].1))
        } else {
            None
        }
    }
}
