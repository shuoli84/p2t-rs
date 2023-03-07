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

pub fn is_scan_area(a: Point, b: Point, c: Point, d: Point) -> bool {
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

#[cfg(test)]
mod tests {
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
}
