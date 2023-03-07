mod advancing_front;
mod edge;
mod points;
mod shape;
mod triangles;
mod utils;
use advancing_front::{AdvancingFront, Node};
use edge::Edges;
use points::Points;
use rustc_hash::FxHashSet;
use shape::*;
use triangles::{TriangleId, Triangles};
use utils::{in_circle, orient_2d, Orientation};

use crate::advancing_front::LocateNode;

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

    pub fn add_point(&mut self, point: Point) -> PointId {
        self.points.add_point(point)
    }

    pub fn triangulate(&mut self) {
        // sort all points first
        self.points = std::mem::take(&mut self.points).into_sorted();

        let initial_triangle = self.triangles.insert(Triangle::new(
            self.points.get_id_by_y(0).unwrap(),
            Points::HEAD_ID,
            Points::TAIL_ID,
        ));

        // create the advancing front with initial triangle
        let advancing_front = AdvancingFront::new(
            self.triangles.get(initial_triangle).unwrap(),
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
                &self.points,
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
        points: &Points,
    ) {
        println!("point event: {point:?}");

        match advancing_front.locate_node(point.x) {
            None => {
                unreachable!()
            }
            Some(LocateNode::Middle((node_point, node)) | LocateNode::Left((node_point, node))) => {
                let (_, right) = advancing_front.next_node(node_point).unwrap();

                let triangle =
                    triangles.insert(Triangle::new(point_id, node.point_id, right.point_id));
                let node_triangle = node.triangle.unwrap();
                triangles.mark_neighbor(node_triangle, triangle);
                map.insert(triangle);
                advancing_front.insert(point_id, point, triangle);

                if !Self::legalize(triangle, points, triangles, advancing_front) {
                    Self::map_triangle_to_nodes(triangle, triangles, advancing_front, points)
                }

                // in middle case, the node's x should be less than point'x
                // in left case, they are same.
                if point.x <= node_point.x + f64::EPSILON {
                    Self::fill(node_point, points, triangles, advancing_front, map);
                }

                Self::fill_advancing_front(point, points, triangles, advancing_front, map);
            }
        }
    }

    /// returns whether it is changed
    fn legalize(
        triangle_id: TriangleId,
        points: &Points,
        triangles: &mut Triangles,
        advancing_front: &mut AdvancingFront,
    ) -> bool {
        // To legalize a triangle we start by finding if any of the three edges
        // violate the Delaunay condition
        for i in 0..3 {
            let triangle = triangles.get(triangle_id).unwrap();
            if triangle.delaunay_edge[i] {
                continue;
            }

            let ot_id = triangle.neighbors[i];
            if let Some(ot) = triangles.get(ot_id) {
                let p = triangle.points[i];
                let op = ot.opposite_point(&triangle, p);

                let oi = ot.point_index(op);

                // if this is a constrained edge or a delaunay edge(only during recursive legalization)
                // then we should not try to legalize
                if ot.constrained_edge[oi] || ot.delaunay_edge[oi] {
                    triangles.set_constrained(triangle_id, i, ot.constrained_edge[oi]);
                    continue;
                }

                // all point id is maintained by points.
                let inside = unsafe {
                    in_circle(
                        points.get_point_uncheck(p),
                        points.get_point_uncheck(triangle.point_ccw(p)),
                        points.get_point_uncheck(triangle.point_cw(p)),
                        points.get_point_uncheck(op),
                    )
                };

                if inside {
                    // first mark this shared edge as delaunay
                    triangles.get_mut(triangle_id).delaunay_edge[i] = true;
                    triangles.get_mut(ot_id).delaunay_edge[oi] = true;

                    // rotate shared edge one vertex cw to legalize it
                    Self::rotate_triangle_pair(triangle_id, p, ot_id, op, triangles);

                    // We now got one valid Delaunay Edge shared by two triangles
                    // This gives us 4 new edges to check for Delaunay
                    let not_legalized =
                        !Self::legalize(triangle_id, points, triangles, advancing_front);
                    if not_legalized {
                        Self::map_triangle_to_nodes(
                            triangle_id,
                            triangles,
                            advancing_front,
                            points,
                        );
                    }

                    let not_legalized = !Self::legalize(ot_id, points, triangles, advancing_front);
                    if not_legalized {
                        Self::map_triangle_to_nodes(ot_id, triangles, advancing_front, points);
                    }

                    triangles.get_mut(triangle_id).delaunay_edge[i] = false;
                    triangles.get_mut(ot_id).delaunay_edge[oi] = false;

                    // If triangle have been legalized no need to check the other edges since
                    // the recursive legalization will handles those so we can end here.
                    return true;
                }
            }
        }

        false
    }

    fn rotate_triangle_pair(
        triangle_id: TriangleId,
        p: PointId,
        ot_id: TriangleId,
        op: PointId,
        triangles: &mut Triangles,
    ) {
        let t = triangles.get(triangle_id).unwrap();
        let ot = triangles.get(ot_id).unwrap();

        let n1 = t.neighbor_ccw(p);
        let n2 = t.neighbor_cw(p);
        let n3 = ot.neighbor_ccw(op);
        let n4 = ot.neighbor_cw(op);

        let ce1 = t.constrained_edge_ccw(p);
        let ce2 = t.constrained_edge_cw(p);
        let ce3 = ot.constrained_edge_ccw(op);
        let ce4 = ot.constrained_edge_cw(op);

        let de1 = t.delaunay_edge_ccw(p);
        let de2 = t.delaunay_edge_cw(p);
        let de3 = ot.delaunay_edge_ccw(op);
        let de4 = ot.delaunay_edge_cw(op);

        // rotate shared edge one vertex cw to legalize it
        triangles.legalize(triangle_id, p, op);
        triangles.legalize(ot_id, p, op);

        let t = triangles.get_mut(triangle_id);
        t.set_delunay_edge_cw(p, de2);
        t.set_delunay_edge_ccw(op, de3);
        t.set_constrained_edge_cw(p, ce2);
        t.set_constrained_edge_ccw(op, ce3);
        t.clear_neighbors();

        let ot = triangles.get_mut(ot_id);
        ot.set_delunay_edge_ccw(p, de1);
        ot.set_delunay_edge_cw(op, de4);
        ot.set_constrained_edge_ccw(p, ce1);
        ot.set_constrained_edge_cw(op, ce4);
        ot.clear_neighbors();

        if !n1.invalid() {
            triangles.mark_neighbor(ot_id, n1);
        }
        if !n2.invalid() {
            triangles.mark_neighbor(triangle_id, n2);
        }
        if !n3.invalid() {
            triangles.mark_neighbor(triangle_id, n3);
        }
        if !n4.invalid() {
            triangles.mark_neighbor(ot_id, n4);
        }

        triangles.mark_neighbor(triangle_id, ot_id);
    }

    /// update advancing front node's triangle
    fn map_triangle_to_nodes(
        triangle_id: TriangleId,
        triangles: &Triangles,
        advancing_front: &mut AdvancingFront,
        points: &Points,
    ) {
        let triangle = triangles.get(triangle_id).unwrap();
        for i in 0..3 {
            if triangle.neighbors[i].invalid() {
                let point = points
                    .get_point(triangle.point_cw(triangle.points[i]))
                    .expect("should exist");
                if let Some(node) = advancing_front.locate_point_mut(point) {
                    node.triangle = Some(triangle_id);
                }
            }
        }
    }

    // todo: now advancing_front didn't delete the filled node
    fn fill(
        node_point: Point,
        points: &Points,
        triangles: &mut Triangles,
        advancing_front: &mut AdvancingFront,
        map: &mut FxHashSet<TriangleId>,
    ) {
        // all following nodes exists for sure
        let node = advancing_front.get_node(node_point).unwrap();
        let prev_node = advancing_front.prev_node(node_point).unwrap();
        let next_node = advancing_front.next_node(node_point).unwrap();

        let triangle_id = triangles.insert(Triangle::new(
            prev_node.1.point_id,
            node.point_id,
            next_node.1.point_id,
        ));

        if let Some(prev_tri) = prev_node.1.triangle {
            triangles.mark_neighbor(triangle_id, prev_tri);
        }
        if let Some(node_tri) = node.triangle {
            triangles.mark_neighbor(triangle_id, node_tri);
        }
        map.insert(triangle_id);

        if !Self::legalize(triangle_id, points, triangles, advancing_front) {
            Self::map_triangle_to_nodes(triangle_id, triangles, advancing_front, points);
        }
    }

    fn fill_advancing_front(
        node_point: Point,
        points: &Points,
        triangles: &mut Triangles,
        advancing_front: &mut AdvancingFront,
        map: &mut FxHashSet<TriangleId>,
    ) {
        // fill right holes
        while let Some((node_point, _)) = advancing_front.next_node(node_point) {
            if advancing_front.next_node(node_point).is_some() {
                // if HoleAngle exceeds 90 degrees then break
                if Self::large_hole_dont_fill(node_point, &advancing_front) {
                    break;
                }

                Self::fill(node_point, points, triangles, advancing_front, map);
            } else {
                break;
            }
        }

        // fill left holes
        while let Some((node_point, _)) = advancing_front.prev_node(node_point) {
            if advancing_front.prev_node(node_point).is_some() {
                // if HoleAngle exceeds 90 degrees then break
                if Self::large_hole_dont_fill(node_point, &advancing_front) {
                    break;
                }

                Self::fill(node_point, points, triangles, advancing_front, map);
            } else {
                break;
            }
        }

        // file right basins
        if let Some(basin_angle) = Self::basin_angle(node_point, advancing_front) {
            if basin_angle < std::f64::consts::FRAC_PI_4 * 3. {
                Self::fill_basin(node_point, points, triangles, advancing_front, map);
            }
        }
    }

    fn edge_event(&self, edge: Edge) {
        println!("edge event: {edge:?}");
    }

    fn large_hole_dont_fill(node_point: Point, advancing_front: &AdvancingFront) -> bool {
        let (next_point, _next_node) = advancing_front.next_node(node_point).unwrap();
        let (prev_point, _prev_node) = advancing_front.prev_node(node_point).unwrap();

        let angle = utils::Angle::new(node_point, next_point, prev_point);
        if angle.exceeds_90_degree() {
            return false;
        }
        if angle.is_negative() {
            return true;
        }

        // the original implentation also add two new check, which is not stated in the paper.
        // I just leave it later, will try it when have deeper understanding.

        true
    }
}

