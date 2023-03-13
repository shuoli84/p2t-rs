use crate::shape::Triangle;

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TriangleId(usize);

impl TriangleId {
    pub const INVALID: TriangleId = TriangleId(usize::MAX);

    /// whether id is invalid
    pub fn invalid(&self) -> bool {
        self.0 == Self::INVALID.0
    }

    pub fn get<'a, 'b>(&'a self, triangles: &'b Triangles) -> &'b Triangle {
        triangles.get_unchecked(*self)
    }

    pub fn try_get<'a, 'b>(&'a self, triangles: &'b Triangles) -> Option<&'b Triangle> {
        triangles.get(*self)
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

    pub fn get_unchecked(&self, id: TriangleId) -> &Triangle {
        if id == TriangleId::INVALID {
            panic!("id should be valid");
        }
        unsafe { self.triangles.get_unchecked(id.0) }
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
        let left_triangle = self.get_unchecked(left);
        let right_triangle = self.get_unchecked(right);

        let (l_ei, r_ei) = if let Some(r_ei) =
            right_triangle.edge_index(left_triangle.points[1], left_triangle.points[2])
        {
            (0, r_ei)
        } else if let Some(r_ei) =
            right_triangle.edge_index(left_triangle.points[0], left_triangle.points[2])
        {
            (1, r_ei)
        } else if let Some(r_ei) =
            right_triangle.edge_index(left_triangle.points[0], left_triangle.points[1])
        {
            (2, r_ei)
        } else {
            debug_assert!(false, "they are not neighbors");
            return;
        };

        let is_constrained_edge =
            left_triangle.constrained_edge[l_ei] || right_triangle.constrained_edge[r_ei];
        {
            let left = self.get_mut_unchecked(left);
            left.neighbors[l_ei] = right;
            left.constrained_edge[l_ei] = is_constrained_edge;
        }

        {
            let right = self.get_mut_unchecked(right);
            right.neighbors[r_ei] = left;
            right.constrained_edge[r_ei] = is_constrained_edge;
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
            let t = triangles.get(t1).unwrap();
            assert_eq!(t.neighbors[0], t2);
            let t = triangles.get(t2).unwrap();
            assert_eq!(t.neighbors[2], t1);
        }
    }
}
