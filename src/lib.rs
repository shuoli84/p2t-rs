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
use utils::{in_circle, orient_2d};

use crate::{advancing_front::LocateNode, utils::is_scan_area};

pub use points::PointId;

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
        let mut context = FillContext {
            points: &self.points,
            triangles: &mut self.triangles,
            advancing_front: &mut advancing_front,
            map: &mut self.map,
        };
        for (point_id, point) in self.points.iter_point_by_y(1) {
            Self::point_event(point_id, point, &mut context);
            for p in self.edges.p_for_q(point_id) {
                let edge = Edge { p: *p, q: point_id };
                Self::edge_event(edge, point, &mut context);
            }
        }
    }
}

/// Point event related methods
impl SweepContext {
    fn point_event(point_id: PointId, point: Point, context: &mut FillContext) {
        match context.advancing_front.locate_node(point.x) {
            None => {
                unreachable!()
            }
            Some(LocateNode::Middle((node_point, node)) | LocateNode::Left((node_point, node))) => {
                let (_, right) = context.advancing_front.next_node(node_point).unwrap();

                let triangle = context.triangles.insert(Triangle::new(
                    point_id,
                    node.point_id,
                    right.point_id,
                ));
                let node_triangle = node.triangle.unwrap();
                context.triangles.mark_neighbor(node_triangle, triangle);
                context.map.insert(triangle);
                context.advancing_front.insert(point_id, point, triangle);

                if !Self::legalize(triangle, context) {
                    Self::map_triangle_to_nodes(triangle, context)
                }

                // in middle case, the node's x should be less than point'x
                // in left case, they are same.
                if point.x <= node_point.x + f64::EPSILON {
                    Self::fill(node_point, context);
                }

                Self::fill_advancing_front(point, context);
            }
        }
    }

