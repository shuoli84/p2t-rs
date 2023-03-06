use std::collections::BTreeMap;

use ordered_float::OrderedFloat;

use crate::{PointId, Triangle, shape::Point};

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
pub struct AdvancingFront {
    nodes: BTreeMap<OrderedFloat<f64>, Node>
}

pub struct Node {
    /// value is the end x value for fronting edge
    pub value: f64,
    pub point: PointId,
    pub triangle: Option<Triangle>,
}

impl AdvancingFront {
    /// Create a new advancing front with the initial triangle
    /// Triangle's point order: P0, P-1, P-2
    pub fn new(triangle: Triangle, points: &[Point]) -> Self {
        let mut nodes = BTreeMap::<OrderedFloat<f64>, Node>::new();

        let first_node = triangle.get_point_1(points);
        let middle_node = triangle.get_point_0(points);
        let tail_node = triangle.get_point_2(points);

        nodes.insert(first_node.x.into(), Node {
            value: first_node.x,
            point: triangle.points.1,
            triangle: Some(triangle.clone()),
        });
        nodes.insert(middle_node.x.into(), Node {
            value: middle_node.x,
            point: triangle.points.0,
            triangle: Some(triangle),
        });
        nodes.insert(tail_node.x.into(), Node {
            value: tail_node.x,
            point: triangle.points.2,
            triangle: None,
        });

        Self {
            nodes
        }
    }
}