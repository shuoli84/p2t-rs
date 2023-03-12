use std::cmp::Ordering;

use crate::{points::Points, shape::Point, triangles::TriangleId, PointId, Triangle};

/// Advancing front, stores all advancing edges in a btree, this makes store compact
/// and easier to update
pub struct AdvancingFront {
    // nodes: BTreeMap<PointKey, Node>,
    nodes: Vec<(PointKey, Node)>,
}

impl AdvancingFront {
    /// Create a new advancing front with the initial triangle
    /// Triangle's point order: P0, P-1, P-2
    pub fn new(triangle: &Triangle, triangle_id: TriangleId, points: &Points) -> Self {
        let mut nodes = Vec::<(PointKey, Node)>::new();

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
        assert!(!triangle_id.invalid());
        match self.get_node_mut(point) {
            Some(node) => {
                *node = Node {
                    point_id,
                    triangle: Some(triangle_id),
                };
            }
            None => {
                self.nodes.push((
                    point.into(),
                    Node {
                        point_id,
                        triangle: Some(triangle_id),
                    },
                ));

                self.nodes.sort_unstable_by_key(|e| e.0);
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

    pub fn iter(&self) -> impl Iterator<Item = (Point, &Node)> {
        self.nodes.iter().map(|(p, n)| (p.point(), n))
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
