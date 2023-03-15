use std::{cmp::Ordering, marker::PhantomData};

use crate::{triangles::TriangleId, Point, PointId};

mod vec_backed;
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
pub struct Node<'a> {
    point_id: PointId,
    /// last node's triangle is None
    pub triangle: Option<TriangleId>,
    /// current index, used to optimize retrieve prev, next etc
    index: usize,

    _priv: PhantomData<&'a str>,
}

impl Node<'_> {
    pub fn point_id(&self) -> PointId {
        self.point_id
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
            let point = p.0;
            assert_eq!(point.x, 0.0);
            assert_eq!(point.y, 3.0);

            let p = advancing_front.locate_node(0.3).unwrap();
            let point = p.0;
            assert_eq!(point.x, 0.0);
            assert_eq!(point.y, 3.0);

            let prev_node = advancing_front.locate_prev_node(point).unwrap();
            assert_eq!(prev_node.0.x, -1.);

            let next_node = advancing_front.locate_next_node(point).unwrap();
            assert_eq!(next_node.0.x, 1.);

            assert_eq!(
                advancing_front
                    .locate_prev_node(Point::new(-0.5, 0.))
                    .unwrap()
                    .0
                    .x,
                -1.
            );

            assert_eq!(
                advancing_front
                    .locate_next_node(Point::new(-0.5, 0.))
                    .unwrap()
                    .0
                    .x,
                0.
            );

            advancing_front.delete(Point::new(0., 3.));
            assert!(advancing_front.get_node(Point::new(0., 3.)).is_none());
        }
    }
}
