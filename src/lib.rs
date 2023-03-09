mod advancing_front;
mod context;
mod edge;
mod points;
mod shape;
mod triangles;
mod utils;
use advancing_front::AdvancingFront;
use context::FillContext;
use edge::Edges;
use points::Points;
use rustc_hash::FxHashSet;
use shape::*;
use triangles::{TriangleId, Triangles};
use utils::{in_circle, orient_2d, Orientation};

use crate::utils::in_scan_area;

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

            if polyline.is_empty() {
                Edges::new(vec![])
            } else {
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
            }
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
        let mut advancing_front = AdvancingFront::new(
            self.triangles.get(initial_triangle).unwrap(),
            initial_triangle,
            &self.points,
        );

        let mut context = FillContext {
            points: &self.points,
            triangles: &mut self.triangles,
            advancing_front: &mut advancing_front,
            edges: &self.edges,
            map: &mut self.map,
            result: Vec::new(),
        };

        Self::sweep_points(&mut context);
        Self::finalize_polygon(&mut context);

        context.draw();
    }

    fn sweep_points(context: &mut FillContext) {
        for (point_id, point) in context.points.iter_point_by_y(1) {
            Self::point_event(point_id, point, context);
            context.draw();

            for p in context.edges.p_for_q(point_id) {
                let edge = Edge { p: *p, q: point_id };
                Self::edge_event(edge, point, context);
                context.draw();
            }
        }
    }

    fn finalize_polygon(context: &mut FillContext) -> Option<()> {
        // get an internal triangle to start with
        // the first node is head, artificial point, so skip
        let (_, node) = context.advancing_front.nth(1)?;

        let mut t = node.triangle?;

        loop {
            if let Some(tri) = context.triangles.get(t) {
                if !tri.constrained_edge_cw(node.point_id) {
                    t = tri.neighbor_ccw(node.point_id);
                } else {
                    break;
                }
            }
        }

        if !t.invalid() {
            Self::clean_mesh(t, context);
        }

        Some(())
    }

    fn clean_mesh(triangle_id: TriangleId, context: &mut FillContext) -> Option<()> {
        let mut triangles = Vec::<TriangleId>::new();
        triangles.push(triangle_id);

        while let Some(t) = triangles.pop() {
            if t.invalid() {
                continue;
            }

            let tri = context.triangles.get_mut(t).unwrap();

            if !tri.interior {
                tri.interior = true;
                println!("adding tri: {}", t.as_usize());
                context.result.push(t);

                for i in 0..3 {
                    if !tri.constrained_edge[i] {
                        triangles.push(tri.neighbors[i]);
                    }
                }
            }
        }

        Some(())
    }
}

// first need to visualize each step, then trouble shoot
// print detailed steps, like what changes this going to address.

/// Point event related methods
impl SweepContext {
    fn point_event(point_id: PointId, point: Point, context: &mut FillContext) {
        println!("\npoint event: {point_id:?} {point:?}");

        let (node_point, node) = context.advancing_front.locate_node(point.x).unwrap();
        let (_, right) = context.advancing_front.next_node(node_point).unwrap();

        let triangle =
            context
                .triangles
                .insert(Triangle::new(point_id, node.point_id, right.point_id));
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

            // in this case, the advancing node should be deleted, as it is covered by new point
            context.advancing_front.delete(node_point);
        }

