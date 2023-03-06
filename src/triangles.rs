use crate::shape::Triangle;

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TriangleId(usize);

impl TriangleId {
    pub const INVALID: TriangleId = TriangleId(usize::MAX);
}

/// Triangle store, store triangles and their neighborhood relations
// Note: For n vetexes, there will around n - 2 triangles, so space complexity is
//       O(n).
#[derive(Debug)]
pub struct Triangles {
    triangles: Vec<Triangle>,
}

impl Triangles {
    pub fn new() -> Self {
        Self { triangles: vec![] }
    }

    /// insert a new triangle
    pub fn insert(&mut self, triangle: Triangle) -> TriangleId {
        let id = TriangleId(self.triangles.len());
        self.triangles.push(triangle);
        id
    }

    pub fn get(&self, id: TriangleId) -> &Triangle {
        unsafe { self.triangles.get_unchecked(id.0) }
    }

    pub fn get_mut(&mut self, id: TriangleId) -> &mut Triangle {
        unsafe { self.triangles.get_unchecked_mut(id.0) }
    }

    /// mark two triangle as neighbor
    pub fn mark_neighbor(&mut self, left: TriangleId, right: TriangleId) {
        let left_triangle = self.get(left).clone();
        let right_triangle = self.get(right).clone();

        if right_triangle.contains_pair((left_triangle.points.1, left_triangle.points.2)) {
            self.get_mut(left).neighbors.0 = right;
        } else if right_triangle.contains_pair((left_triangle.points.0, left_triangle.points.2)) {
            self.get_mut(left).neighbors.1 = right;
        } else if right_triangle.contains_pair((left_triangle.points.0, left_triangle.points.1)) {
            self.get_mut(left).neighbors.2 = right;
        }

        if left_triangle.contains_pair((right_triangle.points.1, right_triangle.points.2)) {
            self.get_mut(right).neighbors.0 = left;
        } else if left_triangle.contains_pair((right_triangle.points.0, right_triangle.points.2)) {
            self.get_mut(right).neighbors.1 = left;
        } else if left_triangle.contains_pair((right_triangle.points.0, right_triangle.points.1)) {
            self.get_mut(right).neighbors.2 = left;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{points::Points, shape::Point};

    use super::*;

    #[test]
    fn test_triangles() {
        let mut triangles = Triangles::new();
        let mut points = Points::new(vec![]);

        let p0 = points.add_point(Point::new(0., 0.));
        let p1 = points.add_point(Point::new(2., 0.));
        let p2 = points.add_point(Point::new(1., 2.));
        let p3 = points.add_point(Point::new(4., 2.));

        let t1 = triangles.insert(Triangle::new(p0, p1, p2));
        let t2 = triangles.insert(Triangle::new(p1, p2, p3));

        triangles.mark_neighbor(t1, t2);
        {
            let t = triangles.get(t1);
            assert_eq!(t.neighbors.0, t2);
            let t = triangles.get(t2);
            assert_eq!(t.neighbors.2, t1);
        }
    }
}
