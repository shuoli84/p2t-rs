mod advancing_front;
mod edge;
mod points;
mod shape;
mod triangles;
mod utils;
use advancing_front::AdvancingFront;
use edge::Edges;
use points::Points;
use rustc_hash::FxHashSet;
use shape::*;
use triangles::{TriangleId, Triangles};

use crate::advancing_front::SearchNode;

/// new type for point id, currently is the index in context
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PointId(usize);

pub struct Sweep {}

impl Sweep {
    pub fn triangulate(context: &mut SweepContext) {
        unimplemented!()
    }

    pub fn sweep_points(&self, context: &mut SweepContext) {
        unimplemented!()
    }
}

#[derive(Debug)]
pub struct SweepContext {
    points: Points,
    edges: Edges,
    triangles: Triangles,
    map: FxHashSet<TriangleId>,
}

impl SweepContext {
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
            triangles: Triangles::new(),
            map: Default::default(),
        }
    }

    pub fn triangulate(&mut self) {
        self.init_triangulate();

        let initial_triangle = self.triangles.insert(Triangle::new(
            self.points.get_id_by_y(0).unwrap(),
            Points::HEAD_ID,
            Points::TAIL_ID,
        ));

        // create the advancing front with initial triangle
        let advancing_front = AdvancingFront::new(
            self.triangles.get(initial_triangle),
            initial_triangle,
            &self.points,
        );

        self.sweep_points(advancing_front);
    }

    fn sweep_points(&mut self, mut advancing_front: AdvancingFront) {
        for (point_id, point) in self.points.iter_point_by_y(1) {
            Self::point_event(
                point_id,
                point,
                &mut advancing_front,
                &mut self.triangles,
                &mut self.map,
            );
            for p in self.edges.p_for_q(point_id) {
                let edge = Edge { p: *p, q: point_id };
                self.edge_event(edge);
            }
        }
    }

    fn point_event(
        point_id: PointId,
        point: Point,
        advancing_front: &mut AdvancingFront,
        triangles: &mut Triangles,
        map: &mut FxHashSet<TriangleId>,
    ) {
        println!("point event: {point:?}");

        match advancing_front.search_node(point.x) {
            None => {
                unreachable!()
            }
            Some(SearchNode::Middle(left, right)) => {
                // create a new triange from (point, left, right)
                let triangle = triangles.insert(Triangle::new(point_id, left.point, right.point));
                let node_triangle = left.triangle.unwrap();
                triangles.mark_neighbor(node_triangle, triangle);
                map.insert(triangle);

                advancing_front.insert(point_id, point, triangle);

                // legalize
            }
            Some(SearchNode::Left(p)) => {
                todo!()
            }
        }
    }

    fn edge_event(&self, edge: Edge) {
        println!("edge event: {edge:?}");
    }

    fn init_triangulate(&mut self) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context() {
        let polyline = vec![
            Point::new(0., 0.),
            Point::new(2., 0.),
            Point::new(1., 4.),
            Point::new(0., 4.),
        ];
        let mut context = SweepContext::new(polyline);
        dbg!(&context);

        context.triangulate();
    }
}
