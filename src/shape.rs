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

    /// whether two points are same.
    /// Note: the lib don't support duplicate point, so eq means they are same point
    ///    not two point with equal values
    pub fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

#[derive(Default, Clone, Copy)]
pub struct EdgeAttr(u8);

impl std::fmt::Debug for EdgeAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgeAttr")
            .field("constrained", &self.is_constrained())
            .field("delaunay", &self.is_delaunay())
            .finish()
    }
}

impl EdgeAttr {
    const CONSTRAINED: u8 = 1;
    const CONSTRAINED_UNSET: u8 = Self::ALL ^ Self::CONSTRAINED;
    const DELAUNAY: u8 = 1 << 1;
    const DELAUNAY_UNSET: u8 = Self::ALL ^ Self::DELAUNAY;

    const ALL: u8 = 0xFF;

    fn set_constrained(&mut self, val: bool) {
        if val {
            self.0 |= Self::CONSTRAINED;
        } else {
            self.0 &= Self::CONSTRAINED_UNSET;
        }
    }

    fn is_constrained(&self) -> bool {
        self.0 & Self::CONSTRAINED != 0
    }

    fn set_delaunay(&mut self, val: bool) {
        if val {
            self.0 |= Self::DELAUNAY;
        } else {
            self.0 &= Self::DELAUNAY_UNSET;
        }
    }

    fn is_delaunay(&self) -> bool {
        self.0 & Self::DELAUNAY != 0
    }
}

/// The triangle struct used internally.
#[derive(Debug, Clone, Copy)]
pub struct InnerTriangle {
    /// triangle points
    pub points: [PointId; 3],

    /// neighbors
    pub neighbors: [TriangleId; 3],

    pub edge_attrs: [EdgeAttr; 3],

    /// Has this triangle been marked as an interior triangle?
    pub interior: bool,
}

