use std::{cmp::Ordering, collections::BTreeMap};

use crate::{points::Points, shape::Point, triangles::TriangleId, PointId, Triangle};

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
pub struct AdvancingFront {
    nodes: BTreeMap<PointKey, Node>,
}

impl std::fmt::Debug for AdvancingFront {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<AdvancingFront ")?;
        for (p, _) in &self.nodes {
            write!(f, "({}, {}) ", p.0.x, p.0.y)?;
        }
        write!(f, ">")
    }
}

#[derive(Debug)]
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
    pub triangle: Option<TriangleId>,
}

impl AdvancingFront {
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
    pub fn insert(&mut self, point_id: PointId, point: Point, triangle_id: TriangleId) {
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

    pub fn iter(&self) -> impl Iterator<Item = (Point, &Node)> {
        self.nodes.iter().map(|(p, n)| (p.point(), n))
    }
}

impl AdvancingFront {
    /// locate the node containing point
    pub fn locate_point_mut(&mut self, point: Point) -> Option<&mut Node> {
        let key = PointKey(point);
        self.nodes.get_mut(&key)
    }
}

impl AdvancingFront {
    pub fn locate_node(&self, x: f64) -> Option<(Point, &Node)> {
        let key = PointKey(Point::new(x, f64::MAX));
        let mut iter = self.nodes.range(..&key).rev();
        let p1 = iter.next()?;
        if p1.0 .0.x.eq(&x) {
            return Some((p1.0.point(), p1.1));
        } else {
            return Some((p1.0.point(), p1.1));
        }
    }

    /// get the node identified by `point`
    pub fn get_node(&self, point: Point) -> Option<&Node> {
        self.nodes.get(&PointKey(point))
    }

    /// Get next node of the node identified by `point`
    pub fn next_node(&self, point: Point) -> Option<(Point, &Node)> {
        self.nodes
            .range(PointKey(point)..)
            .nth(1)
            .map(|(p, v)| (p.point(), v))
    }

    /// Get prev node of the node identified by `point`
    pub fn prev_node(&self, point: Point) -> Option<(Point, &Node)> {
        self.nodes
            .range(..PointKey(point))
            .rev()
            .nth(0)
            .map(|(p, v)| (p.point(), v))
    }
}

#[cfg(test)]
mod tests {
    use crate::{shape::Point, triangles::Triangles};

    use super::*;

    #[test]
    fn test_advancing_front() {
        let mut points = Points::new(vec![]);
        let mut triangles = Triangles::new();

        let p_0 = points.add_point(Point::new(-1., 0.));
        let p_1 = points.add_point(Point::new(0., 3.));
        let p_2 = points.add_point(Point::new(1., 1.));
        let triangle_id = triangles.insert(Triangle::new(p_0, p_1, p_2));
        let triangle = triangles.get(triangle_id).unwrap();

        let advancing_front = AdvancingFront::new(triangle, triangle_id, &points);
        {
            let p = advancing_front.locate_node(0.).unwrap();
            let point = p.0;
            assert_eq!(point.x, 0.0);
            assert_eq!(point.y, 3.0);

            let prev_node = advancing_front.prev_node(point).unwrap();
            assert_eq!(prev_node.0.x, -1.);

            let next_node = advancing_front.next_node(point).unwrap();
            assert_eq!(next_node.0.x, 1.);
        }
    }
}