    /// returns whether it is changed
    fn legalize(triangle_id: TriangleId, context: &mut FillContext) -> bool {
        // To legalize a triangle we start by finding if any of the three edges
        // violate the Delaunay condition
        for i in 0..3 {
            let triangle = context.triangles.get(triangle_id).unwrap();
            if triangle.delaunay_edge[i] {
                continue;
            }

            let ot_id = triangle.neighbors[i];
            if let Some(ot) = context.triangles.get(ot_id) {
                let p = triangle.points[i];
                let op = ot.opposite_point(&triangle, p);

                let oi = ot.point_index(op).unwrap();

                // if this is a constrained edge or a delaunay edge(only during recursive legalization)
                // then we should not try to legalize
                if ot.constrained_edge[oi] || ot.delaunay_edge[oi] {
                    context
                        .triangles
                        .set_constrained(triangle_id, i, ot.constrained_edge[oi]);
                    continue;
                }

                // all point id is maintained by points.
                let inside = unsafe {
                    in_circle(
                        context.points.get_point_uncheck(p),
                        context.points.get_point_uncheck(triangle.point_ccw(p)),
                        context.points.get_point_uncheck(triangle.point_cw(p)),
                        context.points.get_point_uncheck(op),
                    )
                };

                if inside {
                    // first mark this shared edge as delaunay
                    context
                        .triangles
                        .get_mut_unchecked(triangle_id)
                        .delaunay_edge[i] = true;
                    context.triangles.get_mut_unchecked(ot_id).delaunay_edge[oi] = true;

                    // rotate shared edge one vertex cw to legalize it
                    Self::rotate_triangle_pair(triangle_id, p, ot_id, op, context.triangles);

                    // We now got one valid Delaunay Edge shared by two triangles
                    // This gives us 4 new edges to check for Delaunay
                    let not_legalized = !Self::legalize(triangle_id, context);
                    if not_legalized {
                        Self::map_triangle_to_nodes(triangle_id, context);
                    }

                    let not_legalized = !Self::legalize(ot_id, context);
                    if not_legalized {
                        Self::map_triangle_to_nodes(ot_id, context);
                    }

                    context
                        .triangles
                        .get_mut_unchecked(triangle_id)
                        .delaunay_edge[i] = false;
                    context.triangles.get_mut_unchecked(ot_id).delaunay_edge[oi] = false;

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
        triangles.legalize(ot_id, op, p);

        let t = triangles.get_mut_unchecked(triangle_id);
        t.set_delunay_edge_cw(p, de2);
        t.set_delunay_edge_ccw(op, de3);
        t.set_constrained_edge_cw(p, ce2);
        t.set_constrained_edge_ccw(op, ce3);
        t.clear_neighbors();

        let ot = triangles.get_mut_unchecked(ot_id);
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
    fn map_triangle_to_nodes(triangle_id: TriangleId, context: &mut FillContext) {
        let triangle = context.triangles.get(triangle_id).unwrap();
        for i in 0..3 {
            if triangle.neighbors[i].invalid() {
                let point = context
                    .points
                    .get_point(triangle.point_cw(triangle.points[i]))
                    .expect("should exist");
                if let Some(node) = context.advancing_front.locate_point_mut(point) {
                    node.triangle = Some(triangle_id);
                }
            }
        }
    }

    // todo: now advancing_front didn't delete the filled node
    fn fill(node_point: Point, context: &mut FillContext) {
        // all following nodes exists for sure
        let node = context.advancing_front.get_node(node_point).unwrap();
        let prev_node = context.advancing_front.prev_node(node_point).unwrap();
        let next_node = context.advancing_front.next_node(node_point).unwrap();

        let triangle_id = context.triangles.insert(Triangle::new(
            prev_node.1.point_id,
            node.point_id,
            next_node.1.point_id,
        ));

        if let Some(prev_tri) = prev_node.1.triangle {
            context.triangles.mark_neighbor(triangle_id, prev_tri);
        }
        if let Some(node_tri) = node.triangle {
            context.triangles.mark_neighbor(triangle_id, node_tri);
        }
        context.map.insert(triangle_id);

        if !Self::legalize(triangle_id, context) {
            Self::map_triangle_to_nodes(triangle_id, context);
        }
    }

    fn fill_advancing_front(node_point: Point, context: &mut FillContext) {
        // fill right holes
        while let Some((node_point, _)) = context.advancing_front.next_node(node_point) {
            if context.advancing_front.next_node(node_point).is_some() {
                // if HoleAngle exceeds 90 degrees then break
                if Self::large_hole_dont_fill(node_point, &context.advancing_front) {
                    break;
                }

                Self::fill(node_point, context);
            } else {
                break;
            }
        }

        // fill left holes
        while let Some((node_point, _)) = context.advancing_front.prev_node(node_point) {
            if context.advancing_front.prev_node(node_point).is_some() {
                // if HoleAngle exceeds 90 degrees then break
                if Self::large_hole_dont_fill(node_point, &context.advancing_front) {
                    break;
                }

                Self::fill(node_point, context);
            } else {
                break;
            }
        }

        // file right basins
        if let Some(basin_angle) = Self::basin_angle(node_point, context.advancing_front) {
            if basin_angle < std::f64::consts::FRAC_PI_4 * 3. {
                Self::fill_basin(node_point, context);
            }
        }
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

#[derive(Debug)]
struct EdgeEvent {
    constrained_edge: Edge,
    p: Point,
    q: Point,
    /// Whether the constrained edge is "right" edge, p.x larger than q.x
    right: bool,
}

impl EdgeEvent {
    fn p_id(&self) -> PointId {
        self.constrained_edge.p
    }

    fn q_id(&self) -> PointId {
        self.constrained_edge.q
    }

    /// create a new EdgeEvent with new p
    fn with_q(&self, point_id: PointId, point: Point) -> Self {
        EdgeEvent {
            constrained_edge: Edge {
                p: self.constrained_edge.p,
                q: point_id,
            },
            p: self.p,
            q: point,
            right: self.p.x > point.x,
        }
    }
}

/// EdgeEvent related methods
impl SweepContext {
    fn edge_event(edge: Edge, node_point: Point, context: &mut FillContext) {
        let p = context.points.get_point(edge.p).unwrap();
        let q = context.points.get_point(edge.q).unwrap();
        let edge_event = EdgeEvent {
            constrained_edge: edge,
            p,
            q,
            right: p.x > q.x,
        };
        println!("edge event: {edge_event:?}");

        let node = context.advancing_front.get_node(node_point).unwrap();

        if let Some(triangle) = node.triangle {
            if Self::try_mark_edge_for_triangle(&edge, triangle, context.triangles) {
                return;
            }

            // for now we will do all needed filling
            Self::fill_edge_event(&edge_event, node_point, context);
            Self::edge_event_for_point(&edge_event, triangle, edge.q, context);
        }
    }

    /// try mark edge for triangle if the constrained edge already is a edge
    /// returns `true` if yes, otherwise `false`
    fn try_mark_edge_for_triangle(
        edge: &Edge,
        t_id: TriangleId,
        triangles: &mut Triangles,
    ) -> bool {
        let triangle = triangles.get(t_id).unwrap();
        match triangle.edge_index(edge.p, edge.q) {
            None => {
                return false;
            }
            Some(index) => {
                let neighbor_t_id = triangle.neighbors[index];
                if let Some(t) = triangles.get_mut(neighbor_t_id) {
                    let index = t.edge_index(edge.p, edge.q).unwrap();
                    t.constrained_edge[index] = true;
                }

                triangles.get_mut_unchecked(t_id).constrained_edge[index] = true;

                true
            }
        }
    }

    fn fill_edge_event(edge: &EdgeEvent, node_point: Point, context: &mut FillContext) {
        if edge.right {
            Self::fill_right_above_edge_event(
                edge,
                node_point,
                &context.points,
                &mut context.triangles,
                &mut context.advancing_front,
                &mut context.map,
            );
        } else {
            Self::fill_left_above_edge_event(edge, node_point, context);
        }
    }

    fn fill_right_above_edge_event(
        edge: &EdgeEvent,
        mut node_point: Point,

        points: &Points,
        triangles: &mut Triangles,
        advancing_front: &mut AdvancingFront,
        map: &mut FxHashSet<TriangleId>,
    ) {
        while let Some((next_node_point, _)) = advancing_front.next_node(node_point) {
            if next_node_point.x >= edge.p.x {
                break;
            }

            // check if next node is below the edge
            if orient_2d(edge.p, next_node_point, edge.q).is_ccw() {
                Self::fill_right_below_edge_event(
                    edge,
                    node_point,
                    points,
                    triangles,
                    advancing_front,
                    map,
                );
            } else {
                // try next node
                node_point = next_node_point;
                continue;
            }
        }
    }

    fn fill_right_below_edge_event(
        edge: &EdgeEvent,
        node_point: Point,

        points: &Points,
        triangles: &mut Triangles,
        advancing_front: &mut AdvancingFront,
        map: &mut FxHashSet<TriangleId>,
    ) {
        let mut context = FillContext {
            points,
            triangles,
            advancing_front,
            map,
        };

        if node_point.x < edge.p.x {
            // todo: fixme
            let (next_node_point, _) = context.advancing_front.next_node(node_point).unwrap();
            let (next_next_node_point, _) =
                context.advancing_front.next_node(next_node_point).unwrap();

            if orient_2d(node_point, next_node_point, next_next_node_point).is_ccw() {
                // concave
                Self::fill_right_concave_edge_event(edge, node_point, &mut context);
            } else {
                // convex
                Self::fill_right_convex_edge_event(edge, node_point, &mut context);

                // retry this one
                Self::fill_right_below_edge_event(
                    edge,
                    node_point,
                    points,
                    triangles,
                    advancing_front,
                    map,
                );
            }
        }
    }

    /// recursively fill concave nodes
    fn fill_right_concave_edge_event(
        edge: &EdgeEvent,
        node_point: Point,
        context: &mut FillContext,
    ) {
        let (node_next_point, next_node) = context.advancing_front.next_node(node_point).unwrap();
        let next_node_point_id = next_node.point_id;
        Self::fill(node_next_point, context);

        if next_node_point_id != edge.p_id() {
            // next above or below edge?
            if orient_2d(edge.q, node_next_point, edge.p).is_ccw() {
                //  below
                let next_next_point = context
                    .advancing_front
                    .next_node(node_next_point)
                    .unwrap()
                    .0;
                if orient_2d(node_point, node_next_point, next_next_point).is_ccw() {
                    // next is concave
                    Self::fill_right_concave_edge_event(edge, node_point, context);
                } else {
                    // next is convex
                }
            }
        }
    }

    fn fill_right_convex_edge_event(
        edge: &EdgeEvent,
        node_point: Point,
        context: &mut FillContext,
    ) {
        let (next_node_point, _) = context.advancing_front.next_node(node_point).unwrap();
        let (next_next_node_point, _) = context.advancing_front.next_node(next_node_point).unwrap();
        let (next_next_next_node_point, _) = context
            .advancing_front
            .next_node(next_next_node_point)
            .unwrap();
        // next concave or convex?
        if orient_2d(
            next_node_point,
            next_next_node_point,
            next_next_next_node_point,
        )
        .is_ccw()
        {
            // concave
            Self::fill_right_concave_edge_event(edge, node_point, context);
        } else {
            // convex
            // next above or below edge?
            if orient_2d(edge.q, next_next_node_point, edge.p).is_ccw() {
                // Below
                Self::fill_right_convex_edge_event(edge, next_node_point, context);
            } else {
                // Above
            }
        }
    }

    fn fill_left_above_edge_event(
        edge: &EdgeEvent,
        mut node_point: Point,
        context: &mut FillContext,
    ) {
        while let Some((prev_node_point, _)) = context.advancing_front.prev_node(node_point) {
            // check if next node is below the edge
            if prev_node_point.x <= edge.p.x {
                break;
            }

            if orient_2d(edge.q, prev_node_point, edge.p).is_cw() {
                Self::fill_left_below_edge_event(edge, node_point, context);
            } else {
                node_point = prev_node_point;
            }
        }
    }

    fn fill_left_below_edge_event(edge: &EdgeEvent, node_point: Point, context: &mut FillContext) {
        if node_point.x > edge.p.x {
            let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
            let (prev_prev_node_point, _) =
                context.advancing_front.prev_node(prev_node_point).unwrap();
            if orient_2d(node_point, prev_node_point, prev_prev_node_point).is_cw() {
                Self::fill_left_concave_edge_event(edge, node_point, context);
            } else {
                // convex
                Self::fill_left_convex_edge_event(edge, node_point, context);

                // retry this one
                Self::fill_left_below_edge_event(edge, node_point, context);
            }
        }
    }

    fn fill_left_convex_edge_event(edge: &EdgeEvent, node_point: Point, context: &mut FillContext) {
        // next concave or convex?
        let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
        let (prev_prev_node_point, _) = context.advancing_front.prev_node(prev_node_point).unwrap();
        let (prev_prev_prev_node_point, _) = context
            .advancing_front
            .prev_node(prev_prev_node_point)
            .unwrap();

        if orient_2d(
            prev_node_point,
            prev_prev_node_point,
            prev_prev_prev_node_point,
        )
        .is_cw()
        {
            // concave
            Self::fill_left_concave_edge_event(edge, prev_node_point, context);
        } else {
            // convex
            // next above or below edge?
            if orient_2d(edge.q, prev_prev_node_point, edge.p).is_cw() {
                // below
                Self::fill_left_convex_edge_event(edge, node_point, context);
            } else {
                // above
            }
        }
    }

    fn fill_left_concave_edge_event(
        edge: &EdgeEvent,
        node_point: Point,
        context: &mut FillContext,
    ) {
        let (prev_node_point, _) = context.advancing_front.prev_node(node_point).unwrap();
        Self::fill(prev_node_point, context);

        let (prev_node_point, prev_node) = context.advancing_front.prev_node(node_point).unwrap();

        if prev_node.point_id != edge.p_id() {
            // next above or below edge?
            if orient_2d(edge.q, prev_node_point, edge.p).is_cw() {
                // below
                let (prev_prev_node_point, _) =
                    context.advancing_front.prev_node(prev_node_point).unwrap();
                if orient_2d(node_point, prev_node_point, prev_prev_node_point).is_cw() {
                    // next is concave
                    Self::fill_left_concave_edge_event(edge, node_point, context);
                } else {
                    // next is convex
                }
            }
        }
    }

    fn edge_event_for_point(
        edge: &EdgeEvent,
        triangle_id: TriangleId,
        point_id: PointId,
        context: &mut FillContext,
    ) {
        assert!(!triangle_id.invalid());

        if Self::try_mark_edge_for_triangle(&edge.constrained_edge, triangle_id, context.triangles)
        {
            return;
        }

        let triangle = context.triangles.get(triangle_id).unwrap();
        let p1 = triangle.point_ccw(point_id);
        let o1 = orient_2d(edge.q, context.points.get_point(point_id).unwrap(), edge.q);

        if o1.is_collinear() {
            if let Some(edge_index) = triangle.edge_index(edge.q_id(), p1) {
                let neighbor_across_t = triangle.neighbor_across(point_id);
                context
                    .triangles
                    .get_mut(triangle_id)
                    .unwrap()
                    .constrained_edge[edge_index] = true;

                Self::edge_event_for_point(
                    &edge.with_q(p1, context.points.get_point(p1).unwrap()),
                    neighbor_across_t,
                    p1,
                    context,
                );
                return;
            } else {
                panic!("EdgeEvent - collinear points not supported")
            }
        }

        let p2 = triangle.point_cw(point_id);
        let o2 = orient_2d(edge.q, context.points.get_point(p2).unwrap(), edge.p);
        if o2.is_collinear() {
            if let Some(edge_index) = triangle.edge_index(edge.q_id(), p2) {
                let neighbor_across_t = triangle.neighbor_across(point_id);
                context
                    .triangles
                    .get_mut(triangle_id)
                    .unwrap()
                    .constrained_edge[edge_index] = true;
                Self::edge_event_for_point(
                    &edge.with_q(p2, context.points.get_point(p2).unwrap()),
                    neighbor_across_t,
                    point_id,
                    context,
                );

                return;
            } else {
                panic!("collinear points not supported");
            }
        }

        if o1 == o2 {
            // need to decide if we are rotating cw or ccw to get to a triangle
            // that will cross edge
            let triangle_id = if o1.is_cw() {
                triangle.neighbor_ccw(point_id)
            } else {
                triangle.neighbor_cw(point_id)
            };

            Self::edge_event_for_point(edge, triangle_id, point_id, context);
        } else {
            // this triangle crosses constraint so let's flippin start!
            Self::flip_edge_event(edge, triangle_id, point_id, context);
        }
    }
}

struct FillContext<'a> {
    points: &'a Points,
    triangles: &'a mut Triangles,
    advancing_front: &'a mut AdvancingFront,
    map: &'a mut FxHashSet<TriangleId>,
}

/// flip edge related methods
impl SweepContext {
    fn flip_edge_event(
        edge: &EdgeEvent,
        triangle_id: TriangleId,
        point_id: PointId,
        context: &mut FillContext,
    ) {
        let t = context.triangles.get(triangle_id).unwrap();

        let ot = t.neighbor_across(point_id);
        assert!(!ot.invalid(), "neighbor must be valid");

        let ot = context.triangles.get(ot).unwrap();
        let op = ot.opposite_point(t, point_id);

        // is scan area?
        // is_scan_area(a, b, c, d)
    }
}

struct Basin {
    left: Point,
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
    fn fill_basin(node_point: Point, context: &mut FillContext) -> Option<()> {
        let next_node = context.advancing_front.next_node(node_point)?;
        let next_next_node = context.advancing_front.next_node(next_node.0)?;

        // find the left
        let left: Point;
        if orient_2d(node_point, next_node.0, next_next_node.0).is_ccw() {
            left = next_next_node.0;
        } else {
            left = next_node.0;
        }

        // find the bottom
        let mut bottom: Point = left;
        while let Some((next_node_point, _)) = context.advancing_front.next_node(bottom) {
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
        while let Some((next_node_point, _)) = context.advancing_front.next_node(right) {
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
                right,
                width,
                left_higher,
            },
            context,
        );

        Some(())
    }

    fn fill_basin_req(node: Point, basin: Basin, context: &mut FillContext) -> Option<()> {
        if Self::is_shallow(node, &basin) {
            // stop fill if basin is shallow
            return None;
        }

        Self::fill(node, context);

        // find the next node to fill
        let prev_point = context.advancing_front.prev_node(node)?.0;
        let next_point = context.advancing_front.next_node(node)?.0;

        if prev_point.eq(&basin.left) && next_point.eq(&basin.right) {
            return Some(());
        }

        let new_node = if prev_point.eq(&basin.left) {
            let next_next_point = context.advancing_front.next_node(next_point)?.0;
            if orient_2d(node, next_point, next_next_point).is_cw() {
                return None;
            }

            next_point
        } else if next_point.eq(&basin.right) {
            let prev_prev_point = context.advancing_front.prev_node(prev_point)?.0;
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

        Self::fill_basin_req(new_node, basin, context)
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
            // Point::new(0., 4.),
        ];
        let mut context = SweepContext::new(polyline);
        context.triangulate();
    }
}