struct Basin {
    left: Point,
    bottom: Point,
    right: Point,
    width: f64,
    left_higher: bool,
}

/// Basin related methods
impl SweepContext {
    fn basin_angle(node_point: Point, advancing_front: &AdvancingFront) -> Option<f64> {
        let (next_point, _) = advancing_front.next_node(node_point)?;
        let (next_next_point, _) = advancing_front.next_node(next_point)?;

        let ax = node_point.x - next_next_point.x;
        let ay = node_point.y - next_next_point.y;
        Some(ay.atan2(ax))
    }

    /// basin is like a bowl, we first identify it's left, bottom, right node.
    /// then fill it
    fn fill_basin(
        node_point: Point,

        points: &Points,
        triangles: &mut Triangles,
        advancing_front: &mut AdvancingFront,
        map: &mut FxHashSet<TriangleId>,
    ) -> Option<()> {
        let next_node = advancing_front.next_node(node_point)?;
        let next_next_node = advancing_front.next_node(next_node.0)?;

        // find the left
        let left: Point;
        if orient_2d(node_point, next_node.0, next_next_node.0).is_ccw() {
            left = next_next_node.0;
        } else {
            left = next_node.0;
        }

        // find the bottom
        let mut bottom: Point = left;
        while let Some((next_node_point, _)) = advancing_front.next_node(bottom) {
            if bottom.y >= next_node_point.y {
                bottom = next_node_point;
            } else {
                break;
            }
        }

        // no valid basin
        if bottom.eq(&left) {
            return None;
        }

        // find the right
        let mut right = bottom;
        while let Some((next_node_point, _)) = advancing_front.next_node(right) {
            if bottom.y < next_node_point.y {
                right = next_node_point;
            } else {
                break;
            }
        }
        if right.eq(&bottom) {
            // no valid basin
            return None;
        }

        let width = right.x - left.x;
        let left_higher: bool = left.y > right.y;

        Self::fill_basin_req(
            bottom,
            Basin {
                left,
                bottom,
                right,
                width,
                left_higher,
            },
            points,
            triangles,
            advancing_front,
            map,
        );

        Some(())
    }

