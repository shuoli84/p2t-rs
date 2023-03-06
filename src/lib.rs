mod points;
mod edge;
mod shape;
mod advancing_front;
use advancing_front::AdvancingFront;
use edge::Edges;
use points::Points;
use shape::*;

/// new type for point id, currently is the index in context
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointId(usize);

pub struct Sweep {
    
}

impl Sweep {
    pub fn triangulate(context: &mut SweepContext) {
        context.init_triangulate();
        // create the advancing front with initial triangle
        let advancing_front = AdvancingFront::new(Triangle::new(context.points.get_id_by_y(0).unwrap(), Points::HEAD_ID, Points::TAIL_ID), &context.points);
        dbg!(advancing_front);
    }

    pub fn sweep_points(&self, context: &mut SweepContext) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct SweepContext {
    points: Points,
    edges: Edges,
}


impl SweepContext {
    const ALPHA: f64 = 0.3;

    pub fn new(polyline: Vec<Point>) -> Self {
        let mut points = Points::new(vec![]);

        let edges = {
            let mut edge_list = vec![];

            let mut point_iter = polyline.iter().map(|p| (points.add_point(*p), p));
            let first_point = point_iter.next().expect("empty polyline");

            let mut last_point = first_point;
            loop {
                match point_iter.next() {
                    Some(p2) => {
                        let edge = Edge::new(last_point, p2);
                        edge_list.push(edge);
                        last_point = p2;
                    }
                    None => {
                        let edge = Edge::new(last_point, first_point);
                        edge_list.push(edge);
                        break;
                    }
                }

            }

            Edges::new(edge_list)
        };

        Self {
            points,
            edges,
        }
    }

    pub fn triangulate(&mut self) {
        self.init_triangulate();
        // create the advancing front with initial triangle
        let advancing_front = AdvancingFront::new(Triangle::new(self.points.get_id_by_y(0).unwrap(), Points::HEAD_ID, Points::TAIL_ID), &self.points);
        dbg!(advancing_front);
    }

    pub fn init_triangulate(&mut self) {
        self.points = std::mem::take(&mut self.points).into_sorted();
    }

    pub fn get_point_id(&self, y_order: usize) -> Option<PointId> {
        self.points.get_id_by_y(y_order)
    }

    pub fn create_advancing_front(&mut self) {
        // initial triangle
        let p0 = self.get_point_id(0).unwrap();
        let triangle = Triangle::new(p0, Points::TAIL_ID, Points::HEAD_ID);

        let mut map = Vec::new();
        map.push(triangle);
           

    }

    pub fn add_point(&mut self, point: Point) -> PointId {
        self.points.add_point(point)
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    /// flags to determine if an edge is a Constrained edge
    constrained_edge: [bool; 3],

    //// flags to determine if an edge is a Delauney edge
    delaunay_edge: [bool; 3],

    /// triangle points
    pub points: (PointId, PointId, PointId),

    /// Has this triangle been marked as an interior triangle?
    interior: bool,
}

impl Triangle {
    pub fn new(a: PointId, b: PointId, c: PointId) -> Self {
        Self {
            points: (a, b, c),
            constrained_edge: [false, false, false],
            delaunay_edge: [false, false, false],
            interior: false,
        }
    }

    pub fn get_point_0(&self, points: &[Point]) -> Point {
        unsafe { *points.get_unchecked(self.points.0.0) } 
    }

    pub fn get_point_1(&self, points: &[Point]) -> Point {
        unsafe { *points.get_unchecked(self.points.1.0) } 
    }

    pub fn get_point_2(&self, points: &[Point]) -> Point {
        unsafe { *points.get_unchecked(self.points.2.0) } 
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context() {

        let polyline = vec![
            Point::new(0., 0.),
            Point::new(2., 0.),
            Point::new(1., 4.),
            Point::new(0., 4.)
        ];
        let mut context = SweepContext::new(polyline);
        dbg!(&context);

        context.init_triangulate();

        context.triangulate();
        dbg!(&context);

        dbg!(context.edges.all_edges());
        dbg!(context.edges.p_for_q(PointId(2)));
    }
}