        Self::fill_advancing_front(point, context);
    }

    /// returns whether it is changed
    fn legalize(triangle_id: TriangleId, context: &mut FillContext) -> bool {
        println!("legalize {:?}", triangle_id);
        // To legalize a triangle we start by finding if any of the three edges
        // violate the Delaunay condition
        for point_idx in 0..3 {
            let triangle = context.triangles.get(triangle_id).unwrap();
            if triangle.delaunay_edge[point_idx] {
                continue;
            }

            let opposite_triangle_id = triangle.neighbors[point_idx];
            let Some(opposite_triangle) = context.triangles.get(opposite_triangle_id) else {
                continue;
            };

            let p = triangle.points[point_idx];
            let op = opposite_triangle.opposite_point(&triangle, p);

            let oi = opposite_triangle.point_index(op).unwrap();

            // if this is a constrained edge or a delaunay edge(only during recursive legalization)
            // then we should not try to legalize
            if opposite_triangle.constrained_edge[oi] || opposite_triangle.delaunay_edge[oi] {
                context.triangles.set_constrained(
                    triangle_id,
                    point_idx,
                    opposite_triangle.constrained_edge[oi],
                );
                continue;
            }

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
                    .delaunay_edge[point_idx] = true;
                context
                    .triangles
                    .get_mut_unchecked(opposite_triangle_id)
                    .delaunay_edge[oi] = true;

                // rotate shared edge one vertex cw to legalize it
                Self::rotate_triangle_pair(
                    triangle_id,
                    p,
                    opposite_triangle_id,
                    op,
                    context.triangles,
                );

                // We now got one valid Delaunay Edge shared by two triangles
                // This gives us 4 new edges to check for Delaunay
                let not_legalized = !Self::legalize(triangle_id, context);
                if not_legalized {
                    Self::map_triangle_to_nodes(triangle_id, context);
                }

                let not_legalized = !Self::legalize(opposite_triangle_id, context);
                if not_legalized {
                    Self::map_triangle_to_nodes(opposite_triangle_id, context);
                }

                context
                    .triangles
                    .get_mut_unchecked(triangle_id)
                    .delaunay_edge[point_idx] = false;
                context
                    .triangles
                    .get_mut_unchecked(opposite_triangle_id)
                    .delaunay_edge[oi] = false;

                // If triangle have been legalized no need to check the other edges since
                // the recursive legalization will handles those so we can end here.
                return true;
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

    fn fill(node_point: Point, context: &mut FillContext) -> Option<()> {
        // safety: all following nodes exists for sure
        let node = context.advancing_front.get_node(node_point).unwrap();
        let prev_node = context.advancing_front.prev_node(node_point)?;
        let next_node = context.advancing_front.next_node(node_point)?;

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
        context
            .advancing_front
            .insert(prev_node.1.point_id, prev_node.0, triangle_id);

        if !Self::legalize(triangle_id, context) {
            Self::map_triangle_to_nodes(triangle_id, context);
        }

        // this node is shadowed by new triangle, delete it from advancing front
        context.advancing_front.delete(node_point);
        Some(())
    }

    fn fill_advancing_front(node_point: Point, context: &mut FillContext) {
        {
            // fill right holes
            let mut node_point = node_point;
            while let Some((next_node_point, _)) = context.advancing_front.next_node(node_point) {
                if context.advancing_front.next_node(next_node_point).is_some() {
                    // if HoleAngle exceeds 90 degrees then break
                    if Self::large_hole_dont_fill(next_node_point, &context.advancing_front) {
                        break;
                    }

                    Self::fill(next_node_point, context);
                    node_point = next_node_point;
                } else {
                    break;
                }
            }
        }

        {
            // fill left holes
            let mut node_point = node_point;

            while let Some((prev_node_point, _)) = context.advancing_front.prev_node(node_point) {
                if context.advancing_front.prev_node(prev_node_point).is_some() {
                    // if HoleAngle exceeds 90 degrees then break
                    if Self::large_hole_dont_fill(prev_node_point, &context.advancing_front) {
                        break;
                    }

                    Self::fill(prev_node_point, context);
                    node_point = prev_node_point;
                } else {
                    break;
                }
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
            Self::edge_event_for_point(edge.p, edge.q, &edge_event, triangle, edge.q, context);
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
            Self::fill_right_above_edge_event(edge, node_point, context);
        } else {
            Self::fill_left_above_edge_event(edge, node_point, context);
        }
    }

    fn fill_right_above_edge_event(
        edge: &EdgeEvent,
        mut node_point: Point,

        context: &mut FillContext,
    ) {
        while let Some((next_node_point, _)) = context.advancing_front.next_node(node_point) {
            if next_node_point.x >= edge.p.x {
                break;
            }

            // check if next node is below the edge
            if orient_2d(edge.p, next_node_point, edge.q).is_ccw() {
                Self::fill_right_below_edge_event(edge, node_point, context);
            } else {
                // try next node
                node_point = next_node_point;
                continue;
            }
        }
    }

    fn fill_right_below_edge_event(edge: &EdgeEvent, node_point: Point, context: &mut FillContext) {
        if node_point.x < edge.p.x {
            // todo: fixme
            let (next_node_point, _) = context.advancing_front.next_node(node_point).unwrap();
            let (next_next_node_point, _) =
                context.advancing_front.next_node(next_node_point).unwrap();

            if orient_2d(node_point, next_node_point, next_next_node_point).is_ccw() {
                // concave
                Self::fill_right_concave_edge_event(edge, node_point, context);
            } else {
                // convex
                Self::fill_right_convex_edge_event(edge, node_point, context);

                // retry this one
                Self::fill_right_below_edge_event(edge, node_point, context);
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
                Self::fill_left_convex_edge_event(edge, prev_node_point, context);
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
        ep: PointId,
        eq: PointId,
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
        let o1 = orient_2d(
            eq.get(&context.points).unwrap(),
            p1.get(&context.points).unwrap(),
            ep.get(&context.points).unwrap(),
        );

        if o1.is_collinear() {
            if let Some(edge_index) = triangle.edge_index(edge.q_id(), p1) {
                let neighbor_across_t = triangle.neighbor_across(point_id);
                context
                    .triangles
                    .get_mut(triangle_id)
                    .unwrap()
                    .constrained_edge[edge_index] = true;

                Self::edge_event_for_point(ep, p1, &edge, neighbor_across_t, p1, context);
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
                Self::edge_event_for_point(ep, p2, &edge, neighbor_across_t, point_id, context);

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

            Self::edge_event_for_point(ep, eq, edge, triangle_id, point_id, context);
        } else {
            // this triangle crosses constraint so let's flippin start!
            Self::flip_edge_event(ep, eq, edge, triangle_id, point_id, context);
        }
    }
}

/// flip edge related methods
impl SweepContext {
    fn flip_edge_event(
        ep: PointId,
        eq: PointId,
        edge: &EdgeEvent,
        triangle_id: TriangleId,
        p: PointId,
        context: &mut FillContext,
    ) {
        assert!(!triangle_id.invalid());

        let t = context.triangles.get(triangle_id).unwrap();

        let ot_id = t.neighbor_across(p);
        assert!(!ot_id.invalid(), "neighbor must be valid");
        let ot = context.triangles.get(ot_id).unwrap();

        let op = ot.opposite_point(t, p);
        if in_scan_area(
            p.get(&context.points).unwrap(),
            t.point_ccw(p).get(&context.points).unwrap(),
            t.point_cw(p).get(&context.points).unwrap(),
            op.get(&context.points).unwrap(),
        ) {
            // lets rotate shared edge one vertex cw
            Self::rotate_triangle_pair(triangle_id, p, ot_id, op, &mut context.triangles);
            Self::map_triangle_to_nodes(triangle_id, context);
            Self::map_triangle_to_nodes(ot_id, context);

            if p == eq && op == ep {
                if eq == edge.q_id() && ep == edge.p_id() {
                    context
                        .triangles
                        .get_mut(triangle_id)
                        .unwrap()
                        .set_constrained_for_edge(ep, eq);

                    context
                        .triangles
                        .get_mut(ot_id)
                        .unwrap()
                        .set_constrained_for_edge(ep, eq);

                    Self::legalize(triangle_id, context);
                    Self::legalize(ot_id, context);
                } else {
                    // original comment: I think one of the triangles should be legalized here?
                    // todo: figure this out
                }
            } else {
                let o = orient_2d(
                    eq.get(&context.points).unwrap(),
                    op.get(&context.points).unwrap(),
                    ep.get(&context.points).unwrap(),
                );

                let t = Self::next_flip_triangle(o, triangle_id, ot_id, p, op, context);
                Self::flip_edge_event(ep, eq, edge, t, p, context);
            }
        } else {
            let new_p = Self::next_flip_point(ep, eq, ot_id, op, context);
            Self::flip_scan_edge_event(ep, eq, edge, triangle_id, ot_id, new_p, context);
            Self::edge_event_for_point(ep, eq, edge, triangle_id, p, context);
        }
    }

    fn next_flip_triangle(
        o: Orientation,
        t: TriangleId,
        ot: TriangleId,
        p: PointId,
        op: PointId,
        context: &mut FillContext,
    ) -> TriangleId {
        if o.is_ccw() {
            // ot is not crossing edge after flip
            let edge_index = context
                .triangles
                .get(ot)
                .unwrap()
                .edge_index(p, op)
                .unwrap();
            context.triangles.get_mut_unchecked(ot).delaunay_edge[edge_index] = true;
            Self::legalize(ot, context);
            context
                .triangles
                .get_mut_unchecked(ot)
                .clear_delaunay_edges();
            t
        } else {
            // t is not crossing edge after flip
            let edge_index = context.triangles.get(t).unwrap().edge_index(p, op).unwrap();
            context.triangles.get_mut_unchecked(t).delaunay_edge[edge_index] = true;
            Self::legalize(t, context);
            context
                .triangles
                .get_mut_unchecked(t)
                .clear_delaunay_edges();

            ot
        }
    }

    fn next_flip_point(
        ep: PointId,
        eq: PointId,
        ot: TriangleId,
        op: PointId,
        context: &mut FillContext,
    ) -> PointId {
        let o2d = orient_2d(
            eq.get(&context.points).unwrap(),
            op.get(&context.points).unwrap(),
            ep.get(&context.points).unwrap(),
        );

        let ot = context.triangles.get(ot).unwrap();
        match o2d {
            Orientation::CW => {
                // right
                ot.point_ccw(op)
            }
            Orientation::CCW => {
                // left
                ot.point_cw(op)
            }
            Orientation::Collinear => {
                panic!("Opposing point on constrained edge");
            }
        }
    }

    fn flip_scan_edge_event(
        ep: PointId,
        eq: PointId,
        edge: &EdgeEvent,
        flip_triangle_id: TriangleId,
        t_id: TriangleId,
        p: PointId,
        context: &mut FillContext,
    ) {
        let t = t_id.get(&context.triangles);
        let ot = t.neighbor_across(p);
        if ot.invalid() {
            panic!("flip_scan_edge_event - null neighbor across");
        }

        let op = ot.get(&context.triangles).opposite_point(t, p);
        let flip_triangle = flip_triangle_id.get(&context.triangles);
        let p1 = flip_triangle.point_ccw(eq);
        let p2 = flip_triangle.point_cw(eq);

        if in_scan_area(
            eq.get(&context.points).unwrap(),
            p1.get(&context.points).unwrap(),
            p2.get(&context.points).unwrap(),
            op.get(&context.points).unwrap(),
        ) {
            // flip with new edge op -> eq
            Self::flip_edge_event(eq, op, edge, ot, op, context);

            // original comment:
            // TODO: Actually I just figured out that it should be possible to
            //       improve this by getting the next ot and op before the the above
            //       flip and continue the flipScanEdgeEvent here
            // set new ot and op here and loop back to inScanArea test
            // also need to set a new flip_triangle first
            // Turns out at first glance that this is somewhat complicated
            // so it will have to wait.
        } else {
            let new_p = Self::next_flip_point(ep, eq, ot, op, context);
            Self::flip_scan_edge_event(ep, eq, edge, flip_triangle_id, ot, new_p, context);
        }
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
    use rand::Rng;

    use super::*;

    fn attach_debugger() {
        let url = format!(
            "vscode://vadimcn.vscode-lldb/launch/config?{{'request':'attach','pid':{}}}",
            std::process::id()
        );
        std::process::Command::new("code")
            .arg("--open-url")
            .arg(url)
            .output()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1)); // Wait for debugger to attach
    }

    #[test]
    fn test_context() {
        // attach_debugger();

        let polyline = vec![
            Point::new(0., 0.),
            Point::new(200., 0.),
            Point::new(100., 400.),
            Point::new(0., 400.),
            Point::new(30., 300.),
            Point::new(140., 110.),
        ];
        let mut context = SweepContext::new(polyline);
        context.triangulate();
    }

    #[test]
    fn test_rand() {
        attach_debugger();

        let mut points = Vec::<Point>::new();
        for i in 0..100 {
            let x: f64 = rand::thread_rng().gen_range(0.0..800.);
            let y: f64 = rand::thread_rng().gen_range(0.0..800.);
            points.push(Point::new(x, y));
        }
        let mut context = SweepContext::new(vec![
            Point::new(-10., -10.),
            Point::new(810., -10.),
            Point::new(810., 810.),
            Point::new(-10., 810.),
        ]);
        for p in points {
            context.add_point(p);
        }
        context.triangulate();
    }
}
