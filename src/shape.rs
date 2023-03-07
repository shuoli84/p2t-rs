use crate::{triangles::TriangleId, PointId};

#[derive(Clone, Copy, Debug)]
pub struct Edge {
    /// p is the lower end
    pub p: PointId,
    /// q is the higher end
    pub q: PointId,
}

impl Edge {
    pub fn new((p1_id, p1): (PointId, &Point), (p2_id, p2): (PointId, &Point)) -> Self {
        let mut p: PointId = p1_id;
        let mut q: PointId = p2_id;

        if p1.y > p2.y {
            q = p1_id;
            p = p2_id;
        } else if p1.y == p2.y {
            if p1.x > p2.x {
                q = p1_id;
                p = p2_id;
            } else if p1.x == p2.x {
                assert!(false, "repeat points");
            }
        }

        Self { p, q }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Default for Point {
    fn default() -> Self {
        Self { x: 0., y: 0. }
    }
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    /// flags to determine if an edge is a Constrained edge
    pub constrained_edge: [bool; 3],

    //// flags to determine if an edge is a Delauney edge
    pub delaunay_edge: [bool; 3],

    /// triangle points
    pub points: [PointId; 3],

    /// Has this triangle been marked as an interior triangle?
    pub interior: bool,

    /// neighbors
    pub neighbors: [TriangleId; 3],
}

impl Triangle {
    pub fn new(a: PointId, b: PointId, c: PointId) -> Self {
        Self {
            points: [a, b, c],
            constrained_edge: [false, false, false],
            delaunay_edge: [false, false, false],
            interior: false,
            neighbors: [TriangleId::INVALID; 3],
        }
    }

    /// whether contains the point
    pub fn contains(&self, point_id: PointId) -> bool {
        self.points[0] == point_id || self.points[1] == point_id || self.points[2] == point_id
    }

    pub fn contains_pair(&self, points: (PointId, PointId)) -> bool {
        self.contains(points.0) && self.contains(points.1)
    }

    /// The point clockwise to given point
    pub fn point_cw(&self, point: PointId) -> PointId {
        if point == self.points[0] {
            self.points[2]
        } else if point == self.points[1] {
            self.points[0]
        } else if point == self.points[2] {
            self.points[1]
        } else {
            panic!("point not belongs to triangle");
        }
    }

    /// The point counter-clockwise to given point
    pub fn point_ccw(&self, point: PointId) -> PointId {
        if point == self.points[0] {
            self.points[1]
        } else if point == self.points[1] {
            self.points[2]
        } else if point == self.points[2] {
            self.points[0]
        } else {
            panic!("point not belongs to triangle");
        }
    }

    /// The opposite point for point in neighbor `from_triangle`
    pub fn opposite_point(&self, from_triangle: &Triangle, point: PointId) -> PointId {
        let cw = from_triangle.point_cw(point);
        self.point_cw(cw)
    }

    /// get point index
    pub fn point_index(&self, point: PointId) -> usize {
        if self.points[0] == point {
            0
        } else if self.points[1] == point {
            1
        } else if self.points[2] == point {
            2
        } else {
            panic!("point not belongs to triangle");
        }
    }

    /// Legalize triangle by rotating clockwise around `old_point`
    pub fn legalize(&mut self, old_point: PointId, new_point: PointId) {
        if old_point == self.points[0] {
            self.points[1] = self.points[0];
            self.points[0] = self.points[2];
            self.points[2] = new_point;
        } else if old_point == self.points[1] {
            self.points[2] = self.points[1];
            self.points[1] = self.points[0];
            self.points[0] = new_point;
        } else if old_point == self.points[2] {
            self.points[0] = self.points[2];
            self.points[2] = self.points[1];
            self.points[1] = new_point;
        } else {
            panic!("point not belongs to triangle")
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::PointId;

    use super::Triangle;

    #[test]
    fn test_legalize() {
        //
        //      1                1
        //     /  \              | \
        //   2  -  3   =>   2    |  3
        //                       | /
        //       4               4
        //
        let mut t = Triangle::new(PointId(1), PointId(2), PointId(3));
        t.legalize(PointId(1), PointId(4));
        assert_eq!(t.points, [PointId(3), PointId(1), PointId(4)]);

        //
        //       1               1
        //  4   / \          4 \
        //     /   \          \    \
        //    2  -  3   =>      2  - -  3
        //
        let mut t = Triangle::new(PointId(1), PointId(2), PointId(3));
        t.legalize(PointId(3), PointId(4));
        assert_eq!(t.points, [PointId(3), PointId(4), PointId(2)]);

        let mut t = Triangle::new(PointId(1), PointId(2), PointId(3));
        t.legalize(PointId(2), PointId(4));
        assert_eq!(t.points, [PointId(4), PointId(1), PointId(2)]);
    }
}
