use std::cmp::Ordering;

use crate::{triangles::TriangleId, Point, PointId};

mod vec_backed;
pub use vec_backed::AdvancingFrontVec as AdvancingFront;

pub struct NodeRef<'a> {
    point_id: PointId,
    point: Point,
    /// last node's triangle is None
    pub triangle: Option<TriangleId>,
    /// current index, used to optimize retrieve prev, next etc
    index: usize,

    advancing_front: &'a AdvancingFront,
}

impl NodeRef<'_> {
    pub fn point(&self) -> Point {
        self.point
    }

    pub fn point_id(&self) -> PointId {
        self.point_id
    }

    pub fn next(&self) -> Option<NodeRef> {
        self.advancing_front.next_node(self)
    }

    pub fn prev(&self) -> Option<NodeRef> {
        self.advancing_front.prev_node(self)
    }

    pub(crate) fn index(&self) -> usize {
        self.index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        points::PointsBuilder,
        shape::{InnerTriangle, Point},
        triangles::TriangleStore,
    };

    #[test]
    fn test_advancing_front() {
        let mut triangles = TriangleStore::new();

        let mut points = PointsBuilder::default();
        let p_0 = points.add_point(Point::new(-1., 0.));
        let p_1 = points.add_point(Point::new(0., 3.));
        let p_2 = points.add_point(Point::new(1., 1.));
        let points = points.build();

        let triangle_id = triangles.insert(InnerTriangle::new(p_0, p_1, p_2));
        let triangle = triangles.get(triangle_id).unwrap();

        let mut advancing_front = AdvancingFront::new(triangle, triangle_id, &points);
        {
            let p = advancing_front.locate_node(0.).unwrap();
            let point = p.point();
            assert_eq!(point.x, 0.0);
            assert_eq!(point.y, 3.0);

            let p = advancing_front.locate_node(0.3).unwrap();
            let point = p.point();
            assert_eq!(point.x, 0.0);
            assert_eq!(point.y, 3.0);

            let prev_node = advancing_front.locate_prev_node(point).unwrap();
            assert_eq!(prev_node.point().x, -1.);

            let next_node = advancing_front.locate_next_node(point).unwrap();
            assert_eq!(next_node.point().x, 1.);

            assert_eq!(
                advancing_front
                    .locate_prev_node(Point::new(-0.5, 0.))
                    .unwrap()
                    .point()
                    .x,
                -1.
            );

            assert_eq!(
                advancing_front
                    .locate_next_node(Point::new(-0.5, 0.))
                    .unwrap()
                    .point()
                    .x,
                0.
            );
        }
    }
}
