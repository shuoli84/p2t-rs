use crate::shape::Point;

#[derive(Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Clock Wise
    ///     
    ///  a     b    
    ///             c
    ///
    CW,
    /// Counter Clock Wise
    ///             c
    ///  a     b
    CCW,
    /// Collinear
    ///  a     b    c
    Collinear,
}

impl Orientation {
    pub fn is_cw(&self) -> bool {
        matches!(self, Self::CW)
    }

    pub fn is_ccw(&self) -> bool {
        matches!(self, Self::CCW)
    }

    pub fn is_collinear(&self) -> bool {
        matches!(self, Self::Collinear)
    }
}

pub fn orient_2d(a: Point, b: Point, c: Point) -> Orientation {
    let detleft = (a.x - c.x) * (b.y - c.y);
    let detright = (a.y - c.y) * (b.x - c.x);
    let val = detleft - detright;

    if val == 0. {
        Orientation::Collinear
    } else if val > 0. {
        Orientation::CCW
    } else {
        Orientation::CW
    }
}

/// check whether pd is in circle defined by pa, pb, pc
/// requirements: pa is known to be opposite side with pd.
pub fn in_circle(pa: Point, pb: Point, pc: Point, pd: Point) -> bool {
    let adx = pa.x - pd.x;
    let ady = pa.y - pd.y;
    let bdx = pb.x - pd.x;
    let bdy = pb.y - pd.y;

    let adxbdy = adx * bdy;
    let bdxady = bdx * ady;
    let oabd = adxbdy - bdxady;

    if oabd <= 0. {
        return false;
    }

    let cdx = pc.x - pd.x;
    let cdy = pc.y - pd.y;

    let cdxady = cdx * ady;
    let adxcdy = adx * cdy;
    let ocad = cdxady - adxcdy;

    if ocad <= 0. {
        return false;
    }

    let bdxcdy = bdx * cdy;
    let cdxbdy = cdx * bdy;

    let alift = adx * adx + ady * ady;
    let blift = bdx * bdx + bdy * bdy;
    let clift = cdx * cdx + cdy * cdy;

    let det = alift * (bdxcdy - cdxbdy) + blift * ocad + clift * oabd;

    det > 0.
}

pub fn in_scan_area(a: Point, b: Point, c: Point, d: Point) -> bool {
    let oadb = (a.x - b.x) * (d.y - b.y) - (d.x - b.x) * (a.y - b.y);
    if oadb >= -f64::EPSILON {
        return false;
    }

    let oadc = (a.x - c.x) * (d.y - c.y) - (d.x - c.x) * (a.y - c.y);
    if oadc <= f64::EPSILON {
        return false;
    }

    true
}

#[derive(Debug, Clone, Copy)]
pub struct Angle(f64);

impl Angle {
    /// Create angle from three points, is the angle for aob
    pub fn new(o: Point, a: Point, b: Point) -> Self {
        Self(angle(o, a, b))
    }

    /// whether the angle exceeds PI / 2
    pub fn exceeds_90_degree(&self) -> bool {
        self.0 > std::f64::consts::FRAC_PI_2
    }

    /// whether the angle is negative
    pub fn is_negative(&self) -> bool {
        self.0 < 0.
    }
}

/// Calculate angle for aob in radians
pub fn angle(o: Point, a: Point, b: Point) -> f64 {
    let ox = o.x;
    let oy = o.y;
    let dax = a.x - ox;
    let day = a.y - oy;
    let dbx = b.x - ox;
    let dby = b.y - oy;
    let x = dax * dby - day * dbx;
    let y = dax * dbx + day * dby;
    x.atan2(y)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    #[test]
    fn test_in_circle() {
        let pa = Point::new(0., 0.);
        let pb = Point::new(2., 0.);
        let pc = Point::new(1., 1.);
        assert!(in_circle(pa, pb, pc, Point::new(1.5, 0.6)));
    }

    #[test]
    fn test_orient_2d() {
        assert_eq!(
            orient_2d(Point::new(0., 0.), Point::new(0., 1.), Point::new(0., 2.)),
            Orientation::Collinear
        );

        assert_eq!(
            orient_2d(Point::new(0., 0.), Point::new(1., 1.), Point::new(2., 2.)),
            Orientation::Collinear
        );

        assert_eq!(
            orient_2d(Point::new(0., 0.), Point::new(1., 1.), Point::new(2., 3.)),
            Orientation::CCW
        );

        assert_eq!(
            orient_2d(Point::new(0., 0.), Point::new(1., 1.), Point::new(2., 1.)),
            Orientation::CW
        );
    }

    #[test]
    fn test_angle() {
        assert_eq!(
            angle(Point::new(0., 0.), Point::new(1., 0.), Point::new(1., 1.)),
            PI / 4.
        );
    }
}
