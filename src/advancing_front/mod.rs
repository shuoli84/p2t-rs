use std::cmp::Ordering;

use crate::{triangles::TriangleId, Point, PointId};

#[cfg(feature = "af_btree")]
mod btree_backed;
#[cfg(not(feature = "af_btree"))]
mod vec_backed;

#[cfg(feature = "af_btree")]
pub use btee_backed::AdvancingFrontBTree as AdvancingFront;
#[cfg(not(feature = "af_btree"))]
pub use vec_backed::AdvancingFrontVec as AdvancingFront;

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

#[derive(Debug)]
pub struct Node {
    pub point_id: PointId,
    /// last node's triangle is None
    pub triangle: Option<TriangleId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        points::Points,
        shape::{Point, Triangle},
        triangles::Triangles,
    };

    #[test]
    fn test_advancing_front() {
        let mut points = Points::new(vec![]);
        let mut triangles = Triangles::new();

        let p_0 = points.add_point(Point::new(-1., 0.));
        let p_1 = points.add_point(Point::new(0., 3.));
        let p_2 = points.add_point(Point::new(1., 1.));
        let triangle_id = triangles.insert(Triangle::new(p_0, p_1, p_2));
        let triangle = triangles.get(triangle_id).unwrap();

        let mut advancing_front = AdvancingFront::new(triangle, triangle_id, &points);
        {
            let p = advancing_front.locate_node(0.).unwrap();
            let point = p.0;
            assert_eq!(point.x, 0.0);
            assert_eq!(point.y, 3.0);

            let p = advancing_front.locate_node(0.3).unwrap();
            let point = p.0;
            assert_eq!(point.x, 0.0);
            assert_eq!(point.y, 3.0);

            let prev_node = advancing_front.prev_node(point).unwrap();
            assert_eq!(prev_node.0.x, -1.);

            let next_node = advancing_front.next_node(point).unwrap();
            assert_eq!(next_node.0.x, 1.);

            assert_eq!(
                advancing_front.prev_node(Point::new(-0.5, 0.)).unwrap().0.x,
                -1.
            );

            assert_eq!(
                advancing_front.next_node(Point::new(-0.5, 0.)).unwrap().0.x,
                0.
            );

            advancing_front.delete(Point::new(0., 3.));
            assert!(advancing_front.get_node(Point::new(0., 3.)).is_none());
        }
    }
}
