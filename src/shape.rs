use crate::PointId;

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

        Self {
            p, q
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Default for Point {
    fn default() -> Self {
        Self {
            x: 0., y:0.
        }
    }
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Self {x, y}
    }
}