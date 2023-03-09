use crate::{shape::Triangle, PointId};

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TriangleId(usize);

impl TriangleId {
    pub const INVALID: TriangleId = TriangleId(usize::MAX);

    /// whether id is invalid
    pub fn invalid(&self) -> bool {
        self.0 == Self::INVALID.0
    }

    pub fn get<'a, 'b>(&'a self, triangles: &'b Triangles) -> &'b Triangle {
        triangles.get(*self).unwrap()
    }

    pub fn get_mut<'a, 'b>(&'a self, triangles: &'b mut Triangles) -> &'b mut Triangle {
        triangles.get_mut_unchecked(*self)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
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

    pub fn get(&self, id: TriangleId) -> Option<&Triangle> {
        if id == TriangleId::INVALID {
            return None;
        }
        unsafe { Some(self.triangles.get_unchecked(id.0)) }
    }

    pub fn get_mut(&mut self, id: TriangleId) -> Option<&mut Triangle> {
        self.triangles.get_mut(id.0)
    }

    pub fn get_mut_unchecked(&mut self, id: TriangleId) -> &mut Triangle {
        unsafe { self.triangles.get_unchecked_mut(id.0) }
    }

    pub fn iter(&self) -> impl Iterator<Item = (TriangleId, &Triangle)> {
        self.triangles
            .iter()
            .enumerate()
            .map(|(idx, t)| (TriangleId(idx), t))
    }

    /// mark two triangle as neighbor
    pub fn mark_neighbor(&mut self, left: TriangleId, right: TriangleId) {
        let left_triangle = self.get(left).unwrap().clone();
        let right_triangle = self.get(right).unwrap().clone();

        if right_triangle.contains_pair((left_triangle.points[1], left_triangle.points[2])) {
            self.get_mut_unchecked(left).neighbors[0] = right;
        } else if right_triangle.contains_pair((left_triangle.points[0], left_triangle.points[2])) {
            self.get_mut_unchecked(left).neighbors[1] = right;
        } else if right_triangle.contains_pair((left_triangle.points[0], left_triangle.points[1])) {
            self.get_mut_unchecked(left).neighbors[2] = right;
        }

        if left_triangle.contains_pair((right_triangle.points[1], right_triangle.points[2])) {
            self.get_mut_unchecked(right).neighbors[0] = left;
        } else if left_triangle.contains_pair((right_triangle.points[0], right_triangle.points[2]))
        {
            self.get_mut_unchecked(right).neighbors[1] = left;
        } else if left_triangle.contains_pair((right_triangle.points[0], right_triangle.points[1]))
        {
            self.get_mut_unchecked(right).neighbors[2] = left;
        }
    }

    pub fn set_constrained(&mut self, id: TriangleId, index: usize, val: bool) {
        self.get_mut_unchecked(id).constrained_edge[index] = val;
    }

    pub fn legalize(&mut self, id: TriangleId, o_point: PointId, n_point: PointId) {
        self.get_mut_unchecked(id).legalize(o_point, n_point);
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
            let t = triangles.get(t1).unwrap();
            assert_eq!(t.neighbors[0], t2);
            let t = triangles.get(t2).unwrap();
            assert_eq!(t.neighbors[2], t1);
        }
    }
}
