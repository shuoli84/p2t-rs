use super::*;
use crate::{points::Points, shape::Point, triangles::TriangleId, PointId, Triangle};
use std::collections::BTreeMap;

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
pub struct AdvancingFrontBTree {
    nodes: BTreeMap<PointKey, Node>,
}

impl AdvancingFrontBTree {
    /// Create a new advancing front with the initial triangle
    /// Triangle's point order: P0, P-1, P-2
    pub fn new(triangle: &Triangle, triangle_id: TriangleId, points: &Points) -> Self {
        let mut nodes = BTreeMap::<PointKey, Node>::new();

        let first_point = points
            .get_point(triangle.points[1])
            .expect("should not fail");
        let middle_point = points
            .get_point(triangle.points[0])
            .expect("should not fail");
        let tail_node = points
            .get_point(triangle.points[2])
            .expect("should not fail");

        nodes.insert(
            first_point.into(),
            Node {
                point_id: triangle.points[1],
                triangle: Some(triangle_id),
            },
        );
        nodes.insert(
            middle_point.into(),
            Node {
                point_id: triangle.points[0],
                triangle: Some(triangle_id),
            },
        );
        nodes.insert(
            tail_node.into(),
            Node {
                point_id: triangle.points[2],
                triangle: None,
            },
        );

        Self { nodes }
    }

    /// insert a new node for point and triangle
    /// or update the node pointing to new triangle
    pub fn insert(&mut self, point_id: PointId, point: Point, triangle_id: TriangleId) {
        assert!(!triangle_id.invalid());
        self.nodes.insert(
            point.into(),
            Node {
                point_id,
                triangle: Some(triangle_id),
            },
        );
    }

    /// delete the node identified by `point`
    pub fn delete(&mut self, point: Point) {
        self.nodes.remove(&PointKey(point));
    }

    /// Get `n`th node
    pub fn nth(&self, n: usize) -> Option<(Point, &Node)> {
        self.nodes.iter().nth(n).map(|(k, v)| (k.point(), v))
    }

    pub fn iter<'a>(&'a self) -> Box<dyn Iterator<Item = (Point, &Node)> + 'a> {
        Box::new(self.nodes.iter().map(|(p, n)| (p.point(), n)))
    }

    /// locate the node containing point
    /// locate the node for `x`
    pub fn locate_node(&self, x: f64) -> Option<(Point, &Node)> {
        let key = PointKey(Point::new(x, f64::MAX));
        let mut iter = self.nodes.range(..&key).rev();
        let node = iter.next()?;
        Some((node.0.point(), node.1))
    }

    /// Get the node identified by `point`
    pub fn get_node(&self, point: Point) -> Option<&Node> {
        self.nodes.get(&PointKey(point))
    }

    /// Get a mut reference to the node identified by `point`
    pub fn get_node_mut(&mut self, point: Point) -> Option<&mut Node> {
        self.nodes.get_mut(&PointKey(point))
    }

    /// Get next node of the node identified by `point`
    /// Note: even if the node is deleted, this also returns next node as if it is not deleted
    pub fn next_node(&self, point: Point) -> Option<(Point, &Node)> {
        let key = PointKey(point);
        self.nodes
            .range(key..)
            .skip_while(|(p, _)| **p == key)
            .map(|(p, v)| (p.point(), v))
            .next()
    }

    /// Get prev node of the node identified by `point`
    /// Note: even if the node is deleted, then this returns prev node as if it is not deleted
    pub fn prev_node(&self, point: Point) -> Option<(Point, &Node)> {
        self.nodes
            .range(..PointKey(point))
            .rev()
            .nth(0)
            .map(|(p, v)| (p.point(), v))
    }
}