impl InnerTriangle {
    pub fn new(a: PointId, b: PointId, c: PointId) -> Self {
        Self {
            points: [a, b, c],
            edge_attrs: [Default::default(), Default::default(), Default::default()],
            neighbors: [TriangleId::INVALID; 3],
            interior: false,
        }
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
    pub fn opposite_point(&self, from_triangle: &InnerTriangle, point: PointId) -> PointId {
        let cw = from_triangle.point_cw(point);
        self.point_cw(cw)
    }

    /// get point index
    pub fn point_index(&self, point: PointId) -> Option<usize> {
        if self.points[0] == point {
            Some(0)
        } else if self.points[1] == point {
            Some(1)
        } else if self.points[2] == point {
            Some(2)
        } else {
            None
        }
    }

    /// set constrained flag for edge identified by `p` and `q`
    pub fn set_constrained_for_edge(&mut self, p: PointId, q: PointId) {
        if let Some(index) = self.edge_index(p, q) {
            self.edge_attrs[index].set_constrained(true);
        }
    }

    pub fn set_constrained(&mut self, edge_index: usize, val: bool) {
        self.edge_attrs[edge_index].set_constrained(val);
    }

    pub fn is_constrained(&self, edge_index: usize) -> bool {
        self.edge_attrs[edge_index].is_constrained()
    }

    pub fn set_delaunay(&mut self, edge_index: usize, val: bool) {
        self.edge_attrs[edge_index].set_delaunay(val);
    }

    pub fn is_delaunay(&self, edge_index: usize) -> bool {
        self.edge_attrs[edge_index].is_delaunay()
    }

    pub fn edge_attr_ccw(&self, p: PointId) -> EdgeAttr {
        if p == self.points[0] {
            self.edge_attrs[2]
        } else if p == self.points[1] {
            self.edge_attrs[0]
        } else if p == self.points[2] {
            self.edge_attrs[1]
        } else {
            panic!("point not belongs to triangle");
        }
    }

    pub fn set_edge_attr_ccw(&mut self, p: PointId, edge_attr: EdgeAttr) {
        if p == self.points[0] {
            self.edge_attrs[2] = edge_attr;
        } else if p == self.points[1] {
            self.edge_attrs[0] = edge_attr;
        } else if p == self.points[2] {
            self.edge_attrs[1] = edge_attr;
        } else {
            panic!("point not belongs to triangle");
        }
    }

    pub fn set_edge_attr_cw(&mut self, p: PointId, val: EdgeAttr) {
        if p == self.points[0] {
            self.edge_attrs[1] = val;
        } else if p == self.points[1] {
            self.edge_attrs[2] = val;
        } else if p == self.points[2] {
            self.edge_attrs[0] = val;
        } else {
            panic!("point not belongs to triangle");
        }
    }

    pub fn edge_attr_cw(&self, p: PointId) -> EdgeAttr {
        if p == self.points[0] {
            self.edge_attrs[1]
        } else if p == self.points[1] {
            self.edge_attrs[2]
        } else if p == self.points[2] {
            self.edge_attrs[0]
        } else {
            panic!("point not belongs to triangle");
        }
    }

    /// constrained edge flag for edge `cw` to given point
    pub fn constrained_edge_cw(&self, p: PointId) -> bool {
        match self.point_index(p) {
            Some(0) => self.edge_attrs[1].is_constrained(),
            Some(1) => self.edge_attrs[2].is_constrained(),
            Some(2) => self.edge_attrs[0].is_constrained(),
            _ => panic!("point not belongs to triangle"),
        }
    }

    /// neighbor counter clockwise to given point
    pub fn neighbor_ccw(&self, p: PointId) -> TriangleId {
        if p == self.points[0] {
            self.neighbors[2]
        } else if p == self.points[1] {
            self.neighbors[0]
        } else if p == self.points[2] {
            self.neighbors[1]
        } else {
            panic!("point not belongs to triangle");
        }
    }

    /// neighbor clockwise to given point
    pub fn neighbor_cw(&self, p: PointId) -> TriangleId {
        if p == self.points[0] {
            self.neighbors[1]
        } else if p == self.points[1] {
            self.neighbors[2]
        } else if p == self.points[2] {
            self.neighbors[0]
        } else {
            panic!("point not belongs to triangle");
        }
    }

    /// neighbor counter clockwise to given point
    pub fn neighbor_across(&self, p: PointId) -> TriangleId {
        let Some(point_index) = self.point_index(p) else {
            panic!("break here");
        };
        self.neighbors[point_index]
    }

    /// Rotate triangle clockwise around `o_point`
    pub fn rotate_cw(&mut self, o_point: PointId, n_point: PointId) {
        if o_point == self.points[0] {
            self.points[1] = self.points[0];
            self.points[0] = self.points[2];
            self.points[2] = n_point;
        } else if o_point == self.points[1] {
            self.points[2] = self.points[1];
            self.points[1] = self.points[0];
            self.points[0] = n_point;
        } else if o_point == self.points[2] {
            self.points[0] = self.points[2];
            self.points[2] = self.points[1];
            self.points[1] = n_point;
        } else {
            panic!("point not belongs to triangle")
        }
    }

    pub fn clear_neighbors(&mut self) {
        self.neighbors = [TriangleId::INVALID; 3];
    }

    pub fn edge_index(&self, p: PointId, q: PointId) -> Option<usize> {
        let p_index = self.point_index(p)?;
        let q_index = self.point_index(q)?;

        // 0, 1, 2 => 00 01 10
        Some(match p_index | q_index {
            0b01 => 2, // 0,1 or 1,0
            0b11 => 0, // 1,2 or 2,1
            0b10 => 1, // 0,2 or 2,0
            _ => return None,
        })
    }

    pub fn common_edge_index(&self, other: &Self) -> Option<(usize, usize)> {
        if let Some(r_ei) = other.edge_index(self.points[1], self.points[2]) {
            Some((0, r_ei))
        } else if let Some(r_ei) = other.edge_index(self.points[0], self.points[2]) {
            Some((1, r_ei))
        } else if let Some(r_ei) = other.edge_index(self.points[0], self.points[1]) {
            Some((2, r_ei))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PointId;

    #[test]
    fn test_legalize() {
        //
        //      1                1
        //     /  \              | \
        //   2  -  3   =>   2    |  3
        //                       | /
        //       4               4
        //
        let mut t = InnerTriangle::new(PointId(1), PointId(2), PointId(3));
        t.rotate_cw(PointId(1), PointId(4));
        assert_eq!(t.points, [PointId(3), PointId(1), PointId(4)]);

        //
        //       1               1
        //  4   / \          4 \
        //     /   \          \    \
        //    2  -  3   =>      2  - -  3
        //
        let mut t = InnerTriangle::new(PointId(1), PointId(2), PointId(3));
        t.rotate_cw(PointId(3), PointId(4));
        assert_eq!(t.points, [PointId(3), PointId(4), PointId(2)]);

        let mut t = InnerTriangle::new(PointId(1), PointId(2), PointId(3));
        t.rotate_cw(PointId(2), PointId(4));
        assert_eq!(t.points, [PointId(4), PointId(1), PointId(2)]);
    }

    #[test]
    fn test_edge_attr() {
        let mut attr = EdgeAttr::default();
        assert!(!attr.is_constrained());
        attr.set_constrained(false);
        assert!(!attr.is_constrained());
        attr.set_constrained(true);
        assert!(attr.is_constrained());
        attr.set_constrained(false);
        assert!(!attr.is_constrained());

        assert!(!attr.is_delaunay());
        attr.set_delaunay(false);
        assert!(!attr.is_delaunay());
        attr.set_delaunay(true);
        assert!(attr.is_delaunay());
        attr.set_delaunay(false);
        assert!(!attr.is_delaunay());
    }
}