    fn fill_basin_req(
        node: Point,
        basin: Basin,
        points: &Points,
        triangles: &mut Triangles,
        advancing_front: &mut AdvancingFront,
        map: &mut FxHashSet<TriangleId>,
    ) -> Option<()> {
        if Self::is_shallow(node, &basin) {
            // stop fill if basin is shallow
            return None;
        }

        Self::fill(node, points, triangles, advancing_front, map);

        // find the next node to fill
        let prev_point = advancing_front.prev_node(node)?.0;
        let next_point = advancing_front.next_node(node)?.0;

        if prev_point.eq(&basin.left) && next_point.eq(&basin.right) {
            return Some(());
        }

        let new_node = if prev_point.eq(&basin.left) {
            let next_next_point = advancing_front.next_node(next_point)?.0;
            if orient_2d(node, next_point, next_next_point).is_cw() {
                return None;
            }

            next_point
        } else if next_point.eq(&basin.right) {
            let prev_prev_point = advancing_front.prev_node(prev_point)?.0;
            if orient_2d(node, prev_point, prev_prev_point).is_ccw() {
                return None;
            }

            prev_point
        } else {
            // continue with the neighbor node with lowest Y value
            if prev_point.y < next_point.y {
                prev_point
            } else {
                next_point
            }
        };

        Self::fill_basin_req(new_node, basin, points, triangles, advancing_front, map)
    }

    fn is_shallow(node: Point, basin: &Basin) -> bool {
        let height = if basin.left_higher {
            basin.left.y - node.y
        } else {
            basin.right.y - node.y
        };

        basin.width > height
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
