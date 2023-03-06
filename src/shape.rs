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
    pub points: (PointId, PointId, PointId),

    /// Has this triangle been marked as an interior triangle?
    pub interior: bool,

    /// neighbors
    pub neighbors: (TriangleId, TriangleId, TriangleId),
}

impl Triangle {
    pub fn new(a: PointId, b: PointId, c: PointId) -> Self {
        Self {
            points: (a, b, c),
            constrained_edge: [false, false, false],
            delaunay_edge: [false, false, false],
            interior: false,
            neighbors: (
                TriangleId::INVALID,
                TriangleId::INVALID,
                TriangleId::INVALID,
            ),
        }
    }

    /// whether contains the point
    pub fn contains(&self, point_id: PointId) -> bool {
        self.points.0 == point_id || self.points.1 == point_id || self.points.2 == point_id
    }

    pub fn contains_pair(&self, points: (PointId, PointId)) -> bool {
        self.contains(points.0) && self.contains(points.1)
    }
}
