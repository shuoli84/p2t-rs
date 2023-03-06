mod points;
mod edge;
mod shape;
mod advancing_front;
use edge::Edges;
use shape::*;

use std::cmp::Ordering;

/// new type for point id, currently is the index in context
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointId(usize);

pub struct Sweep {
    
}

impl Sweep {
    pub fn triangulate(context: &mut SweepContext) {
        context.init_triangulate();
    }

    pub fn sweep_points(&self, context: &mut SweepContext) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct SweepContext {
    /// All points
    points: Vec<Point>,
    /// store sorted points in y-axis in `init_triangulation`
    sorted_points: Vec<PointId>,

    edges: Edges,

    head: Point,
    tail: Point,
}


impl SweepContext {
    const ALPHA: f64 = 0.3;
    const HEAD_ID: PointId = PointId(usize::MAX);
    const TAIL_ID: PointId = PointId(usize::MAX - 1);

    pub fn new(polyline: Vec<Point>) -> Self {
        let mut point_edges: Vec<Vec<Edge>> = vec![vec![]; polyline.len()];

        let edges = {
            let mut edge_list = vec![];

            let mut point_iter = polyline.iter().enumerate().map(|(idx, p)| (PointId(idx), p));
            let first_point = point_iter.next().expect("empty polyline");
            let mut last_point = first_point;
            loop {
                match point_iter.next() {
                    Some(p2) => {
                        let edge = Edge::new(last_point, p2);
                        // edge.q
                        edge_list.push(edge);
                        point_edges[edge.q.0].push(edge);
                        last_point = p2;
                    }
                    None => {
                        let edge = Edge::new(last_point, first_point);
                        edge_list.push(edge);
                        point_edges[edge.q.0].push(edge);
                        break;
                    }
                }

            }

            Edges::new(edge_list)
        };

        Self {
            points: polyline,
            sorted_points: Default::default(),
            edges,
            head: Default::default(),
            tail: Default::default(),
        }
    }

    pub fn init_triangulate(&mut self) {
        let mut xmax = self.points[0].x;
        let mut xmin = xmax;
        let mut ymax = self.points[0].y;
        let mut ymin = ymax;

        for point in self.points.iter() {
            xmax = xmax.max(point.x);
            xmin = xmin.min(point.x);
            ymax = ymax.max(point.y);
            ymin = ymin.min(point.y);
        }

        let dx = (xmax - xmin) * Self::ALPHA;
        let dy = (ymax - ymin) * Self::ALPHA;

        self.head = Point::new(xmin - dx, ymin - dy);
        self.tail = Point::new(xmax + dx, ymin - dy);

        // sort points
        let mut unsorted_points = self.points.iter().enumerate().map(|(idx, p)| (PointId(idx), p)).collect::<Vec<_>>();

        unsorted_points.sort_by(|p1, p2| {
            let p1 = p1.1;
            let p2 = p2.1;

            if p1.y < p2.y {
                Ordering::Less
            } else if p1.y == p2.y {
                if p1.x < p2.x {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            } else {
                Ordering::Greater
            }
        });

        self.sorted_points = unsorted_points.into_iter().map(|(idx,_)| idx).collect::<Vec<_>>();
    }

    pub fn get_point_id(&self, y_order: usize) -> Option<PointId> {
        self.sorted_points.get(y_order).cloned()
    }

    pub fn create_advancing_front(&mut self) {
        // initial triangle
        let p0 = self.get_point_id(0).unwrap();
        let triangle = Triangle::new(p0, Self::TAIL_ID, Self::HEAD_ID);

        let mut map = Vec::new();
        map.push(triangle);
           

    }

    pub fn add_point(&mut self, point: Point) -> PointId {
        let point_id = PointId(self.points.len());
        self.points.push(point);
        point_id
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
        dbg!(&context);

        dbg!(context.edges.all_edges());

        dbg!(context.edges.p_for_q(PointId(2)));
    }
}