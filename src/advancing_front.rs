use std::collections::BTreeMap;

use ordered_float::OrderedFloat;

use crate::{PointId, Triangle, points::Points};

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
#[derive(Debug)]
pub struct AdvancingFront {
    nodes: BTreeMap<OrderedFloat<f64>, Node>
}

#[derive(Debug)]
pub struct Node {
    /// value is the end x value for fronting edge
    pub value: f64,
    pub point: PointId,
    pub triangle: Option<Triangle>,
}

impl AdvancingFront {
    /// Create a new advancing front with the initial triangle
    /// Triangle's point order: P0, P-1, P-2
    pub fn new(triangle: Triangle, points: &Points) -> Self {
        let mut nodes = BTreeMap::<OrderedFloat<f64>, Node>::new();

        let first_point = points.get_point(triangle.points.1).expect("should not fail");
        let middle_point = points.get_point(triangle.points.0).expect("should not fail");
        let tail_node = points.get_point(triangle.points.2).expect("should not fail");

        nodes.insert(first_point.x.into(), Node {
            value: first_point.x,
            point: triangle.points.1,
            triangle: Some(triangle.clone()),
        });
        nodes.insert(middle_point.x.into(), Node {
            value: middle_point.x,
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

#[cfg(test)]
mod tests {
    use crate::{points::Points, shape::Point};

    use super::AdvancingFront;

    #[test]
    fn test_advancing_front() {
        // let points = Points::new(vec![
        //     Point::new(0., 1.),
        // ]);
        // let mut advancing_front = AdvancingFront::new(

        // );
    }
